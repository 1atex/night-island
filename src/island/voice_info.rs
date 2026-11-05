use crate::island::*;
use crate::platform::{media, steam, volume};
use crate::services::{discord, spotify, weather};

const DAY_NAMES: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MAX_NOTE_CHARS: usize = 1500;
const MAX_NOTE_LIST: usize = 25;

pub fn weekday_label(day: u32, today: u32) -> String {
    let name = DAY_NAMES[(day % 7) as usize];
    match (day + 7 - today % 7) % 7 {
        0 => format!("today ({name})"),
        1 => format!("tomorrow ({name})"),
        _ => name.to_string(),
    }
}

pub fn weather_text(w: &weather::Weather, today: u32) -> String {
    let mut out = format!(
        "Weather in {}: now {:.0} C, {}, wind {:.0} m/s, humidity {:.0}%.\n",
        w.place,
        w.temp,
        w.sky.name(),
        w.wind,
        w.humidity
    );
    if !w.hourly.is_empty() {
        let hours: Vec<String> =
            w.hourly.iter().take(8).map(|h| format!("{:02}:00 {:.0} C {}", h.hour, h.temp, h.sky.name())).collect();
        out.push_str(&format!("Next hours: {}.\n", hours.join("; ")));
    }
    if !w.daily.is_empty() {
        let days: Vec<String> = w
            .daily
            .iter()
            .map(|d| format!("{}: {:.0}..{:.0} C {}", weekday_label(d.weekday, today), d.min, d.max, d.sky.name()))
            .collect();
        out.push_str(&format!("Days: {}.\n", days.join("; ")));
    }
    out
}

pub fn music_text(m: &media::Media, position: Option<(f32, f32)>) -> String {
    if m.title.is_empty() && m.artist.is_empty() {
        return "Nothing is playing in a media player.\n".into();
    }
    let mut out = format!(
        "{} {} - {} (player: {}).\n",
        if m.playing { "Playing:" } else { "Paused:" },
        m.artist,
        m.title,
        if m.source.is_empty() { "unknown" } else { &m.source }
    );
    if let Some((at, len)) = position.filter(|(_, len)| *len > 0.0) {
        out.push_str(&format!("Position {:.0}s of {:.0}s.\n", at, len));
    }
    out
}

pub fn spotify_text(s: &spotify::State) -> String {
    let status = match &s.status {
        spotify::Status::Connected => "connected".to_string(),
        spotify::Status::NoClientId => "not set up".to_string(),
        spotify::Status::Disconnected => "not connected".to_string(),
        spotify::Status::Connecting => "connecting".to_string(),
        spotify::Status::Error(why) => format!("error: {why}"),
    };
    format!(
        "Spotify: {status}; shuffle {}; repeat {:?}; current track liked: {}.\n",
        if s.shuffle { "on" } else { "off" },
        s.repeat,
        if s.liked { "yes" } else { "no" }
    )
}

pub fn discord_text(s: &discord::State) -> String {
    if s.channel.is_empty() {
        return format!("Discord: {:?}, not in a voice channel.\n", s.status);
    }
    let members: Vec<&str> = s.members.iter().take(12).map(|m| m.name.as_str()).collect();
    format!(
        "Discord: in the voice channel {} with {}; your microphone is {}, headphones {}.\n",
        s.channel,
        if members.is_empty() { "nobody else".to_string() } else { members.join(", ") },
        if s.self_mute { "muted" } else { "on" },
        if s.self_deaf { "off (deafened)" } else { "on" }
    )
}

impl App {
    fn system_text(&self) -> String {
        let mut out = String::new();
        if let Some(level) = volume::get() {
            out.push_str(&format!("Volume {:.0}%.\n", level * 100.0));
        }
        if let Some(name) = volume::device_name() {
            out.push_str(&format!("Output device: {name}.\n"));
        }
        if let Some(muted) = volume::mic_muted() {
            out.push_str(&format!("Microphone is {}.\n", if muted { "muted" } else { "on" }));
        }
        match crate::platform::metrics::latest() {
            Some(s) => out.push_str(&format!(
                "CPU {:.0}%, RAM {:.0}%, disk {:.0}%{}.\n",
                s.cpu,
                s.ram,
                s.disk,
                s.gpu.map_or(String::new(), |g| format!(", GPU {g:.0}%"))
            )),
            None => out.push_str("Computer load is not being measured now; use a command for it.\n"),
        }
        out
    }

    fn steam_text(&self) -> String {
        match steam::running_app() {
            Some(id) => format!("Steam game running: {}.\n", steam::name_of(id).unwrap_or_else(|| id.to_string())),
            None => "No Steam game is running.\n".into(),
        }
    }

    fn reminders_text(&self) -> String {
        if self.reminders.is_empty() {
            return "No reminders are waiting.\n".into();
        }
        let items: Vec<String> = self
            .reminders
            .iter()
            .take(12)
            .map(|r| {
                let when = chrono::DateTime::from_timestamp(r.due, 0)
                    .map(|t| t.with_timezone(&chrono::Local).format("%d.%m %H:%M").to_string())
                    .unwrap_or_default();
                format!("{when} {}", r.text)
            })
            .collect();
        format!("Reminders: {}.\n", items.join("; "))
    }

