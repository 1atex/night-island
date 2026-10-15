use serde::{Deserialize, Serialize};

pub const ALT: u32 = 0x1;
pub const CTRL: u32 = 0x2;
pub const SHIFT: u32 = 0x4;
pub const WIN: u32 = 0x8;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Combo {
    pub mods: u32,
    pub vk: u32,
}

impl Combo {
    pub const fn new(mods: u32, vk: u32) -> Self {
        Self { mods, vk }
    }

    pub fn valid(self) -> bool {
        self.mods != 0 && self.mods & !(ALT | CTRL | SHIFT | WIN) == 0 && valid_vk(self.vk)
    }

    pub fn text(self) -> String {
        let mut out = String::new();
        for (bit, name) in [(CTRL, "Ctrl"), (ALT, "Alt"), (SHIFT, "Shift"), (WIN, "Win")] {
            if self.mods & bit != 0 {
                out.push_str(name);
                out.push('+');
            }
        }
        out.push_str(&vk_name(self.vk));
        out
    }
}

pub fn valid_vk(vk: u32) -> bool {
    matches!(vk,
        0x09 | 0x0D | 0x20..=0x28 | 0x2D | 0x2E | 0x30..=0x39 | 0x41..=0x5A | 0x60..=0x6F | 0x70..=0x87
        | 0xBA..=0xC0 | 0xDB..=0xDE)
}

fn vk_name(vk: u32) -> String {
    match vk {
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x20 => "Space".into(),
        0x21 => "PageUp".into(),
        0x22 => "PageDown".into(),
        0x23 => "End".into(),
        0x24 => "Home".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0x2D => "Insert".into(),
        0x2E => "Delete".into(),
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(vk).map(String::from).unwrap_or_default(),
        0x60..=0x69 => format!("Num{}", vk - 0x60),
        0x6A => "Num*".into(),
        0x6B => "Num+".into(),
        0x6C => "Num,".into(),
        0x6D => "Num-".into(),
        0x6E => "Num.".into(),
        0x6F => "Num/".into(),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0xBA => ";".into(),
        0xBB => "=".into(),
        0xBC => ",".into(),
        0xBD => "-".into(),
        0xBE => ".".into(),
        0xBF => "/".into(),
        0xC0 => "`".into(),
        0xDB => "[".into(),
        0xDC => "\\".into(),
        0xDD => "]".into(),
        0xDE => "'".into(),
        _ => format!("Key {vk:#04X}"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hotkey {
    Toggle = 1,
    Clipboard = 2,
    PlainPaste = 3,
    Command = 4,
    Mic = 5,
    ShotArea = 6,
    ShotFull = 7,
    Picker = 8,
    Ocr = 9,
    Qr = 10,
    Rec = 11,
    HidePill = 12,
}

impl Hotkey {
    pub const ALL: [Hotkey; 12] = [
        Hotkey::Toggle,
        Hotkey::Clipboard,
        Hotkey::PlainPaste,
        Hotkey::Command,
        Hotkey::Mic,
        Hotkey::ShotArea,
        Hotkey::ShotFull,
        Hotkey::Picker,
        Hotkey::Ocr,
        Hotkey::Qr,
        Hotkey::Rec,
        Hotkey::HidePill,
    ];

    pub fn from_id(id: i32) -> Option<Hotkey> {
        Self::ALL.into_iter().find(|h| *h as i32 == id)
    }

    pub fn key(self) -> &'static str {
        match self {
            Hotkey::Toggle => "toggle",
            Hotkey::Clipboard => "clipboard",
            Hotkey::PlainPaste => "plain_paste",
            Hotkey::Command => "command",
            Hotkey::Mic => "mic",
            Hotkey::ShotArea => "shot_area",
            Hotkey::ShotFull => "shot_full",
            Hotkey::Picker => "picker",
            Hotkey::Ocr => "ocr",
            Hotkey::Qr => "qr",
            Hotkey::Rec => "rec",
            Hotkey::HidePill => "hide_pill",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Hotkey::Toggle => "Open or close the island",
            Hotkey::Clipboard => "Clipboard history",
            Hotkey::PlainPaste => "Paste as plain text",
            Hotkey::Command => "Command bar",
            Hotkey::Mic => "Microphone on or off",
            Hotkey::ShotArea => "Screenshot of an area",
            Hotkey::ShotFull => "Screenshot of the screen",
            Hotkey::Picker => "Color picker",
            Hotkey::Ocr => "Text from screen",
            Hotkey::Qr => "QR code",
            Hotkey::Rec => "Screen recording",
            Hotkey::HidePill => "Hide the pill for a few seconds",
        }
    }

    pub fn default_combo(self) -> Combo {
        const I: u32 = 'I' as u32;
        let (mods, vk) = match self {
            Hotkey::Toggle => (CTRL | ALT, I),
            Hotkey::Clipboard => (CTRL | ALT, 'V' as u32),
            Hotkey::PlainPaste => (CTRL | ALT | SHIFT, 'V' as u32),
            Hotkey::Command => (ALT, 0x20),
            Hotkey::Mic => (CTRL | ALT, 'M' as u32),
            Hotkey::ShotArea => (CTRL | ALT, 'S' as u32),
            Hotkey::ShotFull => (CTRL | ALT | SHIFT, 'S' as u32),
            Hotkey::Picker => (CTRL | ALT, 'C' as u32),
            Hotkey::Ocr => (CTRL | ALT, 'O' as u32),
            Hotkey::Qr => (CTRL | ALT, 'Q' as u32),
            Hotkey::Rec => (CTRL | ALT, 'R' as u32),
            Hotkey::HidePill => (CTRL, 'E' as u32),
        };
        Combo::new(mods, vk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_unique() {
        let mut seen = Vec::new();
        for h in Hotkey::ALL {
            let c = h.default_combo();
            assert!(c.valid(), "{h:?}");
            assert!(!seen.contains(&c), "{h:?} repeats a combination");
            seen.push(c);
            assert_eq!(Hotkey::from_id(h as i32), Some(h));
        }
    }

    #[test]
    fn combinations_read_naturally() {
        assert_eq!(Hotkey::Toggle.default_combo().text(), "Ctrl+Alt+I");
        assert_eq!(Hotkey::Command.default_combo().text(), "Alt+Space");
        assert_eq!(Hotkey::HidePill.default_combo().text(), "Ctrl+E");
        assert_eq!(Combo::new(CTRL | SHIFT, 0x74).text(), "Ctrl+Shift+F5");
    }

    #[test]
    fn a_bare_key_or_a_modifier_key_is_refused() {
        assert!(!Combo::new(0, 'E' as u32).valid());
        assert!(!Combo::new(CTRL, 0x11).valid());
        assert!(!Combo::new(CTRL, 0x1B).valid());
        assert!(!Combo::new(0x100, 'E' as u32).valid());
    }
}
