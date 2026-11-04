use crate::island::*;

pub(in crate::island) const OPEN_BASE_MS: f32 = 460.0;
pub(in crate::island) const CLOSE_BASE_MS: f32 = 400.0;
pub(in crate::island) const AUTOHIDE_DELAY_MS: u32 = 1500;

impl App {
    pub(in crate::island) fn ease(&self) -> Result<windows::UI::Composition::CompositionEasingFunction> {
        let (x1, y1, x2, y2) =
            crate::ui::anim::bezier_points(self.settings.anim_curve).unwrap_or((0.33, 0.33, 0.67, 0.67));
        self.compositor.CreateCubicBezierEasingFunction(Vector2 { X: x1, Y: y1 }, Vector2 { X: x2, Y: y2 })?.cast()
    }

    pub(in crate::island) fn spring(&self, target: Vector2, ms: u64) -> Result<CompositionAnimation> {
        let anim = self.compositor.CreateVector2KeyFrameAnimation()?;
        anim.InsertKeyFrameWithEasingFunction(1.0, target, &self.ease()?)?;
        anim.SetDuration(Duration::from_millis(ms).into())?;
        anim.cast()
    }

    pub(in crate::island) fn animate_to(&self, expanded: bool, hidden: bool) -> Result<()> {
        self.restore_liquid_body();
        let ms = if expanded { self.open_ms() } else { self.close_ms() };
        let _ = self.collapsed.sprite.SetIsVisible(true);
        let _ = self.expanded.sprite.SetIsVisible(true);
        if self.attached_character_on() {
            let _ = self.pet.char_layer.sprite.SetIsVisible(true);
        }
        let (offset, size, radius) = self.pill(self.win, expanded, hidden);
        self.geometry.StartAnimation(h!("Offset"), &self.spring(offset, ms)?)?;
        self.geometry.StartAnimation(h!("Size"), &self.spring(size, ms)?)?;
        self.geometry.StartAnimation(h!("CornerRadius"), &self.spring(radius, ms)?)?;

        let fade = self.compositor.CreateScalarKeyFrameAnimation()?;
        fade.InsertKeyFrame(1.0, if expanded { self.settings.tint_open } else { self.settings.pill_opacity })?;
        fade.SetDuration(Duration::from_millis(ms * 7 / 10).into())?;
        self.tint.StartAnimation(h!("Opacity"), &fade)?;

        for (layer, shown) in
            [(&self.collapsed, !expanded), (&self.expanded, expanded), (&self.pet.char_layer, !expanded)]
        {
            let a = self.compositor.CreateScalarKeyFrameAnimation()?;
            a.InsertKeyFrame(1.0, if shown { 1.0 } else { 0.0 })?;
            let fade_ms = if shown { ms * 6 / 10 } else { ms / 3 };
            a.SetDuration(Duration::from_millis(fade_ms).into())?;
            layer.sprite.StartAnimation(h!("Opacity"), &a)?;
        }

        let (left, right, ear_opacity) = self.ear_targets(self.win, expanded, hidden);
        for (ear, x) in self.ears.iter().zip([left, right]) {
            let slide = self.compositor.CreateVector3KeyFrameAnimation()?;
            slide.InsertKeyFrameWithEasingFunction(1.0, Vector3 { X: x, Y: 0.0, Z: 0.0 }, &self.ease()?)?;
            slide.SetDuration(Duration::from_millis(ms).into())?;
            ear.sprite.StartAnimation(h!("Offset"), &slide.cast::<CompositionAnimation>()?)?;
            let a = self.compositor.CreateScalarKeyFrameAnimation()?;
            a.InsertKeyFrame(1.0, ear_opacity)?;
            a.SetDuration(Duration::from_millis(ms / 2).into())?;
            ear.sprite.StartAnimation(h!("Opacity"), &a)?;
        }
        Ok(())
    }

    pub(in crate::island) fn expand(&mut self) {
        self.set_decoration_through(false);
        let notification_origin = self.system_notices.origin.take();
        self.hide_notification_card();
        self.cancel_pill_motion();
        match self.state {
            State::Expanded => return,
            State::Collapsing => self.kill_timer(Timer::Shrink),
            State::Hiding => self.kill_timer(Timer::HideDone),
            _ => {}
        }
        self.stop_idle();
        self.kill_timer(Timer::Autohide);
        self.set_blur(true);
        if self.win != Win::Big {
            let hidden = self.state == State::Hidden;
            self.place(Win::Big);
            let _ = self.set_geometry_now(Win::Big, false, hidden);
        }
        self.state = State::Expanded;
        if let Some((x, y, w, h)) = notification_origin {
            let (wx, wy) = self.window_origin(Win::Big);
            let _ = self.geometry.SetOffset(windows_numerics::Vector2 { X: x - wx, Y: y - wy });
            let _ = self.geometry.SetSize(windows_numerics::Vector2 { X: w, Y: h });
            let _ =
                self.geometry.SetCornerRadius(windows_numerics::Vector2 { X: 26.0 * self.scale, Y: 26.0 * self.scale });
        }
        self.sync_mixer();
        self.update_eq();
        self.sync_metrics();
        self.set_timer(Timer::Settle, self.open_ms() as u32 + 80);
        let _ = self.animate_to(true, false);
        self.update_weather_timer();
        if matches!(self.ui.tab, view::Tab::Controls | view::Tab::Home) {
            self.select_tab(self.ui.tab);
        }
    }

