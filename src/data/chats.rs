use rusqlite::{Connection, params};

use super::clips::Store;
use crate::services::ai::ChatTurn;
use crate::services::attach::{Attachment, Kind};

pub const MAX_CHATS: i64 = 60;
pub const MAX_MESSAGES: i64 = 200;
const MAX_TEXT_BYTES: usize = 120_000;
pub const MAX_TITLE_CHARS: usize = 40;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatInfo {
    pub id: i64,
    pub title: String,
    pub ts: i64,
}

pub(super) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ai_chats (
             id    INTEGER PRIMARY KEY,
             title TEXT NOT NULL,
             ts    INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS ai_chats_ts ON ai_chats(ts DESC);
         CREATE TABLE IF NOT EXISTS ai_msgs (
             id        INTEGER PRIMARY KEY,
             chat_id   INTEGER NOT NULL,
             assistant INTEGER NOT NULL,
             text      TEXT NOT NULL,
             files     TEXT NOT NULL DEFAULT ''
         );
         CREATE INDEX IF NOT EXISTS ai_msgs_chat ON ai_msgs(chat_id, id);",
    )
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

pub fn title_of(text: &str, files: &[Attachment]) -> String {
    let line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let line: String = line.chars().filter(|c| !c.is_control()).take(MAX_TITLE_CHARS).collect();
    if !line.is_empty() { line } else { files.first().map_or_else(|| "Chat".to_string(), |f| f.name.clone()) }
}

