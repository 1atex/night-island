use crate::island::*;

impl App {
    pub(in crate::island) fn step_tab_drag(&mut self) -> bool {
        let rects = view::tab_rects(self.pill_full(), self.scale, &self.ui);
        let Some(drag) = self.ui.tab_drag.as_mut() else { return false };
        let slots: Vec<view::Rect> = drag.items.iter().map(|&i| rects[i]).collect();
        let cursor = drag.reorder.cursor;
        let items = drag.items.clone();
        let mut reorder =
            std::mem::replace(&mut drag.reorder, view::Reorder::new(&[(0.0, 0.0, 0.0, 0.0)], 0, (0.0, 0.0)));
        let moving = reorder.step_with(&slots, |r, j| view::tab_drag_target_for(&rects, r, &items, j), cursor);
        if let Some(drag) = self.ui.tab_drag.as_mut() {
            drag.reorder = reorder;
        }
        moving
    }

    pub(in crate::island) fn drag_tabs(&mut self, x: i32, y: i32) {
        if self.state != State::Expanded || !self.settings.tab_drag {
            return;
        }
        let cursor = (x as f32, y as f32);
        if let Some(drag) = self.ui.tab_drag.as_mut() {
            drag.reorder.cursor = cursor;
            self.step_tab_drag();
            self.set_timer(Timer::UiAnim, 16);
            return self.repaint_expanded();
        }
        let Some((i, x0, y0)) = self.ui.tab_press else { return };
        if (x - x0).abs() < 6 && (y - y0).abs() < 6 {
            return;
        }
        let rects = view::tab_rects(self.pill_full(), self.scale, &self.ui);
        let items = view::movable_tabs(&self.ui);
        let Some(from) = items.iter().position(|&k| k == i) else {
            self.ui.tab_press = None;
            return;
        };
        let slots: Vec<view::Rect> = items.iter().map(|&k| rects[k]).collect();
        self.ui.tab_drag = Some(view::TabDrag { reorder: view::Reorder::new(&slots, from, cursor), items });
        unsafe {
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(self.hwnd);
        }
        self.set_timer(Timer::UiAnim, 16);
        self.repaint_expanded();
    }

    pub(in crate::island) fn finish_tab_press(&mut self) {
        let press = self.ui.tab_press.take();
        if let Some(drag) = self.ui.tab_drag.take() {
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
            }
            let r = &drag.reorder;
            if r.to != r.from {
                let mut items = drag.items.clone();
                let moved = items.remove(r.from);
                items.insert(r.to.min(items.len()), moved);
                let hidden: Vec<usize> = view::tab_order(&self.settings)
                    .into_iter()
                    .filter(|i| {
                        !matches!(view::TABS[*i].0, view::Tab::Home | view::Tab::Settings) && !items.contains(i)
                    })
                    .collect();
                self.settings.tab_order =
                    items.into_iter().chain(hidden).map(|i| view::TABS[i].2.to_string()).collect();
                self.ui.cfg = self.settings.clone();
                self.settings.save();
            }
            return self.repaint_expanded();
        }
        if let Some((i, ..)) = press
            && self.state == State::Expanded
            && self.ui.tab != view::TABS[i].0
        {
            self.ui.clip.hover = None;
            self.ui.hover_ctl = None;
            self.disarm();
            let target = self.landing_tab(view::TABS[i].0);
            self.select_tab(target);
        }
    }

    pub(in crate::island) fn landing_tab(&self, entry: view::Tab) -> view::Tab {
        if view::SUBTABS.iter().any(|g| g.0 == entry) {
            let remembered = &self.settings.ai_sub;
            let first = view::SUBTABS.iter().find(|g| g.0 == entry).map_or(entry, |g| g.1[0].0);
            view::tab_from_name(remembered).filter(|t| view::bar_tab(*t) == entry).unwrap_or(first)
        } else {
            entry
        }
    }

    pub(in crate::island) fn remember_tab(&mut self, tab: view::Tab) {
        let name = view::tab_name(tab).to_string();
        let mut changed = self.settings.last_tab != name;
        self.settings.last_tab = name.clone();
        if view::bar_tab(tab) == view::Tab::Ai && self.settings.ai_sub != name {
            self.settings.ai_sub = name;
            changed = true;
        }
        if changed {
            self.ui.cfg = self.settings.clone();
            self.settings.save();
        }
    }

    pub(in crate::island) fn restore_tab(&mut self) {
        if !self.settings.remember_tab {
            return;
        }
        let Some(tab) = view::tab_from_name(&self.settings.last_tab) else { return };
        let entry = view::bar_tab(tab);
        let shown = view::TABS.iter().any(|t| t.0 == entry && view::tab_in_bar(&self.settings, entry, t.2));
        if shown && !matches!(tab, view::Tab::Settings | view::Tab::Ai) {
            self.select_tab(tab);
        }
    }
}