    fn notes_text(&self, name: &str) -> String {
        let Some(store) = &self.store else { return "Notes are not available.\n".into() };
        let items = store.note_items();
        let wanted = name.trim().to_lowercase();
        if wanted.is_empty() {
            let titles: Vec<String> =
                items.iter().filter(|i| !i.folder).take(MAX_NOTE_LIST).map(|i| i.name.clone()).collect();
            return if titles.is_empty() {
                "There are no notes.\n".into()
            } else {
                format!("Notes: {}.\n", titles.join("; "))
            };
        }
        match items.iter().find(|i| !i.folder && i.name.to_lowercase().contains(&wanted)) {
            Some(item) => {
                let text: String = store.note_text(item.id).chars().take(MAX_NOTE_CHARS).collect();
                format!("Note {}:\n{text}\n", item.name)
            }
            None => format!("No note is called like {name}.\n"),
        }
    }

    pub(in crate::island) fn write_note(
        &mut self,
        name: &str,
        text: &str,
        append: bool,
    ) -> std::result::Result<String, String> {
        let Some(store) = &self.store else { return Err("Notes are not available".into()) };
        let title = crate::data::notes::clean_name(name);
        let wanted = title.to_lowercase();
        let items = store.note_items();
        let found = items.iter().find(|i| !i.folder && i.name.to_lowercase() == wanted).map(|i| i.id);
        let (id, created) = match found {
            Some(id) => (id, false),
            None => (store.note_add(None, false, &title).ok_or("There are too many notes")?, true),
        };
        let body = if append && !created {
            let old = store.note_text(id);
            if old.is_empty() { text.to_string() } else { format!("{}\n{text}", old.trim_end()) }
        } else {
            text.to_string()
        };
        store.note_set_text(id, &body);
        Ok(format!("{} the note {title}", if created { "Created" } else { "Updated" }))
    }

    pub(in crate::island) fn island_info(&self, what: &str, name: &str) -> std::result::Result<String, String> {
        let today = chrono::Datelike::weekday(&chrono::Local::now()).num_days_from_monday();
        let weather = || match weather::snapshot() {
            Some(w) => weather_text(&w, today),
            None => "Weather is not available: it is off or still loading.\n".into(),
        };
        let music = || music_text(&media::snapshot(), media::progress()) + &spotify_text(&spotify::state());
        let text = match what {
            "weather" => weather(),
            "music" | "spotify" => music(),
            "system" => self.system_text(),
            "discord" => discord_text(&discord::state()),
            "steam" => self.steam_text(),
            "reminders" => self.reminders_text(),
            "notes" => self.notes_text(name),
            "all" | "overview" => format!(
                "{}{}{}{}{}{}",
                weather(),
                music(),
                self.system_text(),
                discord_text(&discord::state()),
                self.steam_text(),
                self.reminders_text()
            ),
            _ => return Err("what is one of weather, music, system, discord, steam, reminders, notes, all".into()),
        };
        Ok(text.trim_end().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::weather::{Day, Hour, Sky, Weather};

    fn sample() -> Weather {
        Weather {
            place: "Ivanovo".into(),
            temp: 3.4,
            wind: 5.2,
            humidity: 81.0,
            sky: Sky::from_symbol("cloudy").0,
            night: false,
            hourly: vec![Hour { hour: 14, temp: 4.0, sky: Sky::from_symbol("cloudy").0, night: false }],
            daily: vec![
                Day { weekday: 3, min: 1.0, max: 5.0, sky: Sky::from_symbol("cloudy").0 },
                Day { weekday: 4, min: -2.0, max: 2.0, sky: Sky::from_symbol("snow").0 },
                Day { weekday: 5, min: 0.0, max: 3.0, sky: Sky::from_symbol("cloudy").0 },
            ],
            city_missing: false,
        }
    }

    #[test]
    fn the_days_are_named_relative_to_today() {
        assert_eq!(weekday_label(3, 3), "today (Thu)");
        assert_eq!(weekday_label(4, 3), "tomorrow (Fri)");
        assert_eq!(weekday_label(5, 3), "Sat");
        assert_eq!(weekday_label(0, 6), "tomorrow (Mon)");
        assert_eq!(weekday_label(13, 3), "Sun", "a wrong day number does not panic");
    }

    #[test]
    fn the_weather_text_has_tomorrow_and_the_place() {
        let text = weather_text(&sample(), 3);
        assert!(text.contains("Weather in Ivanovo: now 3 C"));
        assert!(text.contains("14:00 4 C"));
        assert!(text.contains("today (Thu): 1..5 C"));
        assert!(text.contains("tomorrow (Fri): -2..2 C"));
        let mut empty = sample();
        empty.hourly.clear();
        empty.daily.clear();
        assert_eq!(weather_text(&empty, 0).lines().count(), 1);
    }

    #[test]
    fn music_and_spotify_are_told_in_plain_words() {
        let m = media::Media {
            title: "Song".into(),
            artist: "Band".into(),
            playing: true,
            accent: None,
            cover: None,
            source: "Spotify".into(),
            players: 1,
        };
        assert_eq!(
            music_text(&m, Some((30.0, 200.0))),
            "Playing: Band - Song (player: Spotify).\nPosition 30s of 200s.\n"
        );
        let idle = media::Media { title: String::new(), artist: String::new(), ..m };
        assert_eq!(music_text(&idle, None), "Nothing is playing in a media player.\n");
        let mut s = spotify::State::default();
        s.status = spotify::Status::Connected;
        s.shuffle = true;
        assert!(spotify_text(&s).starts_with("Spotify: connected; shuffle on;"));
    }
}
