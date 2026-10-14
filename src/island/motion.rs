use crate::island::*;

pub(in crate::island) const OPEN_BASE_MS: f32 = 460.0;
pub(in crate::island) const CLOSE_BASE_MS: f32 = 400.0;
pub(in crate::island) const AUTOHIDE_DELAY_MS: u32 = 1500;

impl App {
    pub(in crate::island) fn ease(&self) -> Result<windows::UI::Composition::CubicBezierEasingFunction> {
        self.compositor.CreateCubicBezierEasingFunction(Vector2 { X: 0.34, Y: 1.45 }, Vector2 { X: 0.64, Y: 1.0 })
    }

    pub(in crate::island) fn spring(&self, target: Vector2, ms: u64) -> Result<CompositionAnimation> {
        let anim = self.compositor.CreateVector2KeyFrameAnimation()?;
        anim.InsertKeyFrameWithEasingFunction(1.0, target, &self.ease()?)?;
        anim.SetDuration(Duration::from_millis(ms).into())?;
        anim.cast()
    }

    pub(in crate::island) fn animate_to(&self, expanded: bool, hidden: bool) -> Result<()> {
        let ms = if expanded { self.open_ms() } else { self.close_ms() };
        let (offset, size, radius) = self.pill(self.win, expanded, hidden);
        self.geometry.StartAnimation(h!("Offset"), &self.spring(offset, ms)?)?;
        self.geometry.StartAnimation(h!("Size"), &self.spring(size, ms)?)?;
        self.geometry.StartAnimation(h!("CornerRadius"), &self.spring(radius, ms)?)?;

        let fade = self.compositor.CreateScalarKeyFrameAnimation()?;
        let open_opacity = if self.settings.acrylic { self.settings.tint_open } else { TINT_OPEN_NO_BLUR };
        fade.InsertKeyFrame(1.0, if expanded { open_opacity } else { self.settings.pill_opacity })?;
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
        match self.state {
            State::Expanded => return,
            State::Collapsing => self.kill_timer(Timer::Shrink),
            State::Hiding => self.kill_timer(Timer::HideDone),
            _ => {}
        }
        self.kill_timer(Timer::Autohide);
        self.set_blur(true);
        if self.win != Win::Big {
            let hidden = self.state == State::Hidden;
            self.place(Win::Big);
            let _ = self.set_geometry_now(Win::Big, false, hidden);
        }
        self.state = State::Expanded;
        self.update_eq();
        let _ = self.animate_to(true, false);
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
        self.end_edit(true);
        self.state = State::Collapsing;
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
        let _ = self.collapsed.resize(window_size(self.scale, Win::Small, self.docked(), self.cw, self.bump_room()));
        let _ = self.set_geometry_now(Win::Small, false, false);
        self.schedule_autohide();
        self.start_liquid();
        self.update_eq();
    }

    pub(in crate::island) fn schedule_autohide(&self) {
        if self.autohide_ok() && self.state == State::Collapsed && !self.cursor_inside() {
            self.set_timer(Timer::Autohide, AUTOHIDE_DELAY_MS);
        }
    }

    pub(in crate::island) fn hide(&mut self) {
        if self.state != State::Collapsed {
            return;
        }
        self.state = State::Hiding;
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
