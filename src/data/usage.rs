use rusqlite::{Connection, params};

use super::clips::Store;

const MAX_USES: i64 = 2000;
const KEEP_DAYS: i64 = 90;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UseKind {
    Mic = 0,
    Cam = 1,
    Screen = 2,
}

impl UseKind {
    fn from_i64(v: i64) -> Option<Self> {
        match v {
            0 => Some(Self::Mic),
            1 => Some(Self::Cam),
            2 => Some(Self::Screen),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UseRow {
    pub kind: UseKind,
    pub app: String,
    pub start: i64,
    pub end: Option<i64>,
}

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS privacy_log (
             id    INTEGER PRIMARY KEY,
             kind  INTEGER NOT NULL,
             app   TEXT NOT NULL,
             start INTEGER NOT NULL,
             end   INTEGER
         );
         CREATE INDEX IF NOT EXISTS privacy_start ON privacy_log(start DESC);
         CREATE TABLE IF NOT EXISTS activity (
             day  INTEGER NOT NULL,
             app  TEXT NOT NULL,
             secs INTEGER NOT NULL,
             PRIMARY KEY (day, app)
         ) WITHOUT ROWID;",
    )
}

fn clean_app(app: &str) -> String {
    app.chars().filter(|c| !c.is_control()).take(60).collect()
}

impl Store {
    pub fn use_started(&self, kind: UseKind, app: &str, at: i64) {
        let _ = self.conn.execute(
            "INSERT INTO privacy_log (kind, app, start) VALUES (?1, ?2, ?3)",
            params![kind as i64, clean_app(app), at],
        );
        let _ = self.conn.execute(
            "DELETE FROM privacy_log WHERE id NOT IN (SELECT id FROM privacy_log ORDER BY start DESC LIMIT ?1)",
            params![MAX_USES],
        );
    }

    pub fn use_ended(&self, kind: UseKind, app: &str, at: i64) {
        let _ = self.conn.execute(
            "UPDATE privacy_log SET end = ?3 WHERE end IS NULL AND kind = ?1 AND app = ?2",
            params![kind as i64, clean_app(app), at],
        );
    }

    pub fn close_open_uses(&self, at: i64) {
        let _ = self.conn.execute("UPDATE privacy_log SET end = ?1 WHERE end IS NULL", params![at]);
    }

    pub fn recent_uses(&self, limit: i64) -> Vec<UseRow> {
        let Ok(mut stmt) = self
            .conn
            .prepare_cached("SELECT kind, app, start, end FROM privacy_log ORDER BY start DESC, id DESC LIMIT ?1")
        else {
            return Vec::new();
        };
        stmt.query_map(params![limit], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, Option<i64>>(3)?))
        })
        .map(|rows| {
            rows.flatten()
                .filter_map(|(k, app, start, end)| Some(UseRow { kind: UseKind::from_i64(k)?, app, start, end }))
                .collect()
        })
        .unwrap_or_default()
    }

    pub fn clear_uses(&self) {
        let _ = self.conn.execute("DELETE FROM privacy_log", []);
    }

    pub fn activity_add(&self, day: i64, app: &str, secs: i64) {
        if secs <= 0 {
            return;
        }
        let _ = self.conn.execute(
            "INSERT INTO activity (day, app, secs) VALUES (?1, ?2, ?3)
             ON CONFLICT(day, app) DO UPDATE SET secs = secs + excluded.secs",
            params![day, clean_app(app), secs],
        );
        let _ = self.conn.execute("DELETE FROM activity WHERE day < ?1", params![day - KEEP_DAYS]);
    }

    pub fn activity_since(&self, first_day: i64, limit: i64) -> Vec<(String, i64)> {
        let Ok(mut stmt) = self.conn.prepare_cached(
            "SELECT app, SUM(secs) AS total FROM activity WHERE day >= ?1 GROUP BY app ORDER BY total DESC LIMIT ?2",
        ) else {
            return Vec::new();
        };
        stmt.query_map(params![first_day, limit], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }

    pub fn activity_clear(&self) {
        let _ = self.conn.execute("DELETE FROM activity", []);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_use_is_logged_from_start_to_end() {
        let store = Store::in_memory().unwrap();
        store.use_started(UseKind::Mic, "chrome", 100);
        store.use_started(UseKind::Cam, "zoom", 150);
        store.use_ended(UseKind::Mic, "chrome", 160);
        let rows = store.recent_uses(10);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], UseRow { kind: UseKind::Cam, app: "zoom".into(), start: 150, end: None });
        assert_eq!(rows[1].end, Some(160));
        store.close_open_uses(200);
        assert_eq!(store.recent_uses(10)[0].end, Some(200));
        store.clear_uses();
        assert!(store.recent_uses(10).is_empty());
    }

    #[test]
    fn the_use_log_stays_bounded() {
        let store = Store::in_memory().unwrap();
        for i in 0..(MAX_USES + 30) {
            store.use_started(UseKind::Mic, "app", i);
        }
        assert_eq!(store.recent_uses(MAX_USES + 100).len() as i64, MAX_USES);
    }

    #[test]
    fn activity_adds_up_per_program_and_day() {
        let store = Store::in_memory().unwrap();
        store.activity_add(20000, "code", 60);
        store.activity_add(20000, "code", 30);
        store.activity_add(20000, "chrome", 200);
        store.activity_add(19990, "code", 1000);
        store.activity_add(20000, "idle", 0);
        assert_eq!(store.activity_since(20000, 10), [("chrome".to_string(), 200), ("code".to_string(), 90)]);
        assert_eq!(store.activity_since(19990, 10)[0], ("code".to_string(), 1090));
        store.activity_add(20000 + KEEP_DAYS + 5, "late", 5);
        assert!(store.activity_since(0, 10).iter().all(|(app, _)| app == "late"));
        store.activity_clear();
        assert!(store.activity_since(0, 10).is_empty());
    }
}
