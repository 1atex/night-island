use crate::island::*;

pub(in crate::island) const NOTE_SAVE_MS: u32 = 800;
const DELETE_ARM_MS: u32 = 3000;

impl App {
    pub(in crate::island) fn remind_active(&self) -> bool {
        self.ui.tab == view::Tab::Remind
    }

    pub(in crate::island) fn rename_active(&self) -> bool {
        self.ui.tab == view::Tab::Note && self.ui.notes.renaming.is_some() && self.ui.editing
    }

    pub(in crate::island) fn note_inner(&self) -> view::Rect {
        self.field_frame().1
    }

    pub(in crate::island) fn note_layout(&self) -> Option<windows::Win32::Graphics::DirectWrite::IDWriteTextLayout> {
        let inner = self.note_inner();
        let one_line = self.remind_active() || self.rename_active();
        crate::ui::gfx::make_layout(
            self.gpu.dwrite(),
            &self.multi().text(),
            if one_line { 100_000.0 } else { inner.2 - inner.0 },
            if self.remind_active() {
                13.5
            } else if self.rename_active() {
                13.0
            } else if self.ui.tab == view::Tab::Agy {
                view::AGY_FONT
            } else {
                view::NOTE_FONT
            } * self.scale,
        )
        .ok()
    }

    pub(in crate::island) fn save_note(&self) {
        self.kill_timer(Timer::NoteSave);
        if let (Some(store), Some(id)) = (&self.store, self.ui.notes.current) {
            store.note_set_text(id, &self.ui.note.text());
        }
    }

    pub(in crate::island) fn note_changed(&mut self) {
        if self.extra_field().is_some() {
            self.set_timer(Timer::Geo, 450);
        }
        if self.extra_field().is_some() {
            self.ui.caret_on = true;
            self.set_timer(Timer::Caret, CARET_BLINK_MS);
            return self.repaint_expanded();
        }
        if self.clip_search() {
            self.ui.caret_on = true;
            self.set_timer(Timer::Caret, CARET_BLINK_MS);
            self.ui.clip.scroll = 0;
            self.apply_clip_filter();
            return self.repaint_expanded();
        }
        if self.ui.cmd.open {
            self.ui.caret_on = true;
            self.set_timer(Timer::Caret, CARET_BLINK_MS);
            self.refresh_cmd();
            return self.repaint_expanded();
        }
        if self.ui.tab == view::Tab::Note && self.ui.notes.renaming.is_none() {
            self.set_timer(Timer::NoteSave, NOTE_SAVE_MS);
        }
        self.ui.caret_on = true;
        if self.ui.editing {
            self.set_timer(Timer::Caret, CARET_BLINK_MS);
        }
        self.ensure_caret_visible();
        self.repaint_expanded();
    }

    pub(in crate::island) fn init_notes(&mut self) {
        self.reload_notes();
        let last = self.store.as_ref().and_then(Store::note_last);
        let first_sheet = self.ui.notes.items.iter().find(|i| !i.folder).map(|i| i.id);
        let pick = last.filter(|id| self.ui.notes.item(*id).is_some_and(|i| !i.folder)).or(first_sheet);
        match pick {
            Some(id) => self.open_note(id),
            None => self.ensure_sheet(),
        }
    }

    pub(in crate::island) fn reload_notes(&mut self) {
        let items = self.store.as_ref().map(Store::note_items).unwrap_or_default();
        let n = &mut self.ui.notes;
        n.items = items;
        let ids: std::collections::HashSet<i64> = n.items.iter().map(|i| i.id).collect();
        n.expanded.retain(|id| ids.contains(id));
        n.sel = n.sel.filter(|id| ids.contains(id));
        n.current = n.current.filter(|id| ids.contains(id));
        let rows = n.rows().len();
        n.scroll = n.scroll.min(rows.saturating_sub(1));
    }

    fn ensure_sheet(&mut self) {
        let existing = self.ui.notes.items.iter().find(|i| !i.folder).map(|i| i.id);
        let id = existing.or_else(|| self.store.as_ref()?.note_add(None, false, &crate::ui::i18n::tr("New note")));
        self.reload_notes();
        if let Some(id) = id {
            self.open_note(id);
        }
    }

    pub(in crate::island) fn open_note(&mut self, id: i64) {
        if self.ui.notes.current != Some(id) {
            self.save_note();
        }
        let Some(store) = &self.store else { return };
        let text = store.note_text(id);
        store.note_set_last(id);
        self.ui.note.set_text(&text);
        self.ui.notes.current = Some(id);
        self.ui.notes.sel = Some(id);
        self.ui.notes.armed_delete = false;
        self.ui.notes.reveal(id);
        self.ensure_row_visible(id);
        self.repaint_expanded();
    }

