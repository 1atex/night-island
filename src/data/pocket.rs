use super::clips::Store;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PocketItem {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub text: String,
    pub url: String,
    pub source: String,
}

pub fn web_url(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("https://").or_else(|| value.strip_prefix("http://")) else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.is_empty()
        && !host.contains('@')
        && value.len() <= 2048
        && !value.chars().any(|c| c.is_control() || c.is_whitespace())
}

pub fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 64 && value.chars().all(|c| c.is_ascii_alphanumeric() || "{}-_".contains(c))
}

impl PocketItem {
    pub fn valid(&self) -> bool {
        identifier(&self.id)
            && matches!(self.kind.as_str(), "text" | "link" | "image")
            && self.title.chars().count() <= 160
            && !self.title.chars().any(char::is_control)
            && self.text.chars().count() <= 4096
            && !self.text.contains('\0')
            && (self.url.is_empty() || web_url(&self.url))
            && (self.source.is_empty() || web_url(&self.source))
            && if self.kind == "text" { !self.text.trim().is_empty() } else { !self.url.is_empty() }
    }
    pub fn content(&self) -> String {
        let value = if self.kind == "text" { &self.text } else { &self.url };
        if self.source.is_empty() || self.source == *value {
            value.clone()
        } else {
            format!("{value}\n\n{}", self.source)
        }
    }
}

pub(super) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS browser_pocket (id TEXT PRIMARY KEY, item TEXT NOT NULL, created INTEGER NOT NULL DEFAULT (unixepoch()));")
}

impl Store {
    pub fn pocket_items(&self) -> Vec<PocketItem> {
        let Ok(mut query) =
            self.conn.prepare("SELECT item FROM browser_pocket ORDER BY created DESC, rowid DESC LIMIT 100")
        else {
            return Vec::new();
        };
        query
            .query_map([], |row| row.get::<_, String>(0))
            .map(|rows| {
                rows.flatten()
                    .filter_map(|text| serde_json::from_str::<PocketItem>(&text).ok())
                    .filter(PocketItem::valid)
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn pocket_add(&self, item: &PocketItem) -> bool {
        if !item.valid() {
            return false;
        }
        let exists = self
            .conn
            .query_row("SELECT EXISTS(SELECT 1 FROM browser_pocket WHERE id=?1)", [&item.id], |row| {
                row.get::<_, bool>(0)
            })
            .unwrap_or(false);
        if exists {
            return true;
        }
        let count =
            self.conn.query_row("SELECT COUNT(*) FROM browser_pocket", [], |row| row.get::<_, i64>(0)).unwrap_or(100);
        if count >= 100 {
            return false;
        }
        let Ok(text) = serde_json::to_string(item) else { return false };
        self.conn.execute("INSERT INTO browser_pocket(id,item) VALUES (?1,?2)", params![item.id, text]).is_ok()
    }
    pub fn pocket_remove(&self, id: &str) -> bool {
        self.conn.execute("DELETE FROM browser_pocket WHERE id=?1", [id]).is_ok_and(|n| n == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pocket_is_persistent_bounded_and_idempotent() {
        let store = Store::in_memory().unwrap();
        let mut item = PocketItem {
            id: "one".into(),
            kind: "text".into(),
            title: "Research".into(),
            text: "Selected text".into(),
            source: "https://example.com/page".into(),
            url: String::new(),
        };
        assert!(store.pocket_add(&item));
        assert!(store.pocket_add(&item));
        assert_eq!(store.pocket_items(), vec![item.clone()]);
        for i in 1..100 {
            item.id = i.to_string();
            assert!(store.pocket_add(&item));
        }
        item.id = "overflow".into();
        assert!(!store.pocket_add(&item));
        assert!(store.pocket_remove("one"));
        assert!(store.pocket_add(&item));
        assert!(item.content().contains("https://example.com/page"));
    }
    #[test]
    fn pocket_rejects_executable_urls_and_invalid_content() {
        for url in [
            "file:///test.exe",
            "javascript:alert(1)",
            "https://user:pass@example.com",
            "https://",
            "https://example.com\n",
        ] {
            assert!(!web_url(url));
        }
        assert!(web_url("https://example.com/path?q=hello#section"));
        let item = PocketItem { id: "x".into(), kind: "text".into(), text: "".into(), ..Default::default() };
        assert!(!item.valid());
    }
}
