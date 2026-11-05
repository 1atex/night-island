use crate::island::*;
use crate::platform::desktop_dock as desktop;
use crate::ui::top_panel::{Block, Request, TopPanel};

impl App {
    pub(in crate::island) fn configure_top_panel(&mut self) {
        if !self.settings.top_panel.enabled || !self.settings.top_panel.has_blocks() {
            self.top_panel.take();
            self.kill_timer(Timer::TopPanel);
            self.apply_layout();
            return;
        }
        if self.top_panel.is_none() {
            match TopPanel::new(self.hwnd, &self.gpu, &self.settings.top_panel) {
                Ok(panel) => self.top_panel = Some(panel),
                Err(error) => {
                    crate::log_error!("top panel: {error}");
                    return;
                }
            }
        }
        if let Some(panel) = &mut self.top_panel {
            panel.configure(&self.settings.top_panel, self.dpi_scale);
            panel.set_capture_hidden(self.settings.capture_hidden);
        }
        self.top_panel_poll = Instant::now() - Duration::from_secs(10);
        system::radios_async(self.hwnd, None);
        self.apply_layout();
        self.update_top_panel(true);
    }

    pub(in crate::island) fn update_top_panel(&mut self, force: bool) {
        if self.top_panel.is_none() {
            return;
        }
        let suppressed = self.top_panel.as_ref().is_some_and(|panel| panel.suppressed());
        let mut snapshot = self.top_panel_data.clone();
        if !suppressed && (force || self.top_panel_poll.elapsed() >= Duration::from_secs(1)) {
            self.top_panel_poll = Instant::now();
            let windows = desktop::windows();
            snapshot.active = windows.iter().find(|w| w.active).cloned();
            snapshot.windows = snapshot
                .active
                .as_ref()
                .map(|active| windows.iter().filter(|w| w.path == active.path).cloned().collect())
                .unwrap_or_default();
            snapshot.volume = volume::get().map(|v| (v * 100.0).round() as u8);
            snapshot.muted = volume::muted().unwrap_or(false);
            snapshot.device = volume::device_name().unwrap_or_default();
            snapshot.battery = view::battery();
        }
        if !suppressed && self.top_panel_radios.elapsed() >= Duration::from_secs(10) {
            self.top_panel_radios = Instant::now();
            system::radios_async(self.hwnd, None);
        }
        snapshot.radios = self.ui.radios;
        self.top_panel_data = snapshot.clone();
        let (x, y, w, h) = self.pill_screen(matches!(self.state, State::Expanded | State::Collapsing));
        let mut occupied = (x, y, x + w, y + h);
        if let Some((nx, ny, nw, nh)) = self.notification_occupied_rect() {
            occupied = (occupied.0.min(nx), occupied.1.min(ny), occupied.2.max(nx + nw), occupied.3.max(ny + nh));
        }
        if let Some(panel) = &mut self.top_panel {
            panel.update(snapshot, occupied, false);
        }
        self.set_timer(
            Timer::TopPanel,
            if suppressed {
                500
            } else if self.settings.top_panel.auto_hide {
                80
            } else {
                250
            },
        );
    }

