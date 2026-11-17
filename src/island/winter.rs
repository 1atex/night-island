use crate::island::*;

impl App {
    pub(in crate::island) fn configure_winter(&mut self) {
        if self.settings.winter.enabled
            && let Some(winter) = &mut self.winter
            && winter.configure(self.settings.winter)
        {
            self.set_timer(Timer::Winter, if self.settings.winter.economy { 33 } else { self.frame_ms() });
            return;
        }
        self.winter.take();
        self.kill_timer(Timer::Winter);
        if !self.settings.winter.enabled {
            return;
        }
        match crate::ui::winter::Winter::new(&self.compositor, self.settings.winter) {
            Ok(winter) => {
                self.winter = Some(winter);
                self.set_timer(Timer::Winter, if self.settings.winter.economy { 33 } else { self.frame_ms() });
            }
            Err(error) => crate::log_warn!("Winter desktop unavailable: {error}"),
        }
    }
    pub(in crate::island) fn winter_tick(&mut self) {
        let mut radius = self.pill(self.win, self.state == State::Expanded, false).2.X;
        let mut rect = self.pill_screen(self.state == State::Expanded);
        if let Some(motion) = &self.open_motion {
            let closed = self.pill_screen(false);
            let opened = self.pill_screen(true);
            let p = motion.progress.value(Instant::now());
            radius = self.pill(self.win, false, false).2.X
                + (self.pill(self.win, true, false).2.X - self.pill(self.win, false, false).2.X) * p;
            rect = (
                closed.0 + (opened.0 - closed.0) * p,
                closed.1 + (opened.1 - closed.1) * p,
                closed.2 + (opened.2 - closed.2) * p,
                closed.3 + (opened.3 - closed.3) * p,
            );
        }
        let visible = self.state != State::Hidden;
        let dock = self.desktop_dock.as_ref().and_then(|dock| dock.snow_ledge());
        if let Some(winter) = &mut self.winter
            && let Err(error) = winter.tick(rect, visible, dock, self.state == State::Expanded, radius)
        {
            crate::log_warn!("Winter desktop stopped: {error}");
            self.winter.take();
            self.kill_timer(Timer::Winter);
        }
        if self.winter.is_some() {
            self.set_timer(Timer::Winter, if self.settings.winter.economy { 33 } else { self.frame_ms() });
        }
    }
}
