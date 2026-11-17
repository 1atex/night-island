use crate::island::*;

impl App {
    pub(in crate::island) fn configure_winter(&mut self) {
        if self.settings.winter.enabled
            && let Some(winter) = &mut self.winter
            && winter.configure(self.settings.winter)
        {
            self.set_timer(Timer::Winter, if self.settings.winter.economy { 50 } else { 33 });
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
                self.set_timer(Timer::Winter, if self.settings.winter.economy { 50 } else { 33 });
            }
            Err(error) => crate::log_warn!("Winter desktop unavailable: {error}"),
        }
    }
    pub(in crate::island) fn winter_tick(&mut self) {
        let rect = self.pill_screen(self.state == State::Expanded);
        let visible = self.state != State::Hidden;
        let dock = self.desktop_dock.as_ref().and_then(|dock| dock.snow_ledge());
        if let Some(winter) = &mut self.winter
            && let Err(error) = winter.tick(rect, visible, dock)
        {
            crate::log_warn!("Winter desktop stopped: {error}");
            self.winter.take();
            self.kill_timer(Timer::Winter);
        }
        if self.winter.is_some() {
            self.set_timer(Timer::Winter, if self.settings.winter.economy { 50 } else { 33 });
        }
    }
}