    fn ensure_row_visible(&mut self, id: i64) {
        let fit = view::tree_rows_fit(self.pill_rect(true), self.scale);
        let Some(index) = self.ui.notes.rows().iter().position(|(_, i)| i.id == id) else { return };
        let n = &mut self.ui.notes;
        if index < n.scroll {
            n.scroll = index;
        } else if index >= n.scroll + fit {
            n.scroll = index + 1 - fit;
        }
    }

    fn add_note_item(&mut self, folder: bool) {
        let parent = self.ui.notes.target_folder();
        let name = crate::ui::i18n::tr(if folder { "New folder" } else { "New note" });
        let Some(id) = self.store.as_ref().and_then(|s| s.note_add(parent, folder, &name)) else {
            return self.notify(Notice::Error, '\u{E783}', "Notes limit reached");
        };
        if let Some(p) = parent {
            self.ui.notes.expanded.insert(p);
        }
        self.reload_notes();
        if folder {
            self.ui.notes.sel = Some(id);
        } else {
            self.open_note(id);
        }
        self.ensure_row_visible(id);
        self.begin_rename(id);
    }

    pub(in crate::island) fn begin_rename(&mut self, id: i64) {
        self.finish_rename(true);
        let Some(name) = self.ui.notes.item(id).map(|i| i.name.clone()) else { return };
        self.ui.notes.name_input.set_text(&name);
        self.ui.notes.name_input.select_all();
        self.ui.notes.renaming = Some(id);
        self.begin_edit();
        self.ui.caret_on = true;
        self.repaint_expanded();
    }

    pub(in crate::island) fn finish_rename(&mut self, apply: bool) {
        let Some(id) = self.ui.notes.renaming.take() else { return };
        if apply {
            let name = self.ui.notes.name_input.text();
            if let Some(store) = &self.store {
                store.note_rename(id, &name);
            }
            self.reload_notes();
        }
        self.repaint_expanded();
    }

    fn delete_selected(&mut self) {
        let Some(id) = self.ui.notes.sel else { return };
        if !self.ui.notes.armed_delete {
            self.ui.notes.armed_delete = true;
            self.set_timer(Timer::Disarm, DELETE_ARM_MS);
            return;
        }
        self.ui.notes.armed_delete = false;
        let current_gone = self.ui.notes.current.is_some_and(|c| {
            let mut at = Some(c);
            let mut hops = 0;
            while let Some(i) = at {
                if i == id {
                    return true;
                }
                hops += 1;
                if hops > crate::data::notes::MAX_DEPTH + 2 {
                    break;
                }
                at = self.ui.notes.item(i).and_then(|n| n.parent);
            }
            false
        });
        if let Some(store) = &self.store {
            store.note_delete(id);
        }
        self.ui.notes.sel = None;
        self.reload_notes();
        if current_gone {
            self.ui.notes.current = None;
            self.ui.note.set_text("");
            self.ensure_sheet();
        }
        self.repaint_expanded();
    }

    fn note_tree_click(&mut self, hit: view::NoteHit) {
        if hit != view::NoteHit::Delete {
            self.ui.notes.armed_delete = false;
        }
        match hit {
            view::NoteHit::Row(index) => {
                let Some(id) = self.ui.notes.rows().get(index).map(|(_, i)| i.id) else { return };
                self.ui.notes.sel = Some(id);
                self.ui.notes.drag_from = Some(id);
                unsafe {
                    windows::Win32::UI::Input::KeyboardAndMouse::SetCapture(self.hwnd);
                }
            }
            view::NoteHit::AddNote => self.add_note_item(false),
            view::NoteHit::AddFolder => self.add_note_item(true),
            view::NoteHit::Rename => {
                if let Some(id) = self.ui.notes.sel {
                    self.begin_rename(id);
                }
            }
            view::NoteHit::Delete => self.delete_selected(),
        }
        self.repaint_expanded();
    }

    pub(in crate::island) fn click_note(&mut self, x: i32, y: i32) {
        self.ui.notes.drag_from = None;
        let pill = self.pill_rect(true);
        let (fx, fy) = (x as f32, y as f32);
        let inside = |r: view::Rect| fx >= r.0 && fx < r.2 && fy >= r.1 && fy < r.3;
        if self.rename_active() {
            if inside(self.field_frame().0) {
                return self.click_in_field(fx, fy);
            }
            self.finish_rename(true);
            self.end_edit(true);
        }
        if let Some(hit) = view::hit_note_tree(pill, self.scale, fx, fy, &self.ui) {
            return self.note_tree_click(hit);
        }
        if !inside(view::note_frame(pill, self.scale).0) {
            self.end_edit(true);
            return;
        }
        self.click_in_field(fx, fy);
    }

