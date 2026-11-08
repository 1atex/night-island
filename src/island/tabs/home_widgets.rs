use crate::data::home_widgets::WidgetSize;
use crate::island::*;

impl App {
    pub(in crate::island) fn toggle_widget_editor(&mut self) {
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
            Move(direction) => {
                if let Some(id) = self.ui.home.widgets.selected {
                    if !self.settings.home_actions.contains(&id) {
                        self.set_widget_size(id, view::widget_size(&self.ui, id));
                    }
                    if let Some(from) = self.settings.home_actions.iter().position(|v| *v == id) {
                        let to = from
                            .saturating_add_signed(isize::from(direction))
                            .min(self.settings.home_actions.len().saturating_sub(1));
                        self.settings.home_actions.swap(from, to);
                        self.save_widget_layout();
                        return;
                    }
                }
            }
        }
        self.ui.home.widgets.status.clear();
        self.repaint_expanded();
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