    pub(in crate::island) fn top_panel_request(&mut self, id: usize) {
        if id == 3 || id == 4 {
            if let Some(panel) = &mut self.top_panel {
                if id == 4 {
                    panel.shell_restarted();
                } else {
                    panel.reflow();
                }
            }
            self.apply_layout();
            self.update_top_panel(true);
            return;
        }
        while let Some(request) = self.top_panel.as_mut().and_then(|panel| panel.take_request(id)) {
            match request {
                Request::Tray(anchor) => crate::platform::system_tray::toggle(self.hwnd, anchor),
                Request::Settings => {
                    if let Some(group) = view::GROUPS.iter().position(|g| g.name == "Top panel") {
                        self.open_settings_at(group);
                    }
                }
                Request::Open(block) => {
                    self.update_top_panel(true);
                    if let Some(panel) = &mut self.top_panel {
                        panel.open(block);
                    }
                }
                Request::ToggleWifi => self.run_ctl(view::Ctl::WiFi),
                Request::ToggleBluetooth => self.run_ctl(view::Ctl::Bluetooth),
                Request::Volume(value) => {
                    volume::set(value as f32 / 100.0);
                    self.ui.home.vol = value as f32 / 100.0;
                    self.top_panel_data.volume = Some(value);
                    self.top_panel_data.muted = value == 0;
                    self.repaint_expanded();
                }
                Request::Mute => {
                    volume::set_muted(!volume::muted().unwrap_or(false));
                    self.update_top_panel(true);
                }
                Request::WindowsSettings(block) => cmd::shell_open(
                    match block {
                        Block::Wifi => "ms-settings:network-wifi",
                        Block::Bluetooth => "ms-settings:bluetooth",
                        Block::Battery => "ms-settings:powersleep",
                        _ => "ms-settings:sound",
                    },
                    None,
                ),
                Request::Mixer => {
                    self.select_tab(view::Tab::Mixer);
                    self.expand();
                }
                Request::Window(window, action) => {
                    desktop::change_window(&window, action);
                    self.update_top_panel(true);
                }
                Request::Activate(window) => {
                    desktop::activate(&window, false);
                    self.update_top_panel(true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Checks island alignment and notification geometry with a temporary native top appbar"]
    fn native_top_panel_keeps_island_and_notifications_in_reserved_strip() -> Result<()> {
        use windows::Win32::System::WinRT::{
            CreateDispatcherQueueController, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
            RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize,
        };
        unsafe {
            RoInitialize(RO_INIT_SINGLETHREADED)?;
        }
        struct Runtime;
        impl Drop for Runtime {
            fn drop(&mut self) {
                unsafe {
                    RoUninitialize();
                }
            }
        }
        let _runtime = Runtime;
        let _queue = unsafe {
            CreateDispatcherQueueController(DispatcherQueueOptions {
                dwSize: size_of::<DispatcherQueueOptions>() as u32,
                threadType: DQTYPE_THREAD_CURRENT,
                apartmentType: DQTAT_COM_NONE,
            })?
        };
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("Top panel island fixture"),
                WS_POPUP,
                -12000,
                -12000,
                1,
                1,
                None,
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?
        };
        struct Window(HWND);
        impl Drop for Window {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }
        let _window = Window(hwnd);
        let before = App::work_area();
        let settings = Settings {
            top_panel: crate::data::top_panel::TopPanelSettings {
                enabled: true,
                auto_hide: false,
                ..Default::default()
            },
            music_eq: false,
            edge_spikes: false,
            placement: Placement::Floating,
            ..Default::default()
        };
        let mut app = App::new_visuals(hwnd, 1.0, settings)?;
        let mut panel = TopPanel::new(hwnd, &app.gpu, &app.settings.top_panel)?;
        panel.configure(&app.settings.top_panel, 1.0);
        let top = panel.top();
        app.top_panel = Some(panel);
        assert!(App::work_area().1 >= top + 50.0);
        assert_eq!(app.island_work_area().1, top);
        assert_eq!(app.pill_screen(false).1, top + 8.0);
        let notification =
            crate::ui::notification_card::card_target(app.pill_screen(false), app.island_work_area(), None, 1.0);
        assert_eq!(notification.1, top + 8.0);
        app.settings.placement = Placement::Docked;
        app.settings.dock_edge = Edge::Top;
        assert_eq!(app.pill_screen(false).1, top);
        app.top_panel.take();
        let deadline = Instant::now() + Duration::from_secs(2);
        while App::work_area() != before {
            assert!(Instant::now() < deadline, "Disabling top panel must restore desktop work area");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(app.island_work_area(), before);
        Ok(())
    }
}