    pub(in crate::island) fn click_in_field(&mut self, fx: f32, fy: f32) {
        let (_, inner) = self.field_frame();
        self.begin_edit();
        let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
        if let Some(layout) = self.note_layout() {
            let scroll = self.multi().scroll;
            self.multi_mut().click(&layout, fx - inner.0, fy - inner.1 + scroll, shift);
        }
        self.pointer.selecting = true;
        self.note_changed();
    }

    pub(in crate::island) fn finish_note_drag(&mut self) {
        let Some(from_id) = self.ui.notes.drag_from.take() else { return };
        unsafe {
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
        }
        if self.state != State::Expanded || self.ui.tab != view::Tab::Note {
            return;
        }
        let pill = self.pill_rect(true);
        let Some((rx, ry)) = self.cursor_client() else { return };
        let to = view::hit_note_tree(pill, self.scale, rx, ry, &self.ui);
        let rows = self.ui.notes.rows();
        let Some(from) = rows.iter().position(|(_, i)| i.id == from_id) else { return };
        let (_, moved) = rows[from];
        if to == Some(view::NoteHit::Row(from)) {
            let (id, folder) = (moved.id, moved.folder);
            if folder {
                if !self.ui.notes.expanded.remove(&id) {
                    self.ui.notes.expanded.insert(id);
                }
                self.repaint_expanded();
            } else {
                self.open_note(id);
            }
            return;
        }
        let target: Option<i64> = match to {
            Some(view::NoteHit::Row(j)) => match rows.get(j) {
                Some(&(_, t)) if t.folder => Some(t.id),
                Some(&(_, t)) => t.parent,
                None => return,
            },
            Some(_) => return,
            None => {
                let tree = view::note_tree(pill, self.scale);
                if !(rx >= tree.0 && rx < tree.2 && ry >= tree.1 && ry < tree.3) {
                    return;
                }
                None
            }
        };
        let moved_ok = self.store.as_ref().is_some_and(|s| s.note_move(from_id, target));
        if moved_ok {
            if let Some(t) = target {
                self.ui.notes.expanded.insert(t);
            }
            self.reload_notes();
            self.ui.notes.sel = Some(from_id);
            self.ensure_row_visible(from_id);
        }
        self.repaint_expanded();
    }

    pub(in crate::island) fn scroll_tree(&mut self, delta: i32) {
        let rows = self.ui.notes.rows().len();
        let max = rows.saturating_sub(view::tree_rows_fit(self.pill_rect(true), self.scale));
        let n = &mut self.ui.notes;
        let next = if delta > 0 { n.scroll.saturating_sub(1) } else { (n.scroll + 1).min(max) };
        if next != n.scroll {
            n.scroll = next;
            n.hover = None;
            self.repaint_expanded();
        }
    }

    pub(in crate::island) fn cursor_client(&self) -> Option<(f32, f32)> {
        let mut pt = windows::Win32::Foundation::POINT::default();
        unsafe {
            if GetCursorPos(&mut pt).is_err()
                || !windows::Win32::Graphics::Gdi::ScreenToClient(self.hwnd, &mut pt).as_bool()
            {
                return None;
            }
        }
        Some((pt.x as f32, pt.y as f32))
    }

    pub(in crate::island) fn cursor_over_tree(&self) -> bool {
        if self.state != State::Expanded || self.ui.tab != view::Tab::Note {
            return false;
        }
        let Some((x, y)) = self.cursor_client() else { return false };
        let tree = view::note_tree(self.pill_rect(true), self.scale);
        x >= tree.0 && x < tree.2 && y >= tree.1 && y < tree.3
    }

    pub(in crate::island) fn cursor_over_note(&self) -> bool {
        if self.state != State::Expanded || !matches!(self.ui.tab, view::Tab::Note | view::Tab::Remind) {
            return false;
        }
        let Some((x, y)) = self.cursor_client() else { return false };
        let pill = self.pill_rect(true);
        let frame = if self.ui.tab == view::Tab::Note {
            view::note_frame(pill, self.scale).0
        } else {
            view::remind_frame(pill, self.scale).0
        };
        x >= frame.0 && x < frame.2 && y >= frame.1 && y < frame.3
    }
}
