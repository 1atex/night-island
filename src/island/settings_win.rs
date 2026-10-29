use std::ffi::c_void;
use std::sync::Once;

use windows::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, BitBlt, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, EndPaint, GetDC, PAINTSTRUCT, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, SetFocus};

use crate::island::*;

const MAX_SEARCH_CHARS: usize = 40;
const SCROLL_STEP: f32 = 64.0;

impl App {
    pub(in crate::island) fn open_settings_at(&mut self, group: usize) {
        self.open_settings_window();
        if self.settings_win.is_some() {
            self.ui.sw.group = group;
            self.ui.sw.scroll = 0.0;
            self.ui.sw.dropdown = None;
            self.set_search("");
            self.render_settings_win();
        }
    }

    pub(in crate::island) fn open_settings_window(&mut self) {
        if let Some(hwnd) = self.settings_win {
            unsafe {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
            return;
        }
        static CLASS: Once = Once::new();
        let dpi = self.dpi_scale;
        let (cw, ch) = ((view::SW_SIZE.0 * dpi).round() as i32, (view::SW_SIZE.1 * dpi).round() as i32);
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        unsafe {
            let Ok(instance) = GetModuleHandleW(None) else { return };
            CLASS.call_once(|| {
                let class = WNDCLASSW {
                    lpfnWndProc: Some(sw_proc),
                    hInstance: instance.into(),
                    lpszClassName: w!("NightIsland.Settings"),
                    hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                    ..Default::default()
                };
                RegisterClassW(&class);
            });
            let (ow, oh) = (cw, ch);
            let (l, t, r, b) = Self::work_area();
            let x = (l + ((r - l) - ow as f32) / 2.0).max(l) as i32;
            let y = (t + ((b - t) - oh as f32) / 2.0).max(t) as i32;
            let Ok(hwnd) = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("NightIsland.Settings"),
                w!("Night Island"),
                style,
                x,
                y,
                ow,
                oh,
                None,
                None,
                Some(instance.into()),
                None,
            ) else {
                crate::log_error!("could not create the settings window");
                return;
            };
            let round: i32 = 2;
            let _ = DwmSetWindowAttribute(
                hwnd,
                windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(33),
                (&raw const round).cast::<c_void>(),
                4,
            );
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            self.settings_win = Some(hwnd);
            self.ui.sw = view::SwUi::default();
            self.ui.int.spotify_id.set_text(&spotify::own_client_id());
            self.sync_voice_fields();
            self.reload_groups();
            spotify::reload_status();
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(Some(hwnd));
        }
        self.render_settings_win();
    }

    pub(in crate::island) fn sw_title_click(&self, x: i32, y: i32) -> u8 {
        match view::sw_hit(self.sw_size(), self.dpi_scale, &self.ui, x as f32, y as f32) {
            Some(view::SwHit::Minimize) => 1,
            Some(view::SwHit::Close) => 2,
            _ => 0,
        }
    }

    pub(in crate::island) fn sw_nc_zone(&self, x: i32, y: i32) -> Option<view::TitleZone> {
        view::sw_title_zone(self.sw_size(), self.dpi_scale, x as f32, y as f32)
    }

    fn sw_client(&self) -> Option<(i32, i32)> {
        let hwnd = self.settings_win?;
        let mut rc = windows::Win32::Foundation::RECT::default();
        unsafe { GetClientRect(hwnd, &mut rc).ok()? };
        Some((rc.right, rc.bottom))
    }

    fn sw_size(&self) -> (f32, f32) {
        self.sw_client().map_or(view::SW_SIZE, |(w, h)| (w as f32, h as f32))
    }

