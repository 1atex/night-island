use crate::island::*;
use crate::services::lyrics;

const LEAD_MS: u32 = 250;
const SLIDE_MS: u32 = 180;
const MAX_WAIT_MS: u32 = 1000;
const MIN_WAIT_MS: u32 = 120;

impl App {
    pub(in crate::island) fn update_lyrics(&mut self, visible: bool) {
        let m = &self.ui.media;
        let allowed = self.settings.lyrics_app_allowed(&m.source);
        let wanted = self.settings.music_lyrics && allowed && m.playing && !m.title.is_empty();
        if !wanted {
            if m.playing && !m.title.is_empty() && self.ui.lyric.key.is_none() {
                crate::log_info!(
                    "lyrics skipped: setting={} player={:?} allowed={allowed}",
                    self.settings.music_lyrics,
                    m.source
                );
            }
            self.kill_timer(Timer::Lyric);
            if !self.settings.music_lyrics || !allowed || m.title.is_empty() {
                self.ui.lyric = view::LyricUi::default();
            }
            return;
        }
        let key = (m.title.clone(), m.artist.clone());
        if self.ui.lyric.key.as_ref() != Some(&key) {
            self.ui.lyric = view::LyricUi { key: Some(key.clone()), ..Default::default() };
            crate::log_info!("now playing: title={:?} artist={:?} player={:?}", key.0, key.1, self.ui.media.source);
            match lyrics::cached(&key.0, &key.1) {
                Some(Some(lines)) => {
                    crate::log_info!("lyrics: memory cache hit ({} lines)", lines.len());
                    self.set_lyric_lines(lines)
                }
                Some(None) => crate::log_info!("lyrics: known miss in the memory cache"),
                None => {
                    let secs = media::progress().map(|(_, len)| len.round() as u32).filter(|s| *s > 0);
                    lyrics::request(self.hwnd, &key.0, &key.1, secs, self.settings.shared_lyrics);
                }
            }
        }
        if visible && self.ui.lyric.lines.is_some() {
            self.sync_lyric_line();
            self.set_timer(Timer::Lyric, self.frame_ms());
        } else {
            self.kill_timer(Timer::Lyric);
        }
    }

    fn set_lyric_lines(&mut self, lines: lyrics::Lines) {
        let mut lens: Vec<usize> = lines.iter().map(|l| l.text.chars().count()).collect();
        lens.sort_unstable();
        let typical = lens.get(lens.len() * 9 / 10).copied().unwrap_or(0);
        self.ui.lyric.width = (typical as f32 * 7.4 + 6.0).clamp(0.0, view::LYRIC_SLOT_MAX);
        self.ui.lyric.lines = Some(lines);
        self.ui.lyric.idx = None;
    }

    pub(in crate::island) fn on_lyrics(&mut self) {
        let Some((title, artist)) = self.ui.lyric.key.clone() else { return };
        if !self.settings.music_lyrics
            || !self.settings.lyrics_app_allowed(&self.ui.media.source)
            || self.ui.lyric.lines.is_some()
        {
            return;
        }
        let Some(Some(lines)) = lyrics::cached(&title, &artist) else {
            crate::log_info!("lyrics: lookup finished, nothing to show for this track");
            return;
        };
        crate::log_info!("lyrics: showing {} lines", lines.len());
        self.set_lyric_lines(lines);
        self.update_eq();
        self.repaint_collapsed();
    }

    pub(in crate::island) fn lyric_tick(&mut self) {
        if self.ui.lyric.lines.is_none() || !self.settings.music_lyrics {
            return;
        }
        let changed = self.sync_lyric_line();
        let animating = self.ui.lyric.slide > 0.0;
        if changed || animating {
            self.repaint_collapsed();
        }
        let wait = if animating { self.frame_ms() } else { self.ms_to_next_lyric() };
        self.set_timer(Timer::Lyric, wait);
    }

    fn sync_lyric_line(&mut self) -> bool {
        let Some(lines) = self.ui.lyric.lines.clone() else { return false };
        let ms = self.lyric_position_ms();
        let idx = lyrics::line_at(&lines, ms);
        let before = self.ui.lyric.idx;
        if idx != before || self.ui.lyric.shown.iter().all(String::is_empty) {
            let first = before.is_none() && self.ui.lyric.shown.iter().all(String::is_empty);
            self.ui.lyric.idx = idx;
            for (slot, shown) in self.ui.lyric.shown.iter_mut().enumerate() {
                let at = idx.map_or(slot as isize - 3, |i| i as isize + slot as isize - 2);
                *shown =
                    usize::try_from(at).ok().and_then(|i| lines.get(i)).map(|l| l.text.clone()).unwrap_or_default();
            }
            let step = matches!((before, idx), (Some(a), Some(b)) if b == a + 1)
                || (before.is_none() && idx == Some(0) && !first);
            self.ui.lyric.slide_start = step.then(Instant::now);
            self.update_lyric_slide();
            return true;
        }
        self.update_lyric_slide()
    }

    fn update_lyric_slide(&mut self) -> bool {
        let Some(start) = self.ui.lyric.slide_start else {
            self.ui.lyric.slide = 0.0;
            return false;
        };
        let t = start.elapsed().as_millis() as f32 / SLIDE_MS as f32;
        if t >= 1.0 {
            self.ui.lyric.slide_start = None;
            self.ui.lyric.slide = 0.0;
        } else {
            self.ui.lyric.slide = (1.0 - t).powi(3);
        }
        true
    }

    fn lyric_position_ms(&self) -> u32 {
        media::progress().map_or(0, |(pos, _)| (pos * 1000.0) as u32).saturating_add(LEAD_MS)
    }

    fn ms_to_next_lyric(&self) -> u32 {
        let (Some(lines), now) = (&self.ui.lyric.lines, self.lyric_position_ms()) else { return MAX_WAIT_MS };
        let next = self.ui.lyric.idx.map_or(0, |i| i + 1);
        lines.get(next).map_or(MAX_WAIT_MS, |l| l.ms.saturating_sub(now).clamp(MIN_WAIT_MS, MAX_WAIT_MS))
    }
}
