use serde_json::Value;

pub type Rgb = (u8, u8, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Palette {
    pub average: Option<Rgb>,
    pub mini_player: Option<Rgb>,
    pub accent: Option<Rgb>,
    pub wave_text: Option<Rgb>,
}

impl Palette {
    pub fn from_track(track: &Value) -> Self {
        let read = |key: &str| {
            colour(&track["derivedColors"][key]).or_else(|| {
                track["albums"].as_array().into_iter().flatten().find_map(|album| colour(&album["derivedColors"][key]))
            })
        };
        Self {
            average: read("average"),
            mini_player: read("miniPlayer"),
            accent: read("accent"),
            wave_text: read("waveText"),
        }
    }

    pub fn with_cover(self, rgb: Rgb) -> Self {
        Self {
            average: self.average.or(Some(rgb)),
            mini_player: self.mini_player.or(Some(rgb)),
            accent: self.accent.or(Some(rgb)),
            ..self
        }
    }
}

fn colour(value: &Value) -> Option<Rgb> {
    let value = value.as_str()?.strip_prefix('#').unwrap_or(value.as_str()?);
    if value.len() != 6 || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(value, 16).ok()?;
    Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn server_palette_wins_over_album_and_cover_per_component() {
        let p = Palette::from_track(&json!({"derivedColors":{"accent":"#fF8020","average":"transparent"},"albums":[{"derivedColors":{"accent":"#000000","average":"#123456","miniPlayer":"abcdef","waveText":"#ffffff"}}]})).with_cover((1,2,3));
        assert_eq!(p.accent, Some((255, 128, 32)));
        assert_eq!(p.average, Some((18, 52, 86)));
        assert_eq!(p.mini_player, Some((171, 205, 239)));
        assert_eq!(p.wave_text, Some((255, 255, 255)));
    }
    #[test]
    fn malformed_colours_fall_back_without_panics() {
        for s in ["#fff", "#abcdabcd", "#12你好", "rgba(0,0,0,1)", "#zzzzzz", ""] {
            assert_eq!(colour(&json!(s)), None);
        }
        assert_eq!(Palette::from_track(&json!({})).with_cover((4, 5, 6)).accent, Some((4, 5, 6)));
    }
}