    pub(in crate::island) fn render_settings_win(&self) {
        let (Some(hwnd), Some((w, h))) = (self.settings_win, self.sw_client()) else { return };
        if w <= 0 || h <= 0 {
            return;
        }
        unsafe {
            let screen = GetDC(Some(hwnd));
            let mem = CreateCompatibleDC(Some(screen));
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            if let Ok(dib) = CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
                let old = SelectObject(mem, dib.into());
                let painted = self.gpu.paint_dc(mem, (w, h), |p| {
                    let _ = view::paint_settings_window(p, (w as f32, h as f32), self.dpi_scale, &self.ui);
                });
                if painted.is_ok() {
                    let _ = BitBlt(screen, 0, 0, w, h, Some(mem), 0, 0, SRCCOPY);
                }
                SelectObject(mem, old);
                let _ = DeleteObject(dib.into());
            }
            let _ = DeleteDC(mem);
            ReleaseDC(Some(hwnd), screen);
        }
    }

    pub(in crate::island) fn on_sw_move(&mut self, x: i32, y: i32, left_down: bool) {
        if left_down && self.ui.sw.drag.is_some() {
            return self.sw_drag(x);
        }
        if left_down && self.ui.sw.field == Some(view::Field::Search) && self.ui.sw.search_press {
            self.sw_click_field(view::Field::Search, x, true);
            return self.render_settings_win();
        }
        let hit = view::sw_hit(self.sw_size(), self.dpi_scale, &self.ui, x as f32, y as f32);
        if hit != self.ui.sw.hover {
            self.ui.sw.hover = hit;
            self.render_settings_win();
        }
    }

    pub(in crate::island) fn on_sw_down(&mut self, x: i32, y: i32) {
        let hit = view::sw_hit(self.sw_size(), self.dpi_scale, &self.ui, x as f32, y as f32);
        if let Some(id) = self.ui.sw.dropdown.take() {
            if let Some(view::SwHit::Pick(_, k)) = hit {
                view::cycle_pick(id, &mut self.settings, k);
                self.setting_changed(id, true);
            }
            return self.render_settings_win();
        }
        self.ui.sw.quiet_sys = false;
        let waiting = self.ui.sw.capturing;
        if waiting.is_some() {
            self.end_rebind();
            if hit == waiting.map(|h| view::SwHit::Act(view::Act::Rebind(h))) {
                return self.render_settings_win();
            }
        }
        if let Some(view::SwHit::Field(field)) = hit {
            self.sw_focus(Some(field));
            let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
            self.sw_click_field(field, x, shift);
            if field == view::Field::Search {
                self.ui.sw.search_press = true;
                unsafe {
                    let _ = SetCapture(self.settings_win.unwrap_or(self.hwnd));
                }
            }
            return self.render_settings_win();
        }
        self.sw_focus(None);
        match hit {
            Some(view::SwHit::Group(i)) => {
                self.ui.sw.group = i;
                self.ui.sw.scroll = 0.0;
                self.set_search("");
            }
            Some(view::SwHit::Row(id, hit)) => self.sw_row(id, hit, x),
            Some(view::SwHit::Act(act)) => self.sw_act(act),
            Some(view::SwHit::Theme(hit)) => {
                match hit {
                    view::ThemeHit::Preset(i) => crate::ui::theme::apply_preset(&mut self.settings, i),
                    view::ThemeHit::Panel(i) => crate::ui::theme::set_panel_swatch(&mut self.settings, i),
                    view::ThemeHit::Accent(i) => crate::ui::theme::set_accent_swatch(&mut self.settings, i),
                }
                self.apply_theme();
                self.ui.cfg = self.settings.clone();
                self.settings.save();
                self.repaint_expanded();
            }
            Some(view::SwHit::Field(_) | view::SwHit::Minimize | view::SwHit::Close | view::SwHit::Pick(..)) | None => {
            }
        }
        self.render_settings_win();
    }

    fn sw_focus(&mut self, field: Option<view::Field>) {
        if self.ui.sw.field == field {
            return;
        }
        if self.ui.sw.field == Some(view::Field::WakeWord) {
            self.commit_wake();
        }
        self.ui.sw.field = field;
        self.ui.sw.caret_on = true;
        if field.is_some() {
            self.set_timer(Timer::SwCaret, CARET_BLINK_MS);
        } else {
            self.kill_timer(Timer::SwCaret);
        }
    }

    pub(in crate::island) fn sw_caret_tick(&mut self) {
        if self.ui.sw.field.is_none() || self.settings_win.is_none() {
            return;
        }
        self.ui.sw.caret_on = !self.ui.sw.caret_on;
        self.set_timer(Timer::SwCaret, CARET_BLINK_MS);
        self.render_settings_win();
    }

    fn sw_edit_mut(&mut self) -> Option<&mut edit::TextEdit> {
        Some(match self.ui.sw.field? {
            view::Field::DiscordId => &mut self.ui.int.id,
            view::Field::DiscordSecret => &mut self.ui.int.secret,
            view::Field::SpotifyId => &mut self.ui.int.spotify_id,
            view::Field::AiKey => &mut self.ui.int.ai_key,
            view::Field::AiModel => &mut self.ui.int.ai_model,
            view::Field::AiBase => &mut self.ui.int.ai_base,
            view::Field::VoiceKey => &mut self.ui.int.voice_key,
            view::Field::VoiceModel => &mut self.ui.int.voice_model,
            view::Field::VoiceBase => &mut self.ui.int.voice_base,
            view::Field::WakeWord => &mut self.ui.int.wake,
            view::Field::GroupName => &mut self.ui.int.group_name,
            view::Field::Search => &mut self.ui.int.search,
        })
    }

    fn sw_click_field(&mut self, field: view::Field, x: i32, extend: bool) {
        let Some((_, inner)) = view::sw_field_rects(self.sw_size(), self.dpi_scale, &self.ui, field) else { return };
        let shown = {
            let text = view::field_edit(&self.ui, field).text();
            if matches!(field, view::Field::DiscordSecret | view::Field::AiKey | view::Field::VoiceKey) {
                "\u{2022}".repeat(text.chars().count())
            } else {
                text
            }
        };
        let Ok(layout) = crate::ui::gfx::make_layout(self.gpu.dwrite(), &shown, 100_000.0, 13.0 * self.dpi_scale)
        else {
            return;
        };
        if let Some(n) = self.sw_edit_mut() {
            let fx = if (x as f32) < inner.0 { 1.0e6 } else { x as f32 - inner.0 };
            n.click(&layout, fx, 4.0, extend);
        }
    }

    pub(in crate::island) fn sync_ai_fields(&mut self) {
        let provider = self.settings.ai.provider;
        self.ui.int.ai_model.set_text(self.settings.ai.model(provider));
        self.ui.int.ai_base.set_text(&self.settings.ai.openai_base);
        self.ui.int.ai_key.set_text("");
        self.ui.int.ai_key_saved = ai::has_key(provider);
        self.ui.int.voice_key_shared =
            ai::voice_shares_chat_key(&self.settings.ai) && ai::has_key(crate::data::settings::AiProvider::OpenAi);
    }

    pub(in crate::island) fn sync_voice_fields(&mut self) {
        self.ui.int.voice_model.set_text(&self.settings.ai.voice_model);
        self.ui.int.voice_base.set_text(&self.settings.ai.voice_base);
        self.ui.int.voice_key.set_text("");
        self.ui.int.voice_key_saved = ai::has_voice_key();
        self.ui.int.wake.set_text(&self.settings.ai.voice_wake);
        self.sync_ai_fields();
    }

    fn commit_wake(&mut self) {
        let typed = self.ui.int.wake.text();
        if let Some(wake) = crate::tools::voicecmd::clean_wake_setting(&typed)
            && wake != self.settings.ai.voice_wake
        {
            self.settings.ai.voice_wake = wake;
            self.settings.save();
            self.ui.cfg = self.settings.clone();
            ai::configure(&self.settings.ai);
        }
    }

    fn save_wake(&mut self) {
        let Some(wake) = crate::tools::voicecmd::clean_wake_setting(&self.ui.int.wake.text()) else {
            return self.notify(Notice::Error, '\u{E783}', "Up to 5 names of up to 24 characters, separated by commas");
        };
        self.settings.ai.voice_wake = wake;
        self.settings.save();
        self.ui.cfg = self.settings.clone();
        ai::configure(&self.settings.ai);
        self.sync_voice_fields();
        self.notify(Notice::Integrations, '\u{E73E}', "Command name saved");
    }

    fn save_voice(&mut self) {
        let key = self.ui.int.voice_key.text();
        if !key.trim().is_empty() && !ai::save_voice_key(&key) {
            return self.notify(Notice::Error, '\u{E783}', "That does not look like an API key");
        }
        let model = self.ui.int.voice_model.text().trim().to_string();
        let model = if model.is_empty() { "whisper-1".to_string() } else { model };
        if !ai::valid_model(&model, crate::data::settings::AiProvider::OpenAi) {
            return self.notify(Notice::Error, '\u{E783}', "That model name is not valid");
        }
        let Some(base) = ai::clean_base(&self.ui.int.voice_base.text()) else {
            return self.notify(Notice::Error, '\u{E783}', "The server address must be https:// (or local http://)");
        };
        self.settings.ai.voice_model = model;
        self.settings.ai.voice_base = base;
        self.settings.save();
        self.ui.cfg = self.settings.clone();
        ai::configure(&self.settings.ai);
        self.sync_voice_fields();
        self.notify(Notice::Integrations, '\u{E73E}', "Speech service saved");
    }

    fn save_ai(&mut self) {
        let provider = self.settings.ai.provider;
        let key = self.ui.int.ai_key.text();
        if !key.trim().is_empty() && !ai::save_key(provider, &key) {
            return self.notify(Notice::Error, '\u{E783}', "That does not look like an API key");
        }
        let model = self.ui.int.ai_model.text().trim().to_string();
        let model = if model.is_empty() { provider.default_model().to_string() } else { model };
        if !ai::valid_model(&model, provider) {
            return self.notify(Notice::Error, '\u{E783}', "That model name is not valid");
        }
        if provider == crate::data::settings::AiProvider::OpenAi {
            let Some(base) = ai::clean_base(&self.ui.int.ai_base.text()) else {
                return self.notify(
                    Notice::Error,
                    '\u{E783}',
                    "The server address must be https:// (or local http://)",
                );
            };
            self.settings.ai.openai_base = base;
        }
        self.settings.ai.set_model(provider, &model);
        self.settings.save();
        self.ui.cfg = self.settings.clone();
        ai::configure(&self.settings.ai);
        self.sync_ai_fields();
        self.notify(Notice::Integrations, '\u{E73E}', "AI settings saved");
    }

    fn begin_rebind(&mut self, hotkey: Hotkey) {
        self.ui.sw.capturing = Some(hotkey);
        unregister_hotkeys(self.hwnd);
    }

    pub(in crate::island) fn end_rebind(&mut self) {
        if self.ui.sw.capturing.take().is_some() && !self.gaming {
            register_hotkeys(self.hwnd, &self.settings);
        }
    }

    fn reapply_hotkeys(&mut self) -> Vec<Hotkey> {
        if self.gaming {
            return Vec::new();
        }
        unregister_hotkeys(self.hwnd);
        register_hotkeys(self.hwnd, &self.settings)
    }

    fn keys_changed(&mut self) {
        self.settings.save();
        self.ui.cfg = self.settings.clone();
        self.render_settings_win();
    }

    fn capture_key(&mut self, hotkey: Hotkey, vk: u16) {
        use crate::data::keys::{ALT, CTRL, Combo, SHIFT, WIN, valid_vk};
        let down = |k: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY| unsafe { GetKeyState(k.0 as i32) < 0 };
        let vk32 = u32::from(vk);
        if matches!(vk32, 0x10..=0x12 | 0x5B | 0x5C | 0xA0..=0xA5) {
            return;
        }
        let mut mods = 0;
        for (bit, key) in [(CTRL, VK_CONTROL), (ALT, VK_MENU), (SHIFT, VK_SHIFT)] {
            if down(key) {
                mods |= bit;
            }
        }
        if unsafe { GetKeyState(0x5B) < 0 || GetKeyState(0x5C) < 0 } {
            mods |= WIN;
        }
        if down(VK_MENU) {
            self.ui.sw.quiet_sys = true;
        }
        if vk32 == u32::from(VK_ESCAPE.0) {
            self.end_rebind();
            return self.render_settings_win();
        }
        let combo = if mods == 0 && vk32 == u32::from(VK_DELETE.0) {
            hotkey.default_combo()
        } else if mods == 0 || !valid_vk(vk32) {
            let why = if mods == 0 { "Add Ctrl, Alt, Shift or Win" } else { "That key cannot be used" };
            return self.notify(Notice::Error, '\u{E783}', why);
        } else {
            Combo::new(mods, vk32)
        };
        if let Some(other) = self.settings.hotkey_using(hotkey, combo) {
            return self.notify(Notice::Error, '\u{E783}', &format!("Already used by: {}", other.label()));
        }
        self.set_hotkey(hotkey, combo);
    }

    fn set_hotkey(&mut self, hotkey: Hotkey, combo: crate::data::keys::Combo) {
        let old = self.settings.combo(hotkey);
        self.settings.set_combo(hotkey, combo);
        self.ui.sw.capturing = None;
        if self.reapply_hotkeys().contains(&hotkey) {
            self.settings.set_combo(hotkey, old);
            self.reapply_hotkeys();
            self.notify(Notice::Error, '\u{E783}', "Another program already uses these keys");
        }
        self.keys_changed();
    }

    fn sw_act(&mut self, act: view::Act) {
        match act {
            view::Act::Rebind(hotkey) => self.begin_rebind(hotkey),
            view::Act::KeysReset => {
                self.settings.keys.clear();
                self.reapply_hotkeys();
                self.keys_changed();
            }
            view::Act::LyricsAppAdd => {
                let source = self.ui.media.source.clone();
                if self.settings.add_lyrics_app(&source) {
                    self.lyrics_apps_changed();
                }
            }
            view::Act::Agent(i) => {
                if let Some(agent) = agents::Agent::ALL.get(i).copied() {
                    let result =
                        if agents::connected(agent) { agents::disconnect(agent) } else { agents::connect(agent) };
                    if let Err(why) = result {
                        self.notify_for(Notice::Error, '\u{E783}', why, 5000);
                    }
                }
            }
            view::Act::Metric(i) => {
                if let Some(metric) = metrics::Metric::ALL.get(i) {
                    let key = metric.key().to_string();
                    let on_tab = self.settings.metrics_tab.contains(&key);
                    let on_pill = self.settings.metrics_pill.contains(&key);
                    let (to_tab, to_pill) = match (on_tab, on_pill) {
                        (false, false) => (true, false),
                        (true, false) => (false, true),
                        (false, true) => (true, true),
                        (true, true) => (false, false),
                    };
                    for (list, on) in
                        [(&mut self.settings.metrics_tab, to_tab), (&mut self.settings.metrics_pill, to_pill)]
                    {
                        list.retain(|k| *k != key);
                        if on {
                            list.push(key.clone());
                        }
                    }
                    self.metrics_changed();
                }
            }
            view::Act::LyricsAppRemove(i) => {
                if i < self.settings.lyrics_apps.len() {
                    self.settings.lyrics_apps.remove(i);
                    self.lyrics_apps_changed();
                }
            }
            view::Act::DiscordMain => match self.ui.discord.status {
                discord::Status::NoConfig => self.save_credentials(),
                discord::Status::Idle | discord::Status::Error(_) => discord::connect(),
                discord::Status::NotRunning | discord::Status::Connected => discord::disconnect(),
                discord::Status::Authorizing => {}
            },
            view::Act::DiscordMute => discord::set_mute(!self.ui.discord.self_mute),
            view::Act::DiscordDeaf => discord::set_deaf(!self.ui.discord.self_deaf),
            view::Act::DiscordOverlay => self.toggle_overlay(),
            view::Act::SpotifyMain => match self.ui.spotify.status {
                spotify::Status::Connected => spotify::disconnect(),
                spotify::Status::Connecting => {}
                _ => {
                    spotify::reload_status();
                    spotify::connect();
                }
            },
            view::Act::UpdateCheck => {
                update::configure(self.settings.update_check, self.settings.update_beta);
                update::check(self.hwnd);
            }
            view::Act::UpdateDownload => {
                if let update::Status::Available(found) = update::state() {
                    update::download(self.hwnd, found);
                }
            }
            view::Act::UpdateInstall => {
                if let update::Status::Ready(_, path) = update::state() {
                    match update::install(&path) {
                        Ok(()) => unsafe {
                            let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                        },
                        Err(e) => {
                            crate::log_warn!("could not install the update: {e}");
                            self.notify(Notice::Error, '\u{E783}', "The update could not be installed");
                        }
                    }
                }
            }
            view::Act::AiSave => self.save_ai(),
            view::Act::VoiceSave => self.save_voice(),
            view::Act::WakeSave => self.save_wake(),
            view::Act::GroupAdd => {
                let name = self.ui.int.group_name.text();
                let added = self.store.as_ref().and_then(|s| s.act_group_add(&name)).is_some();
                if added {
                    self.ui.int.group_name.set_text("");
                    self.reload_groups();
                } else {
                    self.notify(Notice::Error, '\u{E783}', "That group name is taken, empty or one group too many");
                }
            }
            view::Act::GroupDelete(id) => {
                if let Some(store) = &self.store {
                    store.act_group_delete(id);
                }
                self.reload_groups();
                self.reload_activity();
            }
            view::Act::VoiceRemoveKey => {
                ai::remove_voice_key();
                self.sync_voice_fields();
            }
            view::Act::AiRemoveKey => {
                ai::remove_key(self.settings.ai.provider);
                self.sync_ai_fields();
            }
            view::Act::SpotifySaveId => {
                if spotify::set_client_id(&self.ui.int.spotify_id.text()) {
                    self.notify(Notice::Integrations, '\u{E73E}', "Client ID saved");
                } else {
                    self.notify(Notice::Error, '\u{E783}', "That does not look like a Client ID");
                }
            }
        }
    }

    fn metrics_changed(&mut self) {
        self.settings.save();
        self.ui.cfg = self.settings.clone();
        if let Some(sample) = metrics::latest() {
            self.ui.metrics.pill = view::pill_text(&sample, &self.settings.metrics_pill);
        }
        self.sync_metrics();
        self.repaint_collapsed();
    }

    fn lyrics_apps_changed(&mut self) {
        self.settings.save();
        self.ui.cfg = self.settings.clone();
        self.ui.lyric = view::LyricUi::default();
        self.update_eq();
        self.repaint_collapsed();
    }

    fn sw_row(&mut self, id: view::Sid, hit: view::SetHit, x: i32) {
        match hit {
            view::SetHit::Option(n) => {
                if matches!(
                    id,
                    view::Sid::Character
                        | view::Sid::Edge
                        | view::Sid::PillBattery
                        | view::Sid::AiProvider
                        | view::Sid::CoverMode
                        | view::Sid::TabAnimKind
                        | view::Sid::AnimCurve
                        | view::Sid::TextLayout
                ) {
                    view::cycle_set(id, &mut self.settings, n == 1);
                } else {
                    view::segment_set(id, &mut self.settings, n);
                }
                self.setting_changed(id, true);
            }
            view::SetHit::Label => {}
            view::SetHit::Select => self.ui.sw.dropdown = Some(id),
            view::SetHit::Toggle => {
                let on = !view::toggle_get(id, &self.settings);
                view::toggle_set(id, &mut self.settings, on);
                self.setting_changed(id, true);
            }
            view::SetHit::Slider => {
                self.pointer.slider = Some(id);
                self.ui.sw.drag = Some(id);
                self.sw_drag(x);
                unsafe { SetCapture(self.settings_win.unwrap_or_default()) };
            }
            view::SetHit::Button => {
                if id == view::Sid::OpenLogs
                    && let Some(dir) = crate::data::log::dir()
                {
                    let _ = std::fs::create_dir_all(&dir);
                    cmd::shell_open(&dir.to_string_lossy(), None);
                }
            }
        }
    }

    fn sw_drag(&mut self, x: i32) {
        let Some(id) = self.ui.sw.drag else { return };
        let Some((left, right)) = view::sw_slider_track(self.sw_size(), self.dpi_scale, &self.ui, id) else { return };
        let norm = (x as f32 - left) / (right - left).max(1.0);
        view::slider_apply(id, &mut self.settings, norm);
        self.setting_changed(id, id != view::Sid::Size);
        self.render_settings_win();
    }

    pub(in crate::island) fn set_search(&mut self, text: &str) {
        self.ui.int.search.set_text(text);
        self.sync_search();
    }

    fn sync_search(&mut self) {
        let text = self.ui.int.search.text();
        if text != self.ui.sw.search {
            self.ui.sw.search = text;
            self.ui.sw.scroll = 0.0;
            self.ui.sw.hover = None;
        }
    }

    pub(in crate::island) fn on_sw_up(&mut self) {
        unsafe {
            let _ = ReleaseCapture();
        }
        self.ui.sw.search_press = false;
        if self.ui.sw.drag.take().is_some() {
            self.finish_slider();
            self.render_settings_win();
        }
    }

    pub(in crate::island) fn on_sw_wheel(&mut self, delta: i32) {
        self.ui.sw.dropdown = None;
        let max = view::max_scroll(self.sw_size(), self.dpi_scale, &self.ui);
        let scroll = (self.ui.sw.scroll - delta as f32 / 120.0 * SCROLL_STEP * self.dpi_scale).clamp(0.0, max);
        if scroll != self.ui.sw.scroll {
            self.ui.sw.scroll = scroll;
            self.render_settings_win();
        }
    }

    pub(in crate::island) fn on_sw_char(&mut self, c: u16) -> bool {
        if c == 0x1B && self.ui.sw.dropdown.take().is_some() {
            self.render_settings_win();
            return false;
        }
        if self.ui.sw.field.is_none() && ((c >= 0x20 && c != 0x7F) || c == 0x16) {
            self.sw_focus(Some(view::Field::Search));
        }
        if self.ui.sw.field == Some(view::Field::Search) {
            match c {
                0x1B => {
                    if self.ui.int.search.is_empty() {
                        return true;
                    }
                    self.set_search("");
                    self.render_settings_win();
                    return false;
                }
                0x0D | 0x09 => return false,
                _ => {}
            }
        }
        if self.ui.sw.field.is_some() {
            self.on_sw_field_char(c);
            self.sync_search();
            self.render_settings_win();
            return false;
        }
        if c == 0x1B {
            if self.ui.sw.search.is_empty() {
                return true;
            }
            self.set_search("");
            self.render_settings_win();
        }
        false
    }

    fn on_sw_field_char(&mut self, unit: u16) {
        let Some(field) = self.ui.sw.field else { return };
        let max_chars = if field == view::Field::Search { MAX_SEARCH_CHARS } else { 400 };
        let hwnd = self.settings_win.unwrap_or(self.hwnd);
        let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
        match unit {
            0x1B => self.sw_focus(None),
            0x09 => {
                let next = view::sw_next_field(&self.ui, field);
                self.sw_focus(next);
            }
            0x0D => {
                let act = match field {
                    view::Field::SpotifyId => view::Act::SpotifySaveId,
                    view::Field::AiKey | view::Field::AiModel | view::Field::AiBase => view::Act::AiSave,
                    view::Field::VoiceKey | view::Field::VoiceModel | view::Field::VoiceBase => view::Act::VoiceSave,
                    view::Field::WakeWord => view::Act::WakeSave,
                    view::Field::GroupName => view::Act::GroupAdd,
                    _ => view::Act::DiscordMain,
                };
                self.sw_act(act);
            }
            0x08 => {
                if let Some(n) = self.sw_edit_mut() {
                    n.backspace(ctrl);
                }
            }
            0x01 => {
                if let Some(n) = self.sw_edit_mut() {
                    n.select_all();
                }
            }
            0x03 | 0x18 => {
                if !matches!(field, view::Field::DiscordSecret | view::Field::AiKey | view::Field::VoiceKey)
                    && let Some(n) = self.sw_edit_mut()
                    && let Some(text) = n.selected_text()
                {
                    crate::data::clips::sys::write_text(hwnd, &text);
                    if unit == 0x18 {
                        n.delete_selection();
                    }
                }
            }
            0x16 => {
                if let Some(text) = crate::data::clips::sys::read_text(hwnd)
                    && let Some(n) = self.sw_edit_mut()
                {
                    let line: String = text.lines().next().unwrap_or("").trim().chars().take(max_chars).collect();
                    n.insert(&line);
                }
            }
            0xD800..=0xDBFF => {
                self.pending_high_surrogate = Some(unit);
                return;
            }
            u if u < 0x20 || u == 0x7F => return,
            u => {
                let c = if (0xDC00..=0xDFFF).contains(&u) {
                    let Some(high) = self.pending_high_surrogate.take() else { return };
                    char::decode_utf16([high, u]).next().and_then(|r| r.ok())
                } else {
                    char::from_u32(u32::from(u))
                };
                if let (Some(c), Some(n)) = (c, self.sw_edit_mut())
                    && n.text().chars().count() < max_chars
                {
                    let mut buf = [0u8; 4];
                    n.insert(c.encode_utf8(&mut buf));
                }
            }
        }
        self.ui.sw.caret_on = true;
        if self.ui.sw.field.is_some() {
            self.set_timer(Timer::SwCaret, CARET_BLINK_MS);
        }
        self.render_settings_win();
    }

    pub(in crate::island) fn on_sw_keydown(&mut self, vk: u16) -> bool {
        if let Some(hotkey) = self.ui.sw.capturing {
            self.capture_key(hotkey, vk);
            return true;
        }
        self.ui.sw.quiet_sys = false;
        if self.ui.sw.field.is_none() {
            return false;
        }
        let down = |k: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY| unsafe { GetKeyState(k.0 as i32) < 0 };
        let (ctrl, shift) = (down(VK_CONTROL), down(VK_SHIFT));
        let Some(n) = self.sw_edit_mut() else { return false };
        match vk {
            v if v == VK_LEFT.0 => n.left(shift, ctrl),
            v if v == VK_RIGHT.0 => n.right(shift, ctrl),
            v if v == VK_HOME.0 => n.caret_to_start(shift),
            v if v == VK_END.0 => n.caret_to_end(shift),
            v if v == VK_DELETE.0 => n.delete(ctrl),
            _ => return false,
        }
        self.ui.sw.caret_on = true;
        self.sync_search();
        self.render_settings_win();
        true
    }

    pub(in crate::island) fn on_sw_destroyed(&mut self) {
        self.commit_wake();
        self.end_rebind();
        self.ui.int.search.set_text("");
        self.kill_timer(Timer::SwCaret);
        self.settings_win = None;
        self.pointer.slider = None;
        self.ui.sw = view::SwUi::default();
    }
}

