use crate::data::desktop_dock::{Pin, key};
use crate::island::*;
use crate::platform::desktop_dock as desktop;
use crate::ui::desktop_dock::{Dock, Request, WM_DESKTOP_DOCK};
use crate::ui::i18n::tr;

impl App {
    pub(in crate::island) fn configure_desktop_dock(&mut self) {
        if !self.settings.desktop_dock.enabled {
            self.desktop_dock.take();
            return;
        }
        if let Some(dock) = &mut self.desktop_dock {
            dock.update(&self.settings.desktop_dock);
        } else {
            match Dock::new(self.hwnd, &self.compositor, &self.gpu, &self.settings.desktop_dock) {
                Ok(dock) => {
                    dock.set_capture_hidden(self.settings.capture_hidden);
                    self.desktop_dock = Some(dock);
                }
                Err(error) => {
                    self.ui.sw.dock_status = tr("Could not create the desktop dock").into_owned();
                    self.settings.desktop_dock.enabled = false;
                    self.ui.cfg = self.settings.clone();
                    crate::log_error!("desktop dock: {error}");
                }
            }
        }
    }

    pub(in crate::island) fn save_desktop_dock(&mut self) {
        self.settings.desktop_dock.sanitize();
        self.ui.cfg = self.settings.clone();
        self.settings.save();
        self.configure_desktop_dock();
        if self.settings_win.is_some() {
            self.render_settings_win();
        }
    }

    pub(in crate::island) fn desktop_dock_setting(&mut self, id: view::Sid) {
        match id {
            view::Sid::DockAdd | view::Sid::DockImport => unsafe {
                let _ = PostMessageW(
                    Some(self.hwnd),
                    WM_DESKTOP_DOCK,
                    WPARAM(if id == view::Sid::DockAdd { 1 } else { 2 }),
                    LPARAM(0),
                );
            },
            view::Sid::DockTaskbar => cmd::shell_open("ms-settings:taskbar", None),
            view::Sid::DockMonitor => {
                let monitors = desktop::monitors();
                let index = monitors.iter().position(|m| m.device == self.settings.desktop_dock.monitor);
                self.settings.desktop_dock.monitor = match index {
                    Some(i) if i + 1 < monitors.len() => monitors[i + 1].device.clone(),
                    None if !monitors.is_empty() => monitors[0].device.clone(),
                    _ => String::new(),
                };
                self.save_desktop_dock();
            }
            _ => {}
        }
    }
}

fn add_file(hwnd: HWND) {
    let owner = with_app_value(|a| a.settings_win.unwrap_or(hwnd)).unwrap_or(hwnd);
    if let Some(path) = pick_file(owner) {
        with_app(|a| {
            let added = a.settings.desktop_dock.add(Pin::from_path(&path));
            a.ui.sw.dock_status =
                tr(if added { "Application added to the dock" } else { "Already pinned or dock is full (32)" })
                    .into_owned();
            a.save_desktop_dock();
        });
    }
}

fn import_dock(hwnd: HWND) {
    let default = std::env::var_os("ProgramFiles(x86)")
        .map(std::path::PathBuf::from)
        .map(|p| p.join("Steam/steamapps/common/MyDockFinder/ico.ini"));
    let file = default.filter(|p| p.is_file()).or_else(|| {
        let owner = with_app_value(|a| a.settings_win.unwrap_or(hwnd)).unwrap_or(hwnd);
        pick_file(owner).map(std::path::PathBuf::from)
    });
    let Some(file) = file else { return };
    let imported = std::fs::File::open(file).map_err(|_| "Could not read ico.ini").and_then(|file| {
        use std::io::Read;
        let mut bytes = Vec::new();
        file.take(256 * 1024 + 1).read_to_end(&mut bytes).map_err(|_| "Could not read ico.ini")?;
        crate::data::desktop_dock::import_mydock(&bytes)
    });
    with_app(|a| match imported {
        Ok(pins) => {
            let mut count = 0;
            for pin in pins {
                count += usize::from(a.settings.desktop_dock.add(pin));
            }
            a.ui.sw.dock_status = format!("{}: {count}", tr("Imported applications"));
            a.save_desktop_dock();
        }
        Err(message) => {
            a.ui.sw.dock_status = tr(message).into_owned();
            a.render_settings_win();
        }
    });
}