fn cut(text: &str) -> &str {
    if text.len() <= MAX_TEXT_BYTES {
        return text;
    }
    let mut end = MAX_TEXT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn files_json(files: &[Attachment]) -> String {
    if files.is_empty() {
        return String::new();
    }
    let list: Vec<serde_json::Value> = files
        .iter()
        .map(|f| match f.kind {
            Kind::Text => serde_json::json!({"n": f.name, "k": f.kind.id(), "t": cut(&f.text())}),
            _ => serde_json::json!({"n": f.name, "k": f.kind.id()}),
        })
        .collect();
    serde_json::Value::Array(list).to_string()
}

fn files_from(json: &str) -> Vec<Attachment> {
    let Ok(serde_json::Value::Array(list)) = serde_json::from_str(json) else { return Vec::new() };
    list.iter()
        .take(crate::services::attach::MAX_FILES)
        .filter_map(|v| {
            let kind = Kind::from_id(v["k"].as_str()?)?;
            Some(Attachment::restored(v["n"].as_str()?, kind, v["t"].as_str().unwrap_or("")))
        })
        .collect()
}

impl Store {
    pub fn chat_list(&self) -> Vec<ChatInfo> {
        let Ok(mut stmt) =
            self.conn.prepare_cached("SELECT id, title, ts FROM ai_chats ORDER BY ts DESC, id DESC LIMIT ?1")
        else {
            return Vec::new();
        };
        stmt.query_map(params![MAX_CHATS], |r| Ok(ChatInfo { id: r.get(0)?, title: r.get(1)?, ts: r.get(2)? }))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }

    pub fn chat_create(&self, title: &str) -> Option<i64> {
        let title: String = title.chars().filter(|c| !c.is_control()).take(MAX_TITLE_CHARS).collect();
        self.conn.execute("INSERT INTO ai_chats (title, ts) VALUES (?1, ?2)", params![title, now()]).ok()?;
        let id = self.conn.last_insert_rowid();
        let _ = self.conn.execute(
            "DELETE FROM ai_chats WHERE id NOT IN (SELECT id FROM ai_chats ORDER BY ts DESC, id DESC LIMIT ?1)",
            params![MAX_CHATS],
        );
        let _ = self.conn.execute("DELETE FROM ai_msgs WHERE chat_id NOT IN (SELECT id FROM ai_chats)", []);
        Some(id)
    }

    pub fn chat_add(&self, chat: i64, turn: &ChatTurn) {
        let _ = self.conn.execute(
            "INSERT INTO ai_msgs (chat_id, assistant, text, files) VALUES (?1, ?2, ?3, ?4)",
            params![chat, i64::from(turn.assistant), cut(&turn.text), files_json(&turn.attachments)],
        );
        let _ = self.conn.execute("UPDATE ai_chats SET ts = ?1 WHERE id = ?2", params![now(), chat]);
    }

    pub fn chat_turns(&self, chat: i64) -> Vec<ChatTurn> {
        let Ok(mut stmt) = self.conn.prepare_cached(
            "SELECT assistant, text, files FROM
                 (SELECT id, assistant, text, files FROM ai_msgs WHERE chat_id = ?1 ORDER BY id DESC LIMIT ?2)
             ORDER BY id",
        ) else {
            return Vec::new();
        };
        stmt.query_map(params![chat, MAX_MESSAGES], |r| {
            Ok(ChatTurn {
                assistant: r.get::<_, i64>(0)? != 0,
                text: r.get(1)?,
                attachments: files_from(&r.get::<_, String>(2)?),
            })
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }

    pub fn chat_delete(&self, chat: i64) {
        let _ = self.conn.execute("DELETE FROM ai_msgs WHERE chat_id = ?1", params![chat]);
        let _ = self.conn.execute("DELETE FROM ai_chats WHERE id = ?1", params![chat]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conversation_is_saved_and_read_back() {
        let store = Store::in_memory().unwrap();
        let id = store.chat_create("Plans").unwrap();
        let note = Attachment::restored("n.txt", Kind::Text, "the text");
        let photo = Attachment::restored("p.png", Kind::Image, "");
        store.chat_add(id, &ChatTurn { assistant: false, text: "Hi".into(), attachments: vec![note, photo] });
        store.chat_add(id, &ChatTurn::text(true, "Hello"));
        let turns = store.chat_turns(id);
        assert_eq!(turns.len(), 2);
        assert!(!turns[0].assistant && turns[1].assistant);
        assert_eq!(turns[0].attachments.len(), 2);
        assert_eq!(turns[0].attachments[0].text(), "the text", "text files come back whole");
        assert!(!turns[0].attachments[1].kept(), "pictures are not kept, only named");
        assert_eq!(store.chat_list()[0].title, "Plans");
    }

    #[test]
    fn the_last_used_conversation_is_listed_first_and_delete_removes_it() {
        let store = Store::in_memory().unwrap();
        let a = store.chat_create("first").unwrap();
        let b = store.chat_create("second").unwrap();
        store.conn.execute("UPDATE ai_chats SET ts = 100 WHERE id = ?1", params![b]).unwrap();
        store.chat_add(a, &ChatTurn::text(false, "bump"));
        assert_eq!(store.chat_list()[0].id, a);
        store.chat_delete(a);
        assert_eq!(store.chat_list().len(), 1);
        assert!(store.chat_turns(a).is_empty());
    }

    #[test]
    fn old_conversations_and_long_texts_are_limited() {
        let store = Store::in_memory().unwrap();
        for i in 0..MAX_CHATS + 5 {
            store.chat_create(&format!("chat {i}")).unwrap();
        }
        assert_eq!(store.chat_list().len() as i64, MAX_CHATS);
        let id = store.chat_list()[0].id;
        store.chat_add(id, &ChatTurn::text(false, "я".repeat(MAX_TEXT_BYTES)));
        assert!(store.chat_turns(id)[0].text.len() <= MAX_TEXT_BYTES);
    }

    #[test]
    fn titles_come_from_the_first_words() {
        assert_eq!(title_of("  how   do I\nfix this?  ", &[]), "how do I fix this?");
        assert_eq!(title_of(&"x".repeat(100), &[]).chars().count(), MAX_TITLE_CHARS);
        assert_eq!(title_of("", &[Attachment::restored("report.pdf", Kind::Pdf, "")]), "report.pdf");
        assert_eq!(title_of("", &[]), "Chat");
    }

    #[test]
    fn a_broken_files_column_gives_no_files() {
        assert!(files_from("not json").is_empty());
        assert!(files_from("[{\"k\":\"exe\",\"n\":\"x\"}]").is_empty());
        assert!(files_from("").is_empty());
    }
}
