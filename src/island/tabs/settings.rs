use crate::island::*;

impl App {
    pub(in crate::island) fn click_theme(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        if let Some(hit) = view::hit_theme(pill, self.scale, x as f32, y as f32) {
            match hit {
                view::ThemeHit::Preset(i) => crate::ui::theme::apply_preset(&mut self.settings, i),
                view::ThemeHit::Panel(i) => crate::ui::theme::set_panel_swatch(&mut self.settings, i),
                view::ThemeHit::Accent(i) => crate::ui::theme::set_accent_swatch(&mut self.settings, i),
            }
            self.apply_theme();
            self.ui.cfg = self.settings.clone();
            self.settings.save();
            return self.repaint_expanded();
        }
        self.click_settings(x, y);
    }

    pub(in crate::island) fn click_settings(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        if self.ui.tab == view::Tab::Settings && view::hit_pager(pill, self.scale, x as f32, y as f32).is_some() {
            self.ui.settings_page = 1 - self.ui.settings_page.min(1);
            return self.repaint_expanded();
        }
        let Some((id, hit)) =
            view::hit_setting(pill, self.scale, x as f32, y as f32, self.ui.tab, self.ui.settings_page)
        else {
            return;
        };
        match hit {
            view::SetHit::Option(n) => {
                if matches!(id, view::Sid::Character | view::Sid::Edge | view::Sid::PillBattery) {
                    view::cycle_set(id, &mut self.settings, n == 1);
                } else {
                    view::segment_set(id, &mut self.settings, n);
                }
                self.setting_changed(id, true);
            }
            view::SetHit::Toggle => {
                let on = !view::toggle_get(id, &self.settings);
                view::toggle_set(id, &mut self.settings, on);
                self.setting_changed(id, true);
            }
            view::SetHit::Slider => {
                self.pointer.slider = Some(id);
                self.drag_slider(x);
            }
        }
    }

    pub(in crate::island) fn drag_slider(&mut self, x: i32) {
        let Some(id) = self.pointer.slider else { return };
        let pill = self.pill_rect(true);
        let Some((left, right)) = view::slider_track(pill, self.scale, id, self.ui.tab, self.ui.settings_page) else {
            return;
        };
        let norm = (x as f32 - left) / (right - left).max(1.0);
        view::slider_apply(id, &mut self.settings, norm);
        self.setting_changed(id, id != view::Sid::Size);
    }

    pub(in crate::island) fn finish_slider(&mut self) {
        let Some(id) = self.pointer.slider.take() else { return };
        self.setting_changed(id, true);
        self.settings.save();
    }

    pub(in crate::island) fn setting_changed(&mut self, id: view::Sid, apply: bool) {
        use view::Sid::*;
        if apply {
            match id {
                Placement | Size | Edge => self.apply_layout(),
                Style => self.apply_style(),
                Tint => {
                    if self.state == State::Expanded {
                        let _ = self.tint.SetOpacity(if self.settings.acrylic {
                            self.settings.tint_open
                        } else {
                            TINT_OPEN_NO_BLUR
                        });
                    }
                }
                AutoHide => {
                    if self.settings.auto_hide {
                        self.schedule_autohide();
                    } else {
                        self.kill_timer(Timer::Autohide);
                    }
                }
                Acrylic => {
                    self.set_blur(self.state == State::Expanded);
                    if self.state == State::Expanded {
                        let _ = self.animate_to(true, false);
                    }
                }
                PanelHue | PanelSat | PanelVal | AccentHue | AccentSat => self.apply_theme(),
                PillOpacity => self.apply_pill_opacity(),
                PillBlur => self.set_blur(self.state == State::Expanded),
                Capture => self.apply_capture(),
                CaptureSelf => capture::set_hide_self(self.settings.capture_self_hidden),
                GameMode => self.start_game_watch(),
                Weather => weather::set_enabled(self.settings.weather),
                Language => {
                    crate::ui::i18n::set_ru(self.settings.language == crate::data::settings::Language::Ru);
                    self.repaint_collapsed();
                }
                Character => self.apply_character(),
                MusicEq | MusicTitle | MusicLyrics | PillWeather | PillBattery | SpikeColor | Spikes => {
                    self.update_eq();
                    self.repaint_collapsed();
                }
                _ => {}
            }
        }
        self.ui.cfg = self.settings.clone();
        self.repaint_expanded();
        if self.pointer.slider.is_none() {
            self.settings.save();
        }
    }
}