pub(in crate::island) fn handle_request(hwnd: HWND, command: usize) {
    if command == 1 {
        add_file(hwnd);
        return;
    }
    if command == 2 {
        import_dock(hwnd);
        return;
    }
    let mut request = None;
    with_app(|a| request = a.desktop_dock.as_mut().and_then(Dock::take_request));
    match request {
        Some(Request::Save) => with_app(|a| {
            if let Some(dock) = &a.desktop_dock {
                a.settings.desktop_dock = dock.config();
            }
            a.save_desktop_dock();
        }),
        Some(Request::Add) => add_file(hwnd),
        Some(Request::Launch(entry)) => {
            let live: Vec<_> = desktop::windows()
                .into_iter()
                .filter(|w| entry.windows.iter().any(|old| old.handle == w.handle && key(&old.path) == key(&w.path)))
                .collect();
            if live.is_empty() {
                cmd::shell_open(&entry.pin.target, None);
            } else {
                let index = live.iter().position(|w| w.active).map_or(0, |i| (i + 1) % live.len());
                desktop::activate(&live[index], live.len() == 1);
            }
        }
        Some(Request::Menu(entry)) => {
            use crate::ui::menu::{self, Entry};
            let mut items = Vec::new();
            if let Some(entry) = &entry {
                items.push(Entry::item(1, &tr("Open new window")));
                items.push(Entry::item(
                    2,
                    &tr(if entry.pinned { "Unpin from desktop dock" } else { "Pin to desktop dock" }),
                ));
                if entry.pinned {
                    items.push(Entry::item(3, &tr("Move left")));
                    items.push(Entry::item(4, &tr("Move right")));
                }
                if !entry.windows.is_empty() {
                    items.push(Entry::divider());
                    for (i, window) in entry.windows.iter().take(32).enumerate() {
                        items.push(Entry::item(100 + i, &window.title));
                    }
                }
                items.push(Entry::divider());
            }
            items.push(Entry::item(5, &tr("Add application or file")));
            items.push(Entry::item(6, &tr("Desktop dock settings")));
            let mut point = windows::Win32::Foundation::POINT::default();
            unsafe {
                let _ = GetCursorPos(&mut point);
            }
            let scale = with_app_value(|a| a.dpi_scale).unwrap_or(1.0);
            let chosen = menu::show(items, (point.x, point.y), true, scale, &|f| with_app(|a| f(&a.gpu)));
            with_app(|a| {
                if let Some(dock) = &mut a.desktop_dock {
                    dock.close_menu();
                }
            });
            match chosen {
                Some(1) => {
                    if let Some(entry) = entry {
                        cmd::shell_open(&entry.pin.target, None);
                    }
                }
                Some(2..=4) => {
                    if let Some(entry) = entry {
                        with_app(|a| {
                            let pins = &mut a.settings.desktop_dock.pins;
                            let index = pins.iter().position(|p| key(&p.target) == key(&entry.pin.target));
                            match (chosen, index) {
                                (Some(2), Some(i)) => {
                                    pins.remove(i);
                                }
                                (Some(2), None) => {
                                    a.settings.desktop_dock.add(entry.pin);
                                }
                                (Some(3), Some(i)) if i > 0 => pins.swap(i, i - 1),
                                (Some(4), Some(i)) if i + 1 < pins.len() => pins.swap(i, i + 1),
                                _ => {}
                            }
                            a.save_desktop_dock();
                        });
                    }
                }
                Some(5) => add_file(hwnd),
                Some(6) => with_app(|a| {
                    if let Some(group) = view::GROUPS.iter().position(|g| g.name == "Desktop dock") {
                        a.open_settings_at(group);
                    }
                }),
                Some(id) if id >= 100 => {
                    if let Some(window) = entry.as_ref().and_then(|e| e.windows.get(id - 100)) {
                        desktop::activate(window, false);
                    }
                }
                _ => {}
            }
        }
        None => {}
    }
}