    pub(in crate::island) fn collapse(&mut self) {
        if self.state != State::Expanded || self.ui.remind.alert.is_some() {
            return;
        }
        self.ui.cmd.open = false;
        self.ui.armed = None;
        self.ui.home.dev_menu = None;
        self.close_home_mixer();
        self.end_edit(true);
        self.kill_timer(Timer::Collapse);
        self.state = State::Collapsing;
        self.finish_mixer();
        self.sync_mixer();
        self.update_weather_timer();
        self.update_eq();
        let _ = self.animate_to(false, false);
        self.set_timer(Timer::Shrink, self.close_ms() as u32 + 60);
    }

    pub(in crate::island) fn shrink_window(&mut self) {
        if self.state != State::Collapsing {
            return;
        }
        self.state = State::Collapsed;
        self.set_blur(false);
        self.cw = view::collapsed_width(&self.ui, COLLAPSED.0);
        self.place(Win::Small);
        let small = window_size(self.scale, Win::Small, self.docked(), self.cw, &window_room(&self.settings));
        let _ = self.collapsed.resize(small);
        self.resize_ring();
        self.fit_bump();
        let _ = self.set_geometry_now(Win::Small, false, false);
        self.schedule_autohide();
        self.start_liquid();
        self.update_eq();
        self.repaint_collapsed();
        self.settle_layers();
        self.sync_metrics();
    }

    pub(in crate::island) fn settle_layers(&self) {
        if self.pill_motion_busy() {
            return;
        }
        let expanded = match self.state {
            State::Expanded => true,
            State::Collapsed => false,
            _ => return,
        };
        let _ = self.collapsed.sprite.SetIsVisible(!expanded);
        let _ = self.expanded.sprite.SetIsVisible(expanded);
        if expanded {
            let _ = self.pet.char_layer.sprite.SetIsVisible(false);
            self.set_bump_visible(false);
            for bar in &self.music.spectrum {
                let _ = bar.SetIsVisible(false);
            }
        } else {
            let _ = self.pet.char_layer.sprite.SetIsVisible(self.attached_character_on());
        }
        let tint_want = if expanded { self.settings.tint_open } else { self.settings.pill_opacity };
        let layers = [
            ("collapsed", &self.collapsed.sprite, if expanded { 0.0 } else { 1.0 }),
            ("expanded", &self.expanded.sprite, if expanded { 1.0 } else { 0.0 }),
            ("character", &self.pet.char_layer.sprite, if expanded { 0.0 } else { 1.0 }),
        ];
        for (name, sprite, want) in layers {
            let have = sprite.Opacity().unwrap_or(want);
            if (have - want).abs() > 0.02 {
                crate::log_warn!("layer {name} stuck at opacity {have:.2}, should be {want:.2}; fixed");
                let _ = sprite.StopAnimation(h!("Opacity"));
                let _ = sprite.SetOpacity(want);
            }
        }
        let have = self.tint.Opacity().unwrap_or(tint_want);
        if (have - tint_want).abs() > 0.02 {
            crate::log_warn!("tint stuck at opacity {have:.2}, should be {tint_want:.2}; fixed");
            let _ = self.tint.StopAnimation(h!("Opacity"));
            let _ = self.tint.SetOpacity(tint_want);
        }
    }

    pub(in crate::island) fn schedule_autohide(&self) {
        if !self.notification_card_visible()
            && self.pill_motion.intro.is_none()
            && self.autohide_ok()
            && self.state == State::Collapsed
            && !self.cursor_inside()
        {
            self.set_timer(Timer::Autohide, AUTOHIDE_DELAY_MS);
        }
    }

    pub(in crate::island) fn hide(&mut self) {
        self.hide_notification_card();
        self.cancel_pill_motion();
        if self.state != State::Collapsed {
            return;
        }
        self.state = State::Hiding;
        self.update_weather_timer();
        self.update_eq();
        let _ = self.animate_to(false, true);
        self.set_timer(Timer::HideDone, self.close_ms() as u32 + 60);
    }

    pub(in crate::island) fn hide_done(&mut self) {
        if self.state != State::Hiding {
            return;
        }
        self.state = State::Hidden;
        self.place(Win::Strip);
        let _ = self.set_geometry_now(Win::Strip, false, true);
    }

    pub(in crate::island) fn reveal(&mut self) {
        match self.state {
            State::Hiding => self.kill_timer(Timer::HideDone),
            State::Hidden => {
                self.place(Win::Small);
                let _ = self.set_geometry_now(Win::Small, false, true);
            }
            _ => return,
        }
        self.state = State::Collapsed;
        let _ = self.animate_to(false, false);
        self.start_liquid();
        self.update_eq();
        self.update_weather_timer();
    }

    pub(in crate::island) fn peek(&mut self) {
        self.expand();
        if !self.cursor_inside() {
            self.set_timer(Timer::Collapse, 3000);
        }
    }

    pub(in crate::island) fn toggle(&mut self) {
        match self.state {
            State::Expanded => self.collapse(),
            _ => self.expand(),
        }
    }
}
