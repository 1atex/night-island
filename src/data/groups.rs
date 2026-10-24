use std::collections::HashMap;

use rusqlite::{Connection, params};

use super::clips::Store;
use crate::tools::appgroups::{self, BUILTIN};

pub const MAX_GROUPS: i64 = 12;
const MAX_ASSIGNED: i64 = 1000;
pub const PALETTE: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub id: i64,
    pub name: String,
    pub color: u8,
    pub builtin: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    User,
    Ai,
}

pub(super) fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS act_groups (
             id      INTEGER PRIMARY KEY,
             name    TEXT NOT NULL UNIQUE COLLATE NOCASE,
             color   INTEGER NOT NULL,
             builtin INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE IF NOT EXISTS act_apps (
             app      TEXT PRIMARY KEY,
             group_id INTEGER NOT NULL,
             by_user  INTEGER NOT NULL
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS kv (
             key TEXT PRIMARY KEY,
             val TEXT NOT NULL
         );",
    )?;
    for (i, (name, color)) in BUILTIN.iter().enumerate() {
        conn.execute(
            "INSERT OR IGNORE INTO act_groups (id, name, color, builtin) VALUES (?1, ?2, ?3, 1)",
            params![i as i64 + 1, name, i64::from(*color)],
        )?;
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupTotal {
    pub id: i64,
    pub name: String,
    pub color: u8,
    pub secs: i64,
    pub apps: Vec<(String, i64)>,
}

pub fn group_of(app: &str, assigned: &HashMap<String, (i64, Source)>, groups: &[Group]) -> i64 {
    let key = app.trim().to_lowercase();
    if let Some((id, _)) = assigned.get(&key).filter(|(id, _)| groups.iter().any(|g| g.id == *id)) {
        return *id;
    }
    appgroups::builtin_group(app)
        .and_then(|name| groups.iter().find(|g| g.name.eq_ignore_ascii_case(name)))
        .map_or(appgroups::OTHER_ID, |g| g.id)
}

pub fn totals(rows: &[(String, i64)], assigned: &HashMap<String, (i64, Source)>, groups: &[Group]) -> Vec<GroupTotal> {
    let mut out: Vec<GroupTotal> = Vec::new();
    for (app, secs) in rows.iter().filter(|(_, s)| *s > 0) {
        let id = group_of(app, assigned, groups);
        let at = match out.iter().position(|t| t.id == id) {
            Some(at) => at,
            None => {
                let Some(g) = groups.iter().find(|g| g.id == id) else { continue };
                out.push(GroupTotal { id, name: g.name.clone(), color: g.color, secs: 0, apps: Vec::new() });
                out.len() - 1
            }
        };
        out[at].secs += secs;
        out[at].apps.push((app.clone(), *secs));
    }
    for t in &mut out {
        t.apps.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    }
    out.sort_by(|a, b| b.secs.cmp(&a.secs).then_with(|| a.id.cmp(&b.id)));
    out
}

pub fn stack_days(
    rows: &[(i64, String, i64)],
    last_day: i64,
    assigned: &HashMap<String, (i64, Source)>,
    groups: &[Group],
) -> Vec<Vec<(u8, i64)>> {
    let mut days: Vec<Vec<(u8, i64)>> = vec![Vec::new(); 7];
    for (day, app, secs) in rows {
        let at = 6 - (last_day - day);
        if !(0..7).contains(&at) || *secs <= 0 {
            continue;
        }
        let id = group_of(app, assigned, groups);
        let Some(color) = groups.iter().find(|g| g.id == id).map(|g| g.color) else { continue };
        let stack = &mut days[at as usize];
        match stack.iter_mut().find(|(c, _)| *c == color) {
            Some(slot) => slot.1 += secs,
            None => stack.push((color, *secs)),
        }
    }
    for stack in &mut days {
        stack.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    }
    days
}

impl Store {
    pub fn act_groups(&self) -> Vec<Group> {
        let Ok(mut stmt) = self.conn.prepare_cached("SELECT id, name, color, builtin FROM act_groups ORDER BY id")
        else {
            return Vec::new();
        };
        stmt.query_map([], |r| {
            Ok(Group {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get::<_, i64>(2)?.clamp(0, PALETTE as i64 - 1) as u8,
                builtin: r.get::<_, i64>(3)? != 0,
            })
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }

    pub fn act_group_add(&self, name: &str) -> Option<i64> {
        let name = appgroups::clean_group(name)?;
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM act_groups", [], |r| r.get(0)).ok()?;
        if count >= MAX_GROUPS {
            return None;
        }
        let color = (count as usize % PALETTE) as i64;
        self.conn
            .execute("INSERT INTO act_groups (name, color, builtin) VALUES (?1, ?2, 0)", params![name, color])
            .ok()?;
        Some(self.conn.last_insert_rowid())
    }

    pub fn act_group_delete(&self, id: i64) {
        let deleted =
            self.conn.execute("DELETE FROM act_groups WHERE id = ?1 AND builtin = 0", params![id]).unwrap_or(0);
        if deleted > 0 {
            let _ = self.conn.execute("DELETE FROM act_apps WHERE group_id = ?1", params![id]);
        }
    }

    pub fn act_assignments(&self) -> HashMap<String, (i64, Source)> {
        let Ok(mut stmt) = self.conn.prepare_cached(
            "SELECT app, group_id, by_user FROM act_apps WHERE group_id IN (SELECT id FROM act_groups)",
        ) else {
            return HashMap::new();
        };
        stmt.query_map([], |r| {
            let by_user = r.get::<_, i64>(2)? != 0;
            Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)?, if by_user { Source::User } else { Source::Ai })))
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }

    pub fn act_app_set(&self, app: &str, group: i64, source: Source) {
        let Some(app) = appgroups::clean_app(app) else { return };
        let by_user = i64::from(source == Source::User);
        let _ = self.conn.execute(
            "INSERT INTO act_apps (app, group_id, by_user) VALUES (?1, ?2, ?3)
             ON CONFLICT(app) DO UPDATE SET group_id = excluded.group_id, by_user = excluded.by_user
             WHERE act_apps.by_user = 0 OR excluded.by_user = 1",
            params![app, group, by_user],
        );
        let _ = self.conn.execute(
            "DELETE FROM act_apps WHERE app NOT IN (SELECT app FROM act_apps ORDER BY by_user DESC LIMIT ?1)",
            params![MAX_ASSIGNED],
        );
    }

    pub fn activity_days(&self, first_day: i64) -> Vec<(i64, String, i64)> {
        let Ok(mut stmt) =
            self.conn.prepare_cached("SELECT day, app, secs FROM activity WHERE day >= ?1 ORDER BY day LIMIT 5000")
        else {
            return Vec::new();
        };
        stmt.query_map(params![first_day], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }

    pub fn ai_sort_used(&self, day: i64) -> u32 {
        self.conn
            .query_row("SELECT val FROM kv WHERE key = 'act_ai_day'", [], |r| r.get::<_, String>(0))
            .ok()
            .and_then(|v| v.split_once(':').map(|(d, n)| (d.parse::<i64>().ok(), n.parse::<u32>().ok())))
            .and_then(|(d, n)| (d? == day).then_some(n?))
            .unwrap_or(0)
    }

    pub fn ai_sort_count(&self, day: i64) {
        let used = self.ai_sort_used(day) + 1;
        let _ = self.conn.execute(
            "INSERT INTO kv (key, val) VALUES ('act_ai_day', ?1) ON CONFLICT(key) DO UPDATE SET val = excluded.val",
            params![format!("{day}:{used}")],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_are_grouped_by_choice_then_table_then_other() {
        let store = Store::in_memory().unwrap();
        let groups = store.act_groups();
        let mut assigned = HashMap::new();
        let id = |name: &str| groups.iter().find(|g| g.name == name).unwrap().id;
        assert_eq!(group_of("Code", &assigned, &groups), id("Development"));
        assert_eq!(group_of("hades", &assigned, &groups), appgroups::OTHER_ID);
        assigned.insert("hades".to_string(), (id("Games"), Source::Ai));
        assigned.insert("code".to_string(), (id("Work"), Source::User));
        assert_eq!(group_of("Hades", &assigned, &groups), id("Games"));
        assert_eq!(group_of("Code", &assigned, &groups), id("Work"), "a choice beats the table");
        assigned.insert("ghost".to_string(), (999, Source::User));
        assert_eq!(group_of("ghost", &assigned, &groups), appgroups::OTHER_ID, "a deleted group is forgotten");
    }

    #[test]
    fn totals_add_up_per_group_and_sort_by_time() {
        let store = Store::in_memory().unwrap();
        let groups = store.act_groups();
        let rows = vec![
            ("chrome".to_string(), 100),
            ("code".to_string(), 300),
            ("rider".to_string(), 200),
            ("hades".to_string(), 50),
            ("idle".to_string(), 0),
        ];
        let t = totals(&rows, &HashMap::new(), &groups);
        assert_eq!(
            t.iter().map(|g| (g.name.as_str(), g.secs)).collect::<Vec<_>>(),
            [("Development", 500), ("Browsing", 100), ("Other", 50)]
        );
        assert_eq!(t[0].apps, [("code".to_string(), 300), ("rider".to_string(), 200)]);
    }

    #[test]
    fn the_week_is_seven_stacks_oldest_first() {
        let store = Store::in_memory().unwrap();
        let groups = store.act_groups();
        let rows = vec![
            (100, "code".to_string(), 60),
            (100, "rider".to_string(), 40),
            (94, "code".to_string(), 10),
            (93, "code".to_string(), 99),
            (98, "chrome".to_string(), 30),
        ];
        let days = stack_days(&rows, 100, &HashMap::new(), &groups);
        assert_eq!(days.len(), 7);
        assert_eq!(days[6], [(0, 100)], "today: development");
        assert_eq!(days[0], [(0, 10)]);
        assert_eq!(days[4], [(2, 30)]);
        assert!(days[1].is_empty() && days[2].is_empty());
    }

    #[test]
    fn the_built_in_groups_exist_once() {
        let store = Store::in_memory().unwrap();
        let groups = store.act_groups();
        assert_eq!(groups.len(), BUILTIN.len());
        assert!(groups.iter().all(|g| g.builtin));
        assert_eq!(groups.last().map(|g| (g.id, g.name.as_str())), Some((appgroups::OTHER_ID, "Other")));
        init(&store.conn).unwrap();
        assert_eq!(store.act_groups().len(), BUILTIN.len());
    }

    #[test]
    fn own_groups_are_added_limited_and_deleted_with_their_programs() {
        let store = Store::in_memory().unwrap();
        let id = store.act_group_add("  Side   project ").unwrap();
        assert!(store.act_group_add("side project").is_none(), "names are unique without regard to case");
        assert!(store.act_group_add("  ").is_none());
        store.act_app_set("hades", id, Source::User);
        assert_eq!(store.act_assignments()["hades"], (id, Source::User));
        store.act_group_delete(1);
        assert_eq!(store.act_groups().len(), BUILTIN.len() + 1);
        store.act_group_delete(id);
        assert!(store.act_assignments().is_empty(), "its programs lose the assignment");
        while store.act_group_add(&format!("g{}", store.act_groups().len())).is_some() {}
        assert_eq!(store.act_groups().len() as i64, MAX_GROUPS);
    }

    #[test]
    fn the_users_choice_beats_the_ai() {
        let store = Store::in_memory().unwrap();
        store.act_app_set("Blender", 6, Source::Ai);
        assert_eq!(store.act_assignments()["blender"], (6, Source::Ai));
        store.act_app_set("blender", 4, Source::User);
        store.act_app_set("blender", 1, Source::Ai);
        assert_eq!(store.act_assignments()["blender"], (4, Source::User), "the AI cannot change it");
        store.act_app_set("blender", 5, Source::User);
        assert_eq!(store.act_assignments()["blender"], (5, Source::User));
        store.act_app_set("bad\"name", 1, Source::User);
        assert_eq!(store.act_assignments().len(), 1);
    }

    #[test]
    fn days_are_listed_per_program() {
        let store = Store::in_memory().unwrap();
        store.activity_add(100, "code", 60);
        store.activity_add(101, "code", 30);
        store.activity_add(101, "chrome", 5);
        let rows = store.activity_days(101);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|(d, _, _)| *d == 101));
    }

    #[test]
    fn the_daily_ai_counter_starts_over_each_day() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.ai_sort_used(500), 0);
        store.ai_sort_count(500);
        store.ai_sort_count(500);
        assert_eq!(store.ai_sort_used(500), 2);
        assert_eq!(store.ai_sort_used(501), 0, "another day, another count");
        store.ai_sort_count(501);
        assert_eq!(store.ai_sort_used(501), 1);
        assert_eq!(store.ai_sort_used(500), 0);
    }
}
