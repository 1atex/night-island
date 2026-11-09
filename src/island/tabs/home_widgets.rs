use crate::data::home_widgets::WidgetSize;
use crate::island::*;

impl App {
    pub(in crate::island) fn toggle_widget_editor(&mut self) {
        self.cancel_widget_drag();
        self.close_home_mixer();
        self.ui.home.widgets.editing = !self.ui.home.widgets.editing;
        self.ui.home.widgets.editor_motion = (self.settings.island_animations && crate::ui::feedback::motion_allowed())
            .then(|| (self.ui.home.widgets.editing, Instant::now()));
        if self.ui.home.widgets.editing {
            self.ui.home.widgets.selected = self.settings.home_actions.first().copied().or_else(|| {
                (self.settings.home_weather_widget && self.settings.weather && self.ui.weather.is_some())
                    .then_some(view::weather_widget_id())
            });
        }
        self.ui.home.widgets.status.clear();
        self.ui.home.edit = false;
        self.ui.home.dev_menu = None;
        self.ui.home.dock_open = false;
        self.ui.home.tile_press = None;
        self.ui.home.tile_reorder = None;
        self.repaint_expanded();
    }

    pub(in crate::island) fn widget_control(&mut self, control: view::WidgetControl) {
        use view::WidgetControl::*;
        match control {
            Done => {
                self.toggle_widget_editor();
                return;
            }
            Add => {
                self.ui.home.edit = !self.ui.home.edit;
                self.ui.home.edit_scroll = 0;
            }
            Size(size) => {
                if let Some(id) = self.ui.home.widgets.selected {
                    self.set_widget_size(id, size);
                    return;
                }
            }
            Remove => {
                if let Some(id) = self.ui.home.widgets.selected.take() {
                    self.settings.home_actions.retain(|i| *i != id);
                    self.settings.home_widget_sizes.remove(&id);
                    if id == view::weather_widget_id() {
                        self.settings.home_weather_widget = false;
                    }
                    self.save_widget_layout();
                    return;
                }
            }
        }
        self.ui.home.widgets.status.clear();
        self.repaint_expanded();
    }

    pub(in crate::island) fn begin_widget_drag(&mut self, id: u8, x: i32, y: i32) {
        let Some((_, rect, _)) =
            view::home_widget_rects(self.pill_rect(true), self.scale, &self.ui).into_iter().find(|v| v.0 == id)
        else {
            return;
        };
        self.ui.home.widgets.selected = Some(id);
        self.ui.home.widgets.press = Some((id, x, y, rect));
        self.ui.home.widgets.status.clear();
        self.kill_timer(Timer::Collapse);
        unsafe {
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(self.hwnd);
        }
        self.repaint_expanded();
    }

