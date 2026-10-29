use crate::island::*;

impl App {
    pub(in crate::island) fn on_dnd_enter(&mut self) {
        self.kill_timer(Timer::Collapse);
        self.expand();
        if self.ui.tab == view::Tab::Chat && self.ui.chat.keyed {
            self.ui.chat.drop_hot = true;
            self.ui.chat.list_open = false;
            return self.repaint_expanded();
        }
        if self.ui.tab != view::Tab::Home {
            self.select_tab(view::Tab::Home);
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
        if let Some(store) = &self.store {
            for path in dnd::take_dropped() {
                store.dock_add(&path.to_string_lossy());
            }
        }
        self.reload_dock();
        self.repaint_expanded();
        self.on_dnd_leave();
    }
}