extern "system" fn sw_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe {
                BeginPaint(hwnd, &mut ps);
                let _ = EndPaint(hwnd, &ps);
            }
            with_app(|a| a.render_settings_win());
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (x, y) = point_from(lparam);
            let left_down = wparam.0 & 0x0001 != 0;
            with_app(|a| a.on_sw_move(x, y, left_down));
            LRESULT(0)
        }
        WM_NCCALCSIZE if wparam.0 != 0 => LRESULT(0),
        WM_NCHITTEST => {
            let mut pt =
                windows::Win32::Foundation::POINT { x: lparam.0 as i16 as i32, y: (lparam.0 >> 16) as i16 as i32 };
            unsafe {
                let _ = windows::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut pt);
            }
            let mut zone = None;
            with_app(|a| zone = a.sw_nc_zone(pt.x, pt.y));
            match zone {
                Some(view::TitleZone::Caption) => LRESULT(HTCAPTION as isize),
                _ => LRESULT(HTCLIENT as isize),
            }
        }
        WM_LBUTTONDOWN => {
            let (x, y) = point_from(lparam);
            let mut title = 0;
            with_app(|a| title = a.sw_title_click(x, y));
            match title {
                1 => unsafe {
                    let _ = ShowWindow(hwnd, SW_MINIMIZE);
                },
                2 => unsafe {
                    let _ = DestroyWindow(hwnd);
                },
                _ => with_app(|a| a.on_sw_down(x, y)),
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            with_app(|a| a.on_sw_up());
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let delta = (wparam.0 >> 16) as i16 as i32;
            with_app(|a| a.on_sw_wheel(delta));
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let vk = wparam.0 as u16;
            let mut taken = false;
            with_app(|a| taken = a.on_sw_keydown(vk));
            if msg == WM_SYSKEYDOWN && !taken {
                return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            }
            LRESULT(0)
        }
        WM_SYSCHAR | WM_SYSKEYUP => {
            let mut quiet = false;
            with_app(|a| quiet = a.ui.sw.quiet_sys);
            if quiet { LRESULT(0) } else { unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) } }
        }
        WM_KILLFOCUS => {
            with_app(|a| {
                a.end_rebind();
                a.render_settings_win();
            });
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_CHAR => {
            let mut close = false;
            with_app(|a| close = a.on_sw_char(wparam.0 as u16));
            if close {
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            with_app(|a| a.on_sw_destroyed());
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