    pub(in crate::island) fn drag_widget(&mut self, x: i32, y: i32) -> bool {
        let Some((id, start_x, start_y, origin)) = self.ui.home.widgets.press else { return false };
        if self.ui.home.widgets.drag.is_none() {
            if ((x - start_x).abs().max((y - start_y).abs()) as f32) < 6.0 * self.scale {
                return true;
            }
            let order = view::home_widget_targets(self.pill_rect(true), self.scale, &self.ui)
                .into_iter()
                .map(|v| v.0)
                .collect();
            self.ui.home.widgets.drag = Some(view::WidgetDrag {
                id,
                offset: (start_x as f32 - origin.0, start_y as f32 - origin.1),
                cursor: (x as f32, y as f32),
                origin,
                order,
                valid: true,
            });
        }
        let pill = self.pill_rect(true);
        let targets = view::home_widget_targets(pill, self.scale, &self.ui);
        let area = view::home_widget_area(pill, self.scale);
        let margin = 12.0 * self.scale;
        let valid = x as f32 >= area.0 - margin
            && x as f32 <= area.2 + margin
            && y as f32 >= area.1 - margin
            && y as f32 <= area.3 + margin;
        let mut drag = self.ui.home.widgets.drag.take().unwrap();
        drag.cursor = (x as f32, y as f32);
        drag.valid = valid;
        let r = drag.rect();
        let (cx, cy) = ((r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0);
        let original = drag.order.clone();
        self.ui.home.widgets.drag = Some(drag);
        if valid {
            let mut others = original.clone();
            others.retain(|v| *v != id);
            let distance = |r: view::Rect| ((r.0 + r.2) / 2.0 - cx).powi(2) + ((r.1 + r.3) / 2.0 - cy).powi(2);
            let mut best = targets.iter().find(|v| v.0 == id).map_or(f32::MAX, |v| distance(v.1));
            let mut order = original.clone();
            for index in 0..=others.len() {
                let mut candidate = others.clone();
                candidate.insert(index, id);
                self.ui.home.widgets.drag.as_mut().unwrap().order = candidate.clone();
                if let Some((_, r)) =
                    view::home_widget_targets(pill, self.scale, &self.ui).into_iter().find(|v| v.0 == id)
                {
                    let score = distance(r);
                    if score + (5.0 * self.scale).powi(2) < best {
                        best = score;
                        order = candidate;
                    }
                }
            }
            self.ui.home.widgets.drag.as_mut().unwrap().order = order;
        }
        self.repaint_expanded();
        true
    }

    pub(in crate::island) fn cancel_widget_drag(&mut self) -> bool {
        let pressed = self.ui.home.widgets.press.take().is_some();
        let drag = self.ui.home.widgets.drag.take();
        let cancelled = pressed || drag.is_some();
        if let Some(drag) = drag {
            self.ui.home.widgets.motion.borrow_mut().release_from(drag.id, drag.rect());
        }
        if cancelled {
            self.repaint_expanded();
            self.arm_collapse_if_idle(self.settings.close_delay_ms.max(1));
        }
        cancelled
    }

    pub(in crate::island) fn finish_widget_drag(&mut self) -> bool {
        if self.ui.home.widgets.press.take().is_none() {
            return false;
        }
        let drag = self.ui.home.widgets.drag.take();
        unsafe {
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
        }
        if let Some(drag) = drag {
            self.ui.home.widgets.motion.borrow_mut().release_from(drag.id, drag.rect());
            if drag.valid {
                let weather = view::weather_widget_id();
                if drag.order.contains(&weather) && !self.settings.home_actions.contains(&weather) {
                    self.settings.home_widget_sizes.insert(weather, crate::data::home_widgets::WidgetSize::Wide4);
                }
                self.settings.home_actions = drag.order;
                self.save_widget_layout();
            }
        }
        self.repaint_expanded();
        self.arm_collapse_if_idle(self.settings.close_delay_ms.max(1));
        true
    }

    fn save_widget_layout(&mut self) {
        self.ui.cfg = self.settings.clone();
        self.settings.save();
        self.sync_metrics();
        self.repaint_expanded();
    }

    fn set_widget_size(&mut self, id: u8, size: WidgetSize) {
        if !view::can_resize_widget(&self.ui, id, size) {
            self.ui.home.widgets.status = crate::ui::i18n::tr("Not enough room in the grid").into_owned();
            self.repaint_expanded();
            return;
        }
        if !self.settings.home_actions.contains(&id) {
            self.settings.home_actions.push(id);
        }
        self.settings.home_widget_sizes.insert(id, size);
        self.ui.home.widgets.status.clear();
        self.save_widget_layout();
    }

    pub(in crate::island) fn begin_widget_resize(&mut self, id: u8, x: i32, y: i32) {
        let size = view::widget_size(&self.ui, id);
        self.ui.home.widgets.selected = Some(id);
        self.ui.home.widgets.resize = Some((id, x, y, size));
        self.kill_timer(Timer::Collapse);
        self.ui.home.widgets.status.clear();
        unsafe {
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(self.hwnd);
        }
    }

    pub(in crate::island) fn drag_widget_resize(&mut self, x: i32, y: i32) -> bool {
        let Some((id, from_x, from_y, old)) = self.ui.home.widgets.resize else { return false };
        let (width, height) = old.dimensions();
        let width = (f32::from(width) + (x - from_x) as f32 / (132.0 * self.scale)).round().clamp(1.0, 2.0) as u8;
        let height = (f32::from(height) + (y - from_y) as f32 / (54.0 * self.scale)).round().clamp(1.0, 4.0) as u8;
        let size = match (width, height) {
            (1, 1) => WidgetSize::Tile,
            (1, 2) => WidgetSize::Tall2,
            (1, _) => WidgetSize::Tall4,
            (2, 1) => WidgetSize::Wide2,
            _ => WidgetSize::Wide4,
        };
        if view::can_resize_widget(&self.ui, id, size) {
            if self.ui.home.widgets.preview != Some((id, size)) {
                self.ui.home.widgets.preview = Some((id, size));
                self.ui.home.widgets.status.clear();
                self.repaint_expanded();
            }
        } else {
            self.ui.home.widgets.status = crate::ui::i18n::tr("Not enough room in the grid").into_owned();
            self.repaint_expanded();
        }
        true
    }

    pub(in crate::island) fn cancel_widget_resize(&mut self) -> bool {
        if self.ui.home.widgets.resize.take().is_none() {
            return false;
        }
        self.ui.home.widgets.preview = None;
        self.ui.home.widgets.status.clear();
        self.repaint_expanded();
        true
    }

    pub(in crate::island) fn finish_widget_resize(&mut self) -> bool {
        if self.ui.home.widgets.resize.take().is_none() {
            return false;
        }
        unsafe {
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
        }
        if let Some((id, size)) = self.ui.home.widgets.preview.take() {
            self.set_widget_size(id, size);
        }
        self.repaint_expanded();
        true
    }
}
