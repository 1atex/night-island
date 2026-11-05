use windows::Win32::UI::Shell::{ASSOCF_NONE, ASSOCSTR_FRIENDLYAPPNAME, AssocQueryStringW};
use windows::core::{PCWSTR, PWSTR, w};

use crate::platform::{apps, steam};

pub const PROGRAMS_BUDGET: usize = 2000;
pub const GAMES_BUDGET: usize = 500;

pub fn default_browser() -> Option<String> {
    [w!("https"), w!(".html")].into_iter().find_map(|assoc| {
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let asked = unsafe {
            AssocQueryStringW(
                ASSOCF_NONE,
                ASSOCSTR_FRIENDLYAPPNAME,
                assoc,
                PCWSTR::null(),
                Some(PWSTR(buf.as_mut_ptr())),
                &mut len,
            )
        };
        if asked.is_err() {
            return None;
        }
        let name = String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1)]);
        let name = name.trim().to_string();
        (!name.is_empty()).then_some(name)
    })
}

pub fn list_within<'a>(names: impl IntoIterator<Item = &'a str>, budget: usize) -> String {
    let mut out = String::new();
    let mut left = 0usize;
    for name in names {
        let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
        let cost = name.chars().count() + 2;
        if name.is_empty() || left + cost > budget {
            continue;
        }
        if !out.is_empty() {
            out.push_str("; ");
        }
        out.push_str(&name);
        left += cost;
    }
    out
}

pub fn facts() -> String {
    let mut out = String::new();
    if let Some(browser) = default_browser() {
        out.push_str(&format!("Default browser: {browser}\n"));
    }
    let programs = list_within(apps::names(), PROGRAMS_BUDGET);
    if !programs.is_empty() {
        out.push_str(&format!("Installed programs (Start menu): {programs}\n"));
    }
    let games = steam::installed();
    let games = list_within(games.iter().map(|g| g.name.as_str()), GAMES_BUDGET);
    if !games.is_empty() {
        out.push_str(&format!("Installed Steam games: {games}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_stops_at_its_budget_and_skips_blanks() {
        let names = ["Telegram", "", "  Night   gram ", "Notepad"];
        assert_eq!(list_within(names, 100), "Telegram; Night gram; Notepad");
        let cut = list_within(names, 22);
        assert_eq!(cut, "Telegram; Night gram");
        assert!(list_within(["x".repeat(50).as_str()], 10).is_empty());
        assert!(list_within(std::iter::empty::<&str>(), 10).is_empty());
    }

    #[test]
    #[ignore = "reads this computer's default browser"]
    fn the_default_browser_has_a_name() {
        let name = default_browser().expect("a default browser");
        println!("{name}");
        assert!(!name.is_empty());
    }
}
