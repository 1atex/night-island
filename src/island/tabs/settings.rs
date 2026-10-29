use crate::island::*;

impl App {
    pub(in crate::island) fn click_settings(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        let Some((id, hit)) = view::hit_setting(pill, self.scale, x as f32, y as f32, self.ui.tab) else {
            return;
        };
        match hit {
            view::SetHit::Option(n) => {
                if matches!(
                    id,
                    view::Sid::Character
                        | view::Sid::Edge
                        | view::Sid::PillBattery
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
            view::SetHit::Toggle => {
                let on = !view::toggle_get(id, &self.settings);
                view::toggle_set(id, &mut self.settings, on);
                self.setting_changed(id, true);
            }
            view::SetHit::Slider => {
                self.pointer.slider = Some(id);
                self.drag_slider(x);
            }
            view::SetHit::Button | view::SetHit::Label | view::SetHit::Select => {}
        }
    }

    pub(in crate::island) fn drag_slider(&mut self, x: i32) {
        let Some(id) = self.pointer.slider else { return };
        let pill = self.pill_rect(true);
        let Some((left, right)) = view::slider_track(pill, self.scale, id, self.ui.tab) else {
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
        self.ui.cfg = self.settings.clone();
        crate::island::geometry::SNAP_FIT.store(true, std::sync::atomic::Ordering::Relaxed);
        self.apply_setting(id, apply);
        if self.state == State::Collapsed {
            self.update_eq();
            self.repaint_collapsed();
        }
        crate::island::geometry::SNAP_FIT.store(false, std::sync::atomic::Ordering::Relaxed);
        self.set_timer(Timer::Settle, 700);
        if self.pointer.slider.is_none() {
            self.settings.save();
        }
    }

    fn apply_setting(&mut self, id: view::Sid, apply: bool) {
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
                ActivityTrack => self.start_activity(),
                ActivityAi => self.reload_groups(),
                VpnPill => {
                    self.ui.vpn = vpn::state();
                    self.update_eq();
                    self.repaint_collapsed();
                }
                MetricsInterval => self.sync_metrics(),
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
                UpdateCheck | UpdateBeta => {
                    update::configure(self.settings.update_check, self.settings.update_beta);
                }
                AiProvider | VoiceSeparate => {
                    ai::configure(&self.settings.ai);
                    self.sync_ai_fields();
                }
                NotifyLockKeys => {
                    if self.settings.notify.lock_keys {
                        lockkeys::start(self.hwnd);
                    } else {
                        lockkeys::stop();
                    }
                }
                EqStyle | Spikes => {
                    self.apply_layout();
                    self.refresh_eq_shape();
                }
                EqWhenOpen => {
                    self.apply_layout();
                    self.update_eq();
                    self.refresh_eq_shape();
                }
                BumpTint => {
                    self.stop_idle();
                    self.liquid_tick();
                }
                EqStrength | EqGradient => self.refresh_eq_shape(),
                MusicEq | MusicTitle | MusicCover | CoverMode | MusicLyrics | MusicProgress | PillWeather
                | PillBattery | SpikeColor => {
                    self.update_eq();
                    self.repaint_collapsed();
                }
                _ => {}
            }
        }
        self.repaint_expanded();
    }
}
