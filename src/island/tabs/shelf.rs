use crate::island::*;

impl App {
    pub(in crate::island) fn reload_shelf(&mut self) {
        self.shelf = self.store.as_ref().map(Store::shelf).unwrap_or_default();
        let rows = view::clip_rows(self.pill_rect(true), self.scale);
        self.ui.shelf_scroll = self.ui.shelf_scroll.min(self.shelf.len().saturating_sub(rows));
    }

    pub(in crate::island) fn click_shelf(&mut self, x: i32, y: i32) {
        let pill = self.pill_rect(true);
        let visible = self.shelf.len().saturating_sub(self.ui.shelf_scroll);
        let Some(hit) = view::hit_clip(pill, self.scale, x as f32, y as f32, visible) else {
            return;
        };
        match hit {
            view::ClipHit::Delete(i) => {
                if let (Some(item), Some(store)) = (self.shelf.get(self.ui.shelf_scroll + i), &self.store) {
                    store.shelf_remove(item.id);
                }
                self.reload_shelf();
                self.repaint_expanded();
            }
            view::ClipHit::Row(i) | view::ClipHit::Pin(i) => {
                self.pointer.drag_from = Some((self.ui.shelf_scroll + i, x, y));
            }
        }
    }

    pub(in crate::island) fn take_drag(&mut self, x: i32, y: i32, left_down: bool) -> Option<std::path::PathBuf> {
        let (index, x0, y0) = self.pointer.drag_from?;
        if !left_down {
            self.pointer.drag_from = None;
            return None;
        }
        const THRESHOLD: i32 = 6;
        if (x - x0).abs() < THRESHOLD && (y - y0).abs() < THRESHOLD {
            return None;
        }
        self.pointer.drag_from = None;
        let path = std::path::PathBuf::from(&self.shelf.get(index)?.path);
        if path.exists() {
            Some(path)
        } else {
            if let (Some(item), Some(store)) = (self.shelf.get(index), &self.store) {
                store.shelf_remove(item.id);
            }
            self.reload_shelf();
            self.repaint_expanded();
            None
        }
    }

    pub(in crate::island) fn on_dnd_enter(&mut self) {
        self.kill_timer(Timer::Collapse);
        self.expand();
        if self.ui.tab == view::Tab::Chat && self.ui.chat.keyed {
            self.ui.chat.drop_hot = true;
            self.ui.chat.list_open = false;
            return self.repaint_expanded();
        }
        if !matches!(self.ui.tab, view::Tab::Shelf | view::Tab::Home) {
            self.select_tab(view::Tab::Shelf);
        }
    }

    pub(in crate::island) fn on_dnd_leave(&mut self) {
        if std::mem::take(&mut self.ui.chat.drop_hot) {
            self.repaint_expanded();
        }
        if self.state == State::Expanded {
            self.set_timer(Timer::Collapse, 1500);
        }
    }

    pub(in crate::island) fn on_dnd_drop(&mut self) {
        self.react(Reaction::File, 2600);
        if self.ui.tab == view::Tab::Chat && self.ui.chat.keyed {
            self.ui.chat.drop_hot = false;
            self.attach_files(dnd::take_dropped());
            return self.on_dnd_leave();
        }
        let to_dock = self.ui.tab == view::Tab::Home;
        if let Some(store) = &self.store {
            for path in dnd::take_dropped() {
                if to_dock {
                    store.dock_add(&path.to_string_lossy());
                } else {
                    store.shelf_add(&path.to_string_lossy());
                }
            }
        }
        self.ui.shelf_scroll = 0;
        self.reload_shelf();
        self.reload_dock();
        self.repaint_expanded();
        self.on_dnd_leave();
    }
}
