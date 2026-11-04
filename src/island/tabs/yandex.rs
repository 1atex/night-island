use crate::island::*;
use crate::services::yandex::{self, Command};

impl App {
    pub(in crate::island) fn on_yandex(&mut self) {
        let next = yandex::snapshot();
        if next.code != self.ui.yandex.state.code {
            self.ui.yandex.copied = false;
        }
        self.ui.yandex.state = next;
        self.on_media();
        if self.ui.tab == view::Tab::Yandex {
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn click_yandex(&mut self, x: i32, y: i32) {
        use view::YandexHit as Hit;
        let hit = view::hit_yandex(self.pill_rect(true), self.scale, &self.ui.yandex, x as f32, y as f32);
        let command = match hit {
            Some(Hit::Connect) => Command::Connect,
            Some(Hit::Cancel) => Command::Cancel,
            Some(Hit::Disconnect) => Command::Disconnect,
            Some(Hit::Toggle) => Command::Toggle,
            Some(Hit::Previous) => Command::Previous,
            Some(Hit::Next) => Command::Next,
            Some(Hit::Stop) => Command::Stop,
            Some(Hit::Retry) => Command::Retry,
            Some(Hit::Autoplay) => Command::Autoplay(!self.ui.yandex.state.config.autoplay),
            Some(Hit::Filter(index, dir)) => {
                let Some(group) = self.ui.yandex.state.groups.get(index) else { return };
                let current = self
                    .ui
                    .yandex
                    .state
                    .config
                    .selection
                    .get(&group.key)
                    .and_then(|seed| group.items.iter().position(|i| &i.1 == seed))
                    .map_or(0, |i| i + 1);
                let next = (current as i32 + dir).rem_euclid(group.items.len() as i32 + 1) as usize;
                Command::Select(group.key.clone(), next.checked_sub(1).map(|i| group.items[i].1.clone()))
            }
            Some(Hit::Browser) => {
                cmd::shell_open("https://ya.ru/device", None);
                return;
            }
            Some(Hit::Copy) => {
                self.ui.yandex.copied = clips::sys::write_text(self.hwnd, &self.ui.yandex.state.code);
                self.repaint_expanded();
                return;
            }
            None => return,
        };
        yandex::send(command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Renders synthetic My Wave states using Windows Composition"]
    fn my_wave_native_preview() -> Result<()> {
        unsafe {
            RoInitialize(RO_INIT_SINGLETHREADED)?;
        }
        let _queue = unsafe {
            CreateDispatcherQueueController(DispatcherQueueOptions {
                dwSize: size_of::<DispatcherQueueOptions>() as u32,
                threadType: DQTYPE_THREAD_CURRENT,
                apartmentType: DQTAT_COM_NONE,
            })?
        };
        let compositor = windows::UI::Composition::Compositor::new()?;
        let gpu = crate::ui::gfx::Gpu::new(&compositor)?;
        let surface = crate::ui::popup::PopupSurface::new((1320, 820))?;
        let mut ui = view::UiState::new();
        ui.tab = view::Tab::Yandex;
        ui.cfg.header_tabs = vec!["My Wave".into(), "Weather".into(), "App mixer".into()];
        use crate::services::yandex::Group;
        use crate::ui::gfx::rgba;
        let groups = vec![
            Group {
                key: "context".into(),
                name: "Wave activity".into(),
                items: vec![("Работаю".into(), "activity:work-background".into())],
            },
            Group {
                key: "diversity".into(),
                name: "Variety".into(),
                items: vec![("Незнакомое".into(), "settingDiversity:discover".into())],
            },
            Group {
                key: "moodEnergy".into(),
                name: "Mood".into(),
                items: vec![("Спокойное".into(), "settingMoodEnergy:calm".into())],
            },
            Group {
                key: "language".into(),
                name: "Language".into(),
                items: vec![("Без слов".into(), "settingLanguage:without-words".into())],
            },
        ];
        surface.paint(&gpu, |p| {
            p.fill_rect((0.0, 0.0, 1320.0, 820.0), rgba(0.025, 0.035, 0.06, 1.0))?;
            for (i, (x, y)) in [(10.0, 10.0), (670.0, 10.0), (10.0, 420.0), (670.0, 420.0)].into_iter().enumerate() {
                ui.yandex.state = Default::default();
                if i == 1 {
                    ui.yandex.state.code = "A1B2-C3D4".into();
                }
                if i >= 2 {
                    ui.yandex.state.account = "Музыка для тебя".into();
                    ui.yandex.state.groups = groups.clone();
                    ui.yandex.state.config.selection.insert("moodEnergy".into(), "settingMoodEnergy:calm".into());
                    ui.yandex.state.config.selection.insert("language".into(), "settingLanguage:without-words".into());
                }
                if i == 3 {
                    ui.yandex.state.active = true;
                    ui.yandex.state.media.playing = true;
                    ui.yandex.state.media.title = "Ночной город".into();
                    ui.yandex.state.media.artist = "Любимый исполнитель".into();
                    ui.yandex.state.error = "Could not connect to Yandex Music. Try again.".into();
                }
                p.fill_round_rect((x, y, x + 640.0, y + 384.0), 28.0, rgba(0.0, 0.0, 0.0, 1.0))?;
                view::paint_expanded(p, (x, y, 640.0, 384.0), 1.0, &ui, &[])?;
            }
            Ok(())
        })?;
        if let Some(path) = std::env::var_os("NI_WAVE_PREVIEW") {
            let rgba: Vec<u8> = surface.pixels().chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
            let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), 1320, 820);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.write_header().unwrap().write_image_data(&rgba).unwrap();
        }
        Ok(())
    }
}
