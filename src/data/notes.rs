use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, params};

use super::clips::Store;

pub const MAX_ITEMS: i64 = 500;
pub const MAX_NAME_CHARS: usize = 60;
pub const MAX_DEPTH: usize = 4;
const MAX_TEXT_BYTES: usize = 800_000;
const DEFAULT_NAME: &str = "Note";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteItem {
    pub id: i64,
    pub parent: Option<i64>,
    pub folder: bool,
    pub name: String,
}

pub fn clean_name(name: &str) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).collect();
    name.trim().chars().take(MAX_NAME_CHARS).collect()
}

pub fn visible_rows<'a>(items: &'a [NoteItem], expanded: &HashSet<i64>) -> Vec<(usize, &'a NoteItem)> {
    fn walk<'a>(
        items: &'a [NoteItem],
        expanded: &HashSet<i64>,
        parent: Option<i64>,
        depth: usize,
        out: &mut Vec<(usize, &'a NoteItem)>,
    ) {
        if depth > MAX_DEPTH + 1 {
            return;
        }
        for item in items.iter().filter(|i| i.parent == parent) {
            out.push((depth, item));
            if item.folder && expanded.contains(&item.id) {
                walk(items, expanded, Some(item.id), depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(items, expanded, None, 0, &mut out);
    out
}

fn depth_of(conn: &Connection, id: Option<i64>) -> usize {
    let mut depth = 0;
    let mut at = id;
    while let Some(i) = at {
        depth += 1;
        if depth > MAX_DEPTH + 1 {
            break;
        }
        at = conn.query_row("SELECT parent FROM note_items WHERE id = ?1", params![i], |r| r.get(0)).ok().flatten();
    }
    depth
}

pub(super) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS note_items (
             id     INTEGER PRIMARY KEY,
             parent INTEGER,
             folder INTEGER NOT NULL DEFAULT 0,
             name   TEXT NOT NULL,
             text   TEXT NOT NULL DEFAULT ''
         );
         CREATE INDEX IF NOT EXISTS note_items_parent ON note_items(parent);
         CREATE TABLE IF NOT EXISTS kv (
             key TEXT PRIMARY KEY,
             val TEXT NOT NULL
         );",
    )?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM note_items", [], |r| r.get(0))?;
    if count == 0 {
        let legacy: String = conn
            .query_row("SELECT text FROM notes WHERE id = 1", [], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
            .unwrap_or_default();
        conn.execute(
            "INSERT INTO note_items (parent, folder, name, text) VALUES (NULL, 0, ?1, ?2)",
            params![DEFAULT_NAME, legacy],
        )?;
    }
    Ok(())
}

impl Store {
    pub fn note_items(&self) -> Vec<NoteItem> {
        let Ok(mut stmt) = self.conn.prepare_cached(
            "SELECT id, parent, folder, name FROM note_items ORDER BY folder DESC, name COLLATE NOCASE, id LIMIT ?1",
        ) else {
            return Vec::new();
        };
        stmt.query_map(params![MAX_ITEMS], |r| {
            Ok(NoteItem { id: r.get(0)?, parent: r.get(1)?, folder: r.get::<_, i64>(2)? != 0, name: r.get(3)? })
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }

    pub fn note_text(&self, id: i64) -> String {
        self.conn
            .query_row("SELECT text FROM note_items WHERE id = ?1 AND folder = 0", params![id], |r| r.get(0))
            .unwrap_or_default()
    }

    pub fn note_set_text(&self, id: i64, text: &str) {
        let end = (0..=text.len().min(MAX_TEXT_BYTES)).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
        let _ = self
            .conn
            .execute("UPDATE note_items SET text = ?2 WHERE id = ?1 AND folder = 0", params![id, &text[..end]]);
    }

    pub fn note_add(&self, parent: Option<i64>, folder: bool, name: &str) -> Option<i64> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM note_items", [], |r| r.get(0)).ok()?;
        if count >= MAX_ITEMS {
            return None;
        }
        if let Some(p) = parent {
            let is_folder: i64 =
                self.conn.query_row("SELECT folder FROM note_items WHERE id = ?1", params![p], |r| r.get(0)).ok()?;
            if is_folder == 0 || depth_of(&self.conn, parent) >= MAX_DEPTH {
                return None;
            }
        }
        let name = clean_name(name);
        let name = if name.is_empty() { DEFAULT_NAME.to_string() } else { name };
        self.conn
            .execute(
                "INSERT INTO note_items (parent, folder, name) VALUES (?1, ?2, ?3)",
                params![parent, i64::from(folder), name],
            )
            .ok()?;
        Some(self.conn.last_insert_rowid())
    }

    pub fn note_rename(&self, id: i64, name: &str) {
        let name = clean_name(name);
        if !name.is_empty() {
            let _ = self.conn.execute("UPDATE note_items SET name = ?2 WHERE id = ?1", params![id, name]);
        }
    }

    pub fn note_delete(&self, id: i64) {
        let _ = self.conn.execute(
            "WITH RECURSIVE sub(id) AS (
                 SELECT ?1 UNION ALL SELECT n.id FROM note_items n JOIN sub ON n.parent = sub.id
             ) DELETE FROM note_items WHERE id IN (SELECT id FROM sub)",
            params![id],
        );
    }

    pub fn note_move(&self, id: i64, parent: Option<i64>) -> bool {
        if let Some(p) = parent {
            let mut at = Some(p);
            let mut hops = 0;
            while let Some(i) = at {
                if i == id || hops > MAX_DEPTH + 1 {
                    return false;
                }
                hops += 1;
                at = self
                    .conn
                    .query_row("SELECT parent FROM note_items WHERE id = ?1", params![i], |r| r.get(0))
                    .ok()
                    .flatten();
            }
            let is_folder: Option<i64> =
                self.conn.query_row("SELECT folder FROM note_items WHERE id = ?1", params![p], |r| r.get(0)).ok();
            if is_folder != Some(1) || depth_of(&self.conn, parent) >= MAX_DEPTH {
                return false;
            }
        }
        self.conn.execute("UPDATE note_items SET parent = ?2 WHERE id = ?1", params![id, parent]).is_ok_and(|n| n > 0)
    }

    pub fn note_last(&self) -> Option<i64> {
        self.conn
            .query_row("SELECT val FROM kv WHERE key = 'note_last'", [], |r| r.get::<_, String>(0))
            .ok()?
            .parse()
            .ok()
    }

    pub fn note_set_last(&self, id: i64) {
        let _ = self.conn.execute(
            "INSERT INTO kv (key, val) VALUES ('note_last', ?1) ON CONFLICT(key) DO UPDATE SET val = excluded.val",
            params![id.to_string()],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::in_memory().unwrap()
    }

    #[test]
    fn the_old_note_becomes_the_first_sheet() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE notes (id INTEGER PRIMARY KEY CHECK (id = 1), text TEXT NOT NULL);
             INSERT INTO notes VALUES (1, 'my old note');",
        )
        .unwrap();
        init(&conn).unwrap();
        init(&conn).unwrap();
        let (n, text): (i64, String) =
            conn.query_row("SELECT COUNT(*), MAX(text) FROM note_items", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((n, text.as_str()), (1, "my old note"));
    }

    #[test]
    fn folders_nest_and_delete_with_their_content() {
        let s = store();
        let folder = s.note_add(None, true, "Work").unwrap();
        let sheet = s.note_add(Some(folder), false, "Plan").unwrap();
        let sub = s.note_add(Some(folder), true, "Old").unwrap();
        let deep = s.note_add(Some(sub), false, "Deep").unwrap();
        assert!(s.note_add(Some(sheet), false, "x").is_none(), "a sheet cannot hold items");
        s.note_set_text(deep, "hello");
        assert_eq!(s.note_text(deep), "hello");

        let items = s.note_items();
        let mut open = HashSet::new();
        let top: Vec<_> = visible_rows(&items, &open).iter().map(|(d, i)| (*d, i.name.clone())).collect();
        assert_eq!(top, [(0, "Work".to_string()), (0, "Note".to_string())]);
        open.extend([folder, sub]);
        let all: Vec<_> = visible_rows(&items, &open).iter().map(|(d, i)| (*d, i.name.as_str())).collect();
        assert_eq!(all, [(0, "Work"), (1, "Old"), (2, "Deep"), (1, "Plan"), (0, "Note")]);

        s.note_delete(folder);
        assert_eq!(s.note_items().len(), 1);
        assert_eq!(s.note_text(deep), "");
    }

    #[test]
    fn moves_refuse_cycles_and_names_are_cleaned() {
        let s = store();
        let a = s.note_add(None, true, "A").unwrap();
        let b = s.note_add(Some(a), true, "B").unwrap();
        assert!(!s.note_move(a, Some(b)), "a folder cannot go inside its own child");
        assert!(!s.note_move(a, Some(a)));
        assert!(s.note_move(b, None));
        let sheet = s.note_add(None, false, "  \n\tTab\u{0}bed  ").unwrap();
        s.note_rename(sheet, &"x".repeat(500));
        let name = s.note_items().into_iter().find(|i| i.id == sheet).unwrap().name;
        assert_eq!(name.chars().count(), MAX_NAME_CHARS);
        s.note_rename(sheet, "   ");
        assert_eq!(s.note_items().into_iter().find(|i| i.id == sheet).unwrap().name.chars().count(), MAX_NAME_CHARS);
    }

    #[test]
    fn depth_and_count_are_limited() {
        let s = store();
        let mut parent = None;
        for _ in 0..MAX_DEPTH {
            parent = Some(s.note_add(parent, true, "d").unwrap());
        }
        assert!(s.note_add(parent, true, "too deep").is_none());
        for i in 0..MAX_ITEMS {
            if s.note_add(None, false, &format!("n{i}")).is_none() {
                break;
            }
        }
        assert_eq!(s.note_items().len() as i64, MAX_ITEMS);
        assert!(s.note_add(None, false, "one more").is_none());
    }

    #[test]
    fn text_is_cut_on_a_char_boundary_and_last_sheet_is_remembered() {
        let s = store();
        let id = s.note_add(None, false, "t").unwrap();
        s.note_set_text(id, &"я".repeat(MAX_TEXT_BYTES));
        assert!(s.note_text(id).len() <= MAX_TEXT_BYTES);
        assert_eq!(s.note_last(), None);
        s.note_set_last(id);
        assert_eq!(s.note_last(), Some(id));
    }
}
