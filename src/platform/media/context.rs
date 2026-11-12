use super::Media;
use crate::data::settings::{EqSources, Settings};
use crate::platform::desktop_dock;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SourceKind {
    #[default]
    Other,
    Browser,
    YoutubeVideo,
    YoutubeMusic,
    Yandex,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Site {
    Youtube,
    YoutubeMusic,
    Yandex,
}

fn site(title: &str) -> Option<Site> {
    let normalized = title.to_lowercase().replace(['—', '–', '•', '|'], "-");
    let mut parts = normalized.rsplit(" - ").map(str::trim);
    let tail = parts.next()?;
    let brand = if [
        "chrome",
        "chromium",
        "firefox",
        "edge",
        "brave",
        "vivaldi",
        "opera",
        "zen",
        "browser",
        "браузер",
        "floorp",
        "waterfox",
    ]
    .iter()
    .any(|name| tail.contains(name))
    {
        let suffix = parts.next().unwrap_or(tail);
        if suffix.starts_with("profile ")
            || suffix.starts_with("профиль ")
            || matches!(suffix, "personal" | "work" | "личный" | "рабочий")
        {
            parts.next().unwrap_or(suffix)
        } else {
            suffix
        }
    } else {
        tail
    };
    match brand {
        "youtube" => Some(Site::Youtube),
        "youtube music" => Some(Site::YoutubeMusic),
        "yandex music" | "яндекс музыка" | "яндекс.музыка" => Some(Site::Yandex),
        _ => None,
    }
}

pub fn youtube_window(title: &str) -> bool {
    matches!(site(title), Some(Site::Youtube | Site::YoutubeMusic))
}

const BROWSERS: &[&str] = &[
    "chrome",
    "chromium",
    "msedge",
    "firefox",
    "zen",
    "brave",
    "vivaldi",
    "opera",
    "browser",
    "floorp",
    "waterfox",
    "librewolf",
];

pub fn browser_exe(path: &str) -> Option<&'static str> {
    let name = path.rsplit(['\\', '/']).next()?.to_ascii_lowercase();
    BROWSERS.iter().copied().find(|candidate| name.strip_suffix(".exe") == Some(candidate))
}

fn matches_browser(id: &str, exe: &str) -> bool {
    id.contains(exe)
        || (exe == "msedge" && id.contains("microsoftedge"))
        || (exe == "browser" && (id == "yandex" || id.contains("yandexbrowser") || id.contains("yandex.browser")))
        || (id.len() == 16
            && id.bytes().all(|c| c.is_ascii_hexdigit())
            && matches!(exe, "firefox" | "zen" | "floorp" | "waterfox" | "librewolf"))
}

fn browser_id(id: &str) -> bool {
    BROWSERS.iter().any(|exe| matches_browser(id, exe))
}

fn music_credit(title: &str, artist: &str) -> bool {
    let artist = artist.trim().to_lowercase();
    let title = title.to_lowercase();
    artist.ends_with(" - topic")
        || artist.ends_with("vevo")
        || ["official music video", "official audio", "official lyric video", "официальный клип", "официальное аудио"]
            .iter()
            .any(|label| title.contains(label))
}

fn includes_recording(window: &str, recording: &str) -> bool {
    window.match_indices(recording).any(|(start, matched)| {
        !window[..start].chars().next_back().is_some_and(char::is_alphanumeric)
            && !window[start + matched.len()..].chars().next().is_some_and(char::is_alphanumeric)
    })
}

fn classify(
    id: &str,
    title: &str,
    artist: &str,
    publisher: Option<Site>,
    windows: &[desktop_dock::Window],
) -> SourceKind {
    if ["ru.yandex.desktop.music", "yandexmusic", "yandex.music", "yandex music", "яндекс музыка", "яндекс.музыка"]
        .iter()
        .any(|app| id.contains(app))
    {
        return SourceKind::Yandex;
    }
    if title.is_empty() || !browser_id(id) {
        return SourceKind::Other;
    }
    let track = title.to_lowercase();
    let matched = windows
        .iter()
        .filter(|win| browser_exe(&win.path).is_some_and(|exe| matches_browser(id, exe)))
        .filter(|win| includes_recording(&win.title.to_lowercase(), &track))
        .filter_map(|win| site(&win.title).map(|brand| (win.active, brand)))
        .max_by_key(|(active, _)| *active)
        .map(|(_, brand)| brand);
    match matched.or(publisher) {
        Some(Site::Yandex) => SourceKind::Yandex,
        Some(Site::YoutubeMusic) => SourceKind::YoutubeMusic,
        Some(Site::Youtube) if music_credit(title, artist) => SourceKind::YoutubeMusic,
        Some(Site::Youtube) => SourceKind::YoutubeVideo,
        None if artist.trim().to_lowercase().ends_with(" - topic") => SourceKind::YoutubeMusic,
        None => SourceKind::Browser,
    }
}

