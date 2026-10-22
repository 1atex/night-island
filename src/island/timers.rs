use crate::island::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::island) enum Timer {
    Expand = 1,
    Collapse = 2,
    Shrink = 3,
    Autohide = 4,
    HideDone = 5,
    Clock = 6,
    Clip = 7,
    Disarm = 8,
    Paste = 9,
    Liquid = 10,
    Caret = 11,
    NoteSave = 12,
    Volume = 13,
    Notice = 14,
    Spectrum = 15,
    Remind = 16,
    Rec = 17,
    DlStale = 18,
    Char = 19,
    Progress = 20,
    Agy = 21,
    Geo = 22,
    Game = 23,
    Lyric = 24,
    LockKey = 25,
    SwCaret = 26,
    PillBack = 27,
    GhostHold = 28,
    Activity = 29,
    Settle = 30,
}

impl Timer {
    const ALL: [Timer; 30] = [
        Timer::Expand,
        Timer::Collapse,
        Timer::Shrink,
        Timer::Autohide,
        Timer::HideDone,
        Timer::Clock,
        Timer::Clip,
        Timer::Disarm,
        Timer::Paste,
        Timer::Liquid,
        Timer::Caret,
        Timer::NoteSave,
        Timer::Volume,
        Timer::Notice,
        Timer::Spectrum,
        Timer::Remind,
        Timer::Rec,
        Timer::DlStale,
        Timer::Char,
        Timer::Progress,
        Timer::Agy,
        Timer::Geo,
        Timer::Game,
        Timer::Lyric,
        Timer::LockKey,
        Timer::SwCaret,
        Timer::PillBack,
        Timer::GhostHold,
        Timer::Activity,
        Timer::Settle,
    ];

    fn from_id(id: usize) -> Option<Timer> {
        Self::ALL.into_iter().find(|t| *t as usize == id)
    }
}

impl App {
    pub(in crate::island) fn kill_timer(&self, timer: Timer) {
        unsafe {
            let _ = KillTimer(Some(self.hwnd), timer as usize);
        }
    }

    pub(in crate::island) fn set_timer(&self, timer: Timer, ms: u32) {
        unsafe {
            SetTimer(Some(self.hwnd), timer as usize, ms, None);
        }
    }

    pub(in crate::island) fn on_timer(&mut self, id: usize) {
        let Some(timer) = Timer::from_id(id) else { return };
        self.kill_timer(timer);
        match timer {
            Timer::Expand if matches!(self.state, State::Hidden | State::Hiding) => self.reveal(),
            Timer::Expand => self.expand(),
            Timer::Collapse => self.collapse(),
            Timer::Shrink => self.shrink_window(),
            Timer::Autohide => self.hide(),
            Timer::HideDone => self.hide_done(),
            Timer::Liquid => self.liquid_tick(),
            Timer::Caret => self.on_caret_timer(),
            Timer::Spectrum => self.spectrum_tick(),
            Timer::Remind => self.on_remind_timer(),
            Timer::Rec => self.on_rec_timer(),
            Timer::Char => self.char_tick(),
            Timer::Game => self.on_game_timer(),
            Timer::Lyric => self.lyric_tick(),
            Timer::LockKey => self.report_lockkeys(),
            Timer::SwCaret => self.sw_caret_tick(),
            Timer::PillBack => self.restore_pill(),
            Timer::GhostHold => self.ghost_hold_tick(),
            Timer::Activity => self.on_activity_timer(),
            Timer::Settle => self.settle_layers(),
            Timer::Geo => {
                if self.ui.tab == view::Tab::Weather && self.ui.editing {
                    weather::search(self.hwnd, &self.ui.city.input.text());
                }
            }
            Timer::Agy => {
                if self.state == State::Expanded
                    && self.ui.tab == view::Tab::Agy
                    && self.ui.agy.open
                    && self.ui.agy.sel.as_deref().is_some_and(agy::running)
                {
                    self.load_history(true);
                    self.repaint_expanded();
                    self.start_agy_timer();
                }
            }
            Timer::Progress => {
                if self.state == State::Expanded && self.ui.tab == view::Tab::Home {
                    self.repaint_expanded();
                    self.start_progress_timer();
                }
            }
            Timer::DlStale => {
                self.ui.downloading = None;
                self.update_eq();
                self.repaint_collapsed();
            }
            Timer::Notice => {
                self.ui.notice = None;
                self.notice_link = None;
                self.update_eq();
                self.repaint_collapsed();
            }
            Timer::Volume => {
                self.ui.volume = None;
                self.update_eq();
                self.repaint_collapsed();
            }
            Timer::NoteSave => self.save_note(),
            Timer::Clip => self.on_clipboard(),
            Timer::Paste => self.on_paste_timer(),
            Timer::Disarm => {
                self.ui.armed = None;
                self.ui.notes.armed_delete = false;
                self.ui.clip.clear_armed = false;
                self.ui.privacy_log.clear_armed = false;
                self.ui.activity.clear_armed = false;
                self.repaint_expanded();
            }
            Timer::Clock => {
                self.refresh_reminder_labels();
                self.repaint();
                self.schedule_clock();
            }
        }
    }
}