pub(super) fn recording(id: &str, title: &str, artist: &str, subtitle: &str, album: &str) -> (SourceKind, String) {
    let lower = id.to_lowercase();
    let windows = if browser_id(&lower) && !title.is_empty() { desktop_dock::cached_windows() } else { Vec::new() };
    let mut kind = classify(&lower, title, artist, site(subtitle).or_else(|| site(album)), &windows);
    let key = if !id.is_empty() && !title.is_empty() {
        serde_json::to_string(&(id, title, artist)).unwrap_or_default()
    } else {
        String::new()
    };
    if kind == SourceKind::Browser
        && !key.is_empty()
        && let Ok(snapshot) = super::SNAPSHOT.lock()
        && let Some(old) = snapshot.as_ref().filter(|s| s.media.recording_key == key)
    {
        kind = old.media.context;
    }
    (kind, key)
}

impl Media {
    pub fn equalizer(&self, settings: &Settings) -> bool {
        match settings.eq_sources {
            EqSources::All => true,
            EqSources::Yandex => self.context == SourceKind::Yandex,
            EqSources::Music => settings
                .eq_recordings
                .get(&self.recording_key)
                .copied()
                .unwrap_or(matches!(self.context, SourceKind::Yandex | SourceKind::YoutubeMusic)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(path: &str, title: &str) -> desktop_dock::Window {
        desktop_dock::Window {
            handle: 1,
            path: path.into(),
            title: title.into(),
            active: true,
            minimized: false,
            island_settings: false,
        }
    }

    #[test]
    fn fullscreen_brand_is_separate_from_a_video_or_article_title() {
        for name in [
            "Track - YouTube — Zen Browser",
            "Live - YouTube - Google Chrome",
            "Song - YouTube Music",
            "Song - YouTube - Profile 1 - Microsoft Edge",
            "YouTube",
        ] {
            assert!(youtube_window(name), "{name}");
        }
        for name in
            ["YouTube обзор - Документы - Google Chrome", "How YouTube works — Mozilla Firefox", "Reports - Other site"]
        {
            assert!(!youtube_window(name), "{name}");
        }
    }

    #[test]
    fn browser_music_requires_evidence_from_the_playing_recording() {
        use SourceKind::*;
        let chrome = window("chrome.exe", "Night Drive - YouTube - Google Chrome");
        let zen = window("zen.exe", "Night Drive - YouTube Music — Zen Browser");
        let windows = [chrome, zen];
        assert_eq!(classify("chrome", "Night Drive", "Film", None, &windows), YoutubeVideo);
        assert_eq!(classify("chrome", "Night Drive", "Band - Topic", None, &windows), YoutubeMusic);
        assert_eq!(classify("308046b0af4a39cb", "Night Drive", "Band", None, &windows), YoutubeMusic);
        assert_eq!(classify("chrome", "Different tab", "Film", None, &windows), Browser);
        assert_eq!(classify("chrome", "Song", "Band - Topic", None, &[]), YoutubeMusic);
        assert_eq!(classify("chrome", "Track", "Band", Some(Site::YoutubeMusic), &[]), YoutubeMusic);
        assert_eq!(
            classify("chrome", "Night", "Film", None, &[window("chrome.exe", "Midnight - YouTube Music")]),
            Browser
        );
        assert_eq!(classify("vlc.exe", "Night Drive", "Band", None, &windows), Other);
        assert_eq!(classify("yandexmusic.exe", "Track", "Band", None, &[]), Yandex);
    }

    #[test]
    fn recording_overrides_are_specific_and_strict_source_modes_stay_strict() {
        let mut cfg = Settings::default();
        let mut media =
            Media { recording_key: "browser/song".into(), context: SourceKind::YoutubeVideo, ..Default::default() };
        assert!(!media.equalizer(&cfg));
        cfg.eq_recordings.insert(media.recording_key.clone(), true);
        assert!(media.equalizer(&cfg));
        media.recording_key = "browser/film".into();
        assert!(!media.equalizer(&cfg));
        media.context = SourceKind::YoutubeMusic;
        assert!(media.equalizer(&cfg));
        cfg.eq_recordings.insert(media.recording_key.clone(), false);
        assert!(!media.equalizer(&cfg));
        cfg.eq_sources = EqSources::All;
        assert!(media.equalizer(&cfg));
        cfg.eq_sources = EqSources::Yandex;
        assert!(!media.equalizer(&cfg));
        media.context = SourceKind::Yandex;
        assert!(media.equalizer(&cfg));
        let restored: Settings = serde_json::from_slice(&serde_json::to_vec(&cfg).unwrap()).unwrap();
        assert_eq!(restored.eq_sources, cfg.eq_sources);
        assert_eq!(restored.eq_recordings, cfg.eq_recordings);
    }
}
