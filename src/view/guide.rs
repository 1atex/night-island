use std::time::{Duration, Instant};

use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

use crate::view::*;

const DEMO_TIME: Duration = Duration::from_millis(390);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuideAction {
    Next,
    Back,
    Skip,
    Resume,
    Demo,
    OpenIsland,
    OpenDock,
    OpenTop,
    OpenNotifications,
    OpenAssistant,
    OpenWake,
    SaveWake,
}

#[derive(Default)]
pub struct GuideUi {
    page: usize,
    pub visible: bool,
    expanded: bool,
    transition: Option<(Instant, f32)>,
    pub wake: TextEdit,
    pub wake_feedback: Option<(&'static str, bool)>,
}

impl GuideUi {
    pub fn new(wake: &str) -> Self {
        let mut guide = Self { visible: true, ..Default::default() };
        guide.wake.set_text(wake);
        guide
    }

    pub fn validated_wake(&self) -> Option<String> {
        let wake = crate::tools::voicecmd::clean_wake_setting(&self.wake.text())?;
        let mut seen = std::collections::HashSet::new();
        (!wake.is_empty() && wake.split(", ").all(|name| seen.insert(name.to_lowercase()))).then_some(wake)
    }

    pub fn last_page(&self) -> bool {
        self.page + 1 == STEPS.len()
    }

    pub fn move_by(&mut self, offset: isize) {
        let next = self.page.saturating_add_signed(offset).min(STEPS.len() - 1);
        if next != self.page {
            self.page = next;
            self.expanded = false;
            self.transition = None;
        }
    }

    pub fn play_demo(&mut self, animate: bool) {
        let from = if self.page == 3 { 0.0 } else { self.progress() };
        self.expanded = self.page == 3 || !self.expanded;
        self.transition = animate.then(|| (Instant::now(), from));
    }

    fn progress(&self) -> f32 {
        let target = f32::from(self.expanded);
        self.transition.map_or(target, |(started, from)| {
            let t = (started.elapsed().as_secs_f32() / DEMO_TIME.as_secs_f32()).min(1.0);
            from + (target - from) * t * t * (3.0 - 2.0 * t)
        })
    }

    pub fn animating(&self) -> bool {
        self.visible && self.transition.is_some_and(|(started, _)| started.elapsed() < DEMO_TIME)
    }
}

struct Step {
    section: &'static str,
    title: &'static str,
    summary: &'static str,
    features: [(char, &'static str, &'static str); 3],
}

const STEPS: [Step; 6] = [
    Step {
        section: "The island",
        title: "Meet your island",
        summary: "A small panel at the edge of the screen that opens when you need it.",
        features: [
            ('\u{E8A7}', "Open on hover", "Move the pointer over the island to open it. Move away to close it."),
            ('\u{E718}', "Keep it open", "Use the pin when you want the panel to stay open."),
            ('\u{E713}', "Make it yours", "Adjust the position, shape and animation in Settings."),
        ],
    },
    Step {
        section: "Everyday tools",
        title: "The tools you use daily",
        summary: "Start on Home. Open More to find the rest of your tools and change the tab bar.",
        features: [
            ('\u{E189}', "Music and sound", "Control playback, seek through a track and adjust app volume."),
            ('\u{E77F}', "Clipboard and notes", "Find something you copied or jot down a reminder."),
            ('\u{E713}', "Your shortcuts", "Edit the tiles on Home to keep the actions you use within reach."),
        ],
    },
    Step {
        section: "Desktop",
        title: "Organize your desktop",
        summary: "Choose what sits around the island, from favorite apps to the system tray.",
        features: [
            ('\u{E7F4}', "Bottom dock", "Pin and reorder apps. Hover to preview their windows."),
            ('\u{E8A9}', "Top panel", "Choose the tray, active app and system controls you want to see."),
            ('\u{E9D9}', "Screen edges", "Set panels to appear on hover and hide them in fullscreen apps."),
        ],
    },
    Step {
        section: "Notifications",
        title: "Notifications at a glance",
        summary: "Windows notifications can appear as animated cards beside the island.",
        features: [
            ('\u{EA8F}', "Connect Windows notifications", "Open the Notifications tab to grant access."),
            ('\u{E8A7}', "Read at your pace", "Hover to pause the card, then open the full message."),
            ('\u{E7ED}', "Choose where alerts appear", "Use Windows Do not disturb to quiet system banners."),
        ],
    },
    Step {
        section: "Voice assistant",
        title: "Talk to the Assistant",
        summary: "Ask a question, control music or set a reminder. Start with a model and speech recognition.",
        features: [
            ('\u{E99A}', "Connect a model", "Add a model provider in Settings, under Assistant."),
            ('\u{E720}', "Set up speech", "Choose how speech is recognized and how answers are spoken."),
            ('\u{EA18}', "Choose permissions", "Decide which actions run directly and which ask first."),
        ],
    },
    Step {
        section: "Pet and wake words",
        title: "Give your pet a name",
        summary: "Your pet can sit on the island or join you on the desktop.",
        features: [
            ('\u{E7C9}', "Bring your pet out", "Drag it away from the island. Drag it back to dock it again."),
            (
                '\u{E8F2}',
                "Talk to your pet",
                "Use the pet’s voice action or say its wake word when listening is enabled.",
            ),
            (
                '\u{E720}',
                "Pick two distinct words",
                "Use short names that sound different. Either one can wake the Assistant.",
            ),
        ],
    },
];

#[derive(Clone, Copy)]
struct Layout {
    x: f32,
    y: f32,
    scale: f32,
}

impl Layout {
    fn new((w, h): (f32, f32), dpi: f32) -> Self {
        let available = (h - 40.0 * dpi).max(1.0);
        let scale = (w / 940.0).min(available / 640.0).min(dpi).max(0.001);
        Self { x: (w - 940.0 * scale) / 2.0, y: h - 680.0 * scale - (available - 640.0 * scale) / 2.0, scale }
    }

    fn rect(self, (l, t, r, b): Rect) -> Rect {
        (self.x + l * self.scale, self.y + t * self.scale, self.x + r * self.scale, self.y + b * self.scale)
    }
}

fn controls(guide: &GuideUi) -> Vec<(GuideAction, &'static str, Rect)> {
    use GuideAction::*;
    let mut buttons = vec![
        (Skip, "Skip tour", (48.0, 601.0, 176.0, 647.0)),
        (Next, if guide.last_page() { "Finish tour" } else { "Next" }, (734.0, 601.0, 892.0, 647.0)),
    ];
    if guide.page > 0 {
        buttons.push((Back, "Back", (614.0, 601.0, 722.0, 647.0)));
    }
    let wide = (453.0, 490.0, 865.0, 535.0);
    let split = |a, b| [(a, (453.0, 490.0, 653.0, 535.0)), (b, (665.0, 490.0, 865.0, 535.0))];
    match guide.page {
        0 => buttons.push((Demo, if guide.expanded { "Collapse the example" } else { "Open the example" }, wide)),
        1 => buttons.push((OpenIsland, "Open Home", wide)),
        2 => buttons.extend(
            split((OpenDock, "Dock settings"), (OpenTop, "Top panel settings")).map(|((a, label), r)| (a, label, r)),
        ),
        3 => buttons.extend(
            split((Demo, "Show example"), (OpenNotifications, "Open Notifications"))
                .map(|((a, label), r)| (a, label, r)),
        ),
        4 => buttons.push((OpenAssistant, "Set up the Assistant", wide)),
        _ => {
            buttons.push((
                Demo,
                if guide.expanded { "Dock the example pet" } else { "Release the example pet" },
                (453.0, 273.0, 651.0, 315.0),
            ));
            buttons.extend(
                split((SaveWake, "Save wake words"), (OpenWake, "Voice activation settings"))
                    .map(|((a, label), r)| (a, label, r)),
            );
        }
    }
    buttons
}

pub fn guide_hit(size: (f32, f32), dpi: f32, guide: &GuideUi, x: f32, y: f32) -> Option<GuideAction> {
    let layout = Layout::new(size, dpi);
    controls(guide).into_iter().find(|(_, _, rect)| inside(layout.rect(*rect), x, y)).map(|(action, _, _)| action)
}

pub fn guide_wake_rects(size: (f32, f32), dpi: f32, guide: &GuideUi) -> Option<(Rect, Rect)> {
    if !guide.last_page() {
        return None;
    }
    let layout = Layout::new(size, dpi);
    Some((layout.rect((453.0, 375.0, 865.0, 420.0)), layout.rect((467.0, 375.0, 851.0, 420.0))))
}

pub fn guide_button(p: &Painter, r: Rect, s: f32, label: &str, primary: bool, hover: bool) -> Result<()> {
    let fill = if primary {
        acc(if hover { 0.51 } else { 0.37 })
    } else {
        rgba(1.0, 1.0, 1.0, if hover { 0.13 } else { 0.067 })
    };
    p.fill_round_rect(r, 12.0 * s, fill)?;
    if primary {
        p.stroke_round_rect(r, 12.0 * s, s, acc(0.78))?;
    }
    p.text(label, r, 13.5 * s, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER, WHITE)
}

struct Preview<'a> {
    p: &'a Painter<'a>,
    layout: Layout,
}

impl Preview<'_> {
    fn round(&self, rect: Rect, radius: f32, color: D2D1_COLOR_F) -> Result<()> {
        self.p.fill_round_rect(self.layout.rect(rect), radius * self.layout.scale, color)
    }

    fn text(&self, label: &str, rect: Rect, size: f32, strong: bool, color: D2D1_COLOR_F) -> Result<()> {
        self.p.text(
            label,
            self.layout.rect(rect),
            size * self.layout.scale,
            if strong { DWRITE_FONT_WEIGHT_SEMI_BOLD } else { DWRITE_FONT_WEIGHT_NORMAL },
            DWRITE_TEXT_ALIGNMENT_LEADING,
            color,
        )
    }

    fn icon(&self, glyph: char, rect: Rect, size: f32) -> Result<()> {
        self.p.glyph(glyph, self.layout.rect(rect), size * self.layout.scale, WHITE)
    }

    fn island(&self, x: f32, y: f32, width: f32, height: f32) -> Result<()> {
        self.round((x, y, x + width, y + height), 24.0, rgba(0.015, 0.018, 0.023, 1.0))?;
        self.p.stroke_round_rect(
            self.layout.rect((x, y, x + width, y + height)),
            24.0 * self.layout.scale,
            self.layout.scale,
            rgba(1.0, 1.0, 1.0, 0.13),
        )
    }

    fn pet(&self, x: f32, y: f32, size: f32) -> Result<()> {
        use crate::ui::characters::{Character, Pose, Reaction, draw_with_design};
        let r = self.layout.rect((x, y, x + size, y + size));
        let saved = self.p.push_offset(r.0, r.1);
        let result = draw_with_design(
            self.p,
            Character::Spark,
            size * self.layout.scale,
            &Pose {
                motion: Default::default(),
                t: 0.0,
                reaction: Reaction::Idle,
                look: (0.35, 0.0),
                blink: false,
                accent: crate::ui::theme::accent(),
                beat: 0.0,
            },
            None,
        );
        self.p.set_transform(&saved);
        result
    }

    fn home(&self, x: f32, y: f32) -> Result<()> {
        self.round((x + 12.0, y + 12.0, x + 91.0, y + 39.0), 13.0, rgba(1.0, 1.0, 1.0, 0.15))?;
        self.text("Home", (x + 24.0, y + 12.0, x + 90.0, y + 39.0), 11.5, true, WHITE)?;
        for (i, glyph) in ['\u{E77F}', '\u{E70B}', '\u{F800}', '\u{E99A}', '\u{E718}'].into_iter().enumerate() {
            let l = x + 100.0 + i as f32 * 45.0;
            self.icon(glyph, (l, y + 12.0, l + 34.0, y + 39.0), 13.0)?;
        }
        self.text("More", (x + 331.0, y + 12.0, x + 372.0, y + 39.0), 11.0, true, WHITE)?;
        self.round((x + 14.0, y + 49.0, x + 376.0, y + 158.0), 17.0, acc(0.22))?;
        self.p.fill_round_gradient(
            self.layout.rect((x + 28.0, y + 63.0, x + 81.0, y + 116.0)),
            11.0 * self.layout.scale,
            acc(0.88),
            rgba(0.14, 0.24, 0.45, 1.0),
        )?;
        self.text("Now playing", (x + 96.0, y + 62.0, x + 356.0, y + 82.0), 10.5, false, rgba(1.0, 1.0, 1.0, 0.67))?;
        self.text("Midnight City", (x + 96.0, y + 83.0, x + 356.0, y + 109.0), 16.0, true, WHITE)?;
        self.round((x + 28.0, y + 132.0, x + 358.0, y + 135.0), 1.5, rgba(1.0, 1.0, 1.0, 0.2))?;
        self.round((x + 28.0, y + 132.0, x + 187.0, y + 135.0), 1.5, acc(1.0))?;
        self.round((x + 14.0, y + 168.0, x + 376.0, y + 209.0), 13.0, rgba(1.0, 1.0, 1.0, 0.09))?;
        self.icon('\u{E767}', (x + 23.0, y + 171.0, x + 54.0, y + 207.0), 15.0)?;
        self.round((x + 61.0, y + 187.0, x + 237.0, y + 191.0), 2.0, rgba(1.0, 1.0, 1.0, 0.22))?;
        self.round((x + 61.0, y + 187.0, x + 166.0, y + 191.0), 2.0, rgba(1.0, 1.0, 1.0, 0.9))?;
        self.icon('\u{E70F}', (x + 332.0, y + 171.0, x + 368.0, y + 207.0), 14.0)
    }
}

fn paint_preview(v: &Preview<'_>, guide: &GuideUi) -> Result<()> {
    let p = v.p;
    let l = v.layout;
    let t = guide.progress();
    match guide.page {
        0 | 1 => {
            let t = if guide.page == 1 { 1.0 } else { t };
            let width = 224.0 + 166.0 * t;
            let x = 659.0 - width / 2.0;
            let y = 255.0 - 39.0 * t;
            v.island(x, y, width, 43.0 + 181.0 * t)?;
            if t >= 0.8 {
                p.push_clip(l.rect((x + 1.0, y + 1.0, x + width - 1.0, y + 42.0 + 181.0 * t)));
                let result = v.home(x, y);
                p.pop_clip();
                result?;
            } else {
                v.pet(x + 12.0, y + 6.0, 30.0)?;
                v.icon('\u{F800}', (x + 48.0, y, x + 85.0, y + 43.0), 17.0)?;
                v.text("18°     21:08", (x + 91.0, y, x + 191.0, y + 43.0), 13.0, true, WHITE)?;
            }
        }
        2 => {
            v.island(453.0, 216.0, 92.0, 34.0)?;
            v.island(570.0, 216.0, 178.0, 34.0)?;
            v.island(773.0, 216.0, 92.0, 34.0)?;
            v.text("Explorer", (466.0, 216.0, 539.0, 250.0), 10.5, true, WHITE)?;
            v.text("21:08       18°", (619.0, 216.0, 730.0, 250.0), 12.0, true, WHITE)?;
            v.icon('\u{E701}', (780.0, 216.0, 814.0, 250.0), 12.0)?;
            v.icon('\u{E767}', (813.0, 216.0, 855.0, 250.0), 12.0)?;
            v.round((504.0, 378.0, 815.0, 439.0), 18.0, rgba(0.06, 0.075, 0.12, 0.96))?;
            for (i, glyph) in
                ['\u{E8B7}', '\u{E774}', '\u{E189}', '\u{E8F2}', '\u{E70B}', '\u{E713}'].into_iter().enumerate()
            {
                let x = 516.0 + i as f32 * 48.0;
                v.round((x, 389.0, x + 37.0, 425.0), 9.0, acc(if i == 2 { 0.73 } else { 0.34 }))?;
                v.icon(glyph, (x, 389.0, x + 37.0, 425.0), 17.0)?;
            }
        }
        3 => {
            let width = 191.0 + 169.0 * t;
            let height = 38.0 + 124.0 * t;
            let x = 659.0 - width / 2.0;
            v.island(x, 247.0, width, height)?;
            if t >= 0.7 {
                let record = crate::platform::notifications::Record {
                    id: 0,
                    created: 0,
                    app_id: String::new(),
                    app: "Night Island".into(),
                    title: crate::ui::i18n::tr("Example notification").into_owned(),
                    body: crate::ui::i18n::tr("Your message appears here. Hover to keep it visible.").into_owned(),
                    icon: None,
                    avatar: None,
                };
                let r = l.rect((x, 247.0, x + width, 247.0 + height));
                let _ = paint_notification_card(p, (r.0, r.1, r.2 - r.0, r.3 - r.1), l.scale, &record, 0, false, None)?;
            } else {
                v.text("Night Island", (x + 25.0, 247.0, x + 170.0, 285.0), 12.0, true, WHITE)?;
            }
        }
        4 => {
            v.island(474.0, 218.0, 370.0, 220.0)?;
            v.text("Assistant", (495.0, 232.0, 824.0, 266.0), 15.0, true, WHITE)?;
            for (i, (icon, label)) in [('\u{E720}', "Listening…"), ('\u{E99A}', "Thinking…"), ('\u{E8F2}', "Speaking")]
                .into_iter()
                .enumerate()
            {
                let x = 495.0 + i as f32 * 109.0;
                v.round((x, 278.0, x + 101.0, 346.0), 12.0, acc(if i == 0 { 0.36 } else { 0.13 }))?;
                v.icon(icon, (x + 7.0, 283.0, x + 94.0, 315.0), 15.0)?;
                p.text(
                    label,
                    l.rect((x + 3.0, 315.0, x + 98.0, 341.0)),
                    11.0 * l.scale,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    WHITE,
                )?;
            }
            v.round((495.0, 361.0, 823.0, 416.0), 12.0, rgba(1.0, 1.0, 1.0, 0.09))?;
            v.text("I’ll remind you in ten minutes.", (507.0, 366.0, 812.0, 413.0), 13.0, false, WHITE)?;
        }
        _ => {
            v.island(462.0, 215.0, 195.0, 39.0)?;
            v.text("21:08       18°", (520.0, 215.0, 644.0, 254.0), 12.0, true, WHITE)?;
            if t > 0.12 {
                v.round((713.0, 216.0, 857.0, 252.0), 14.0, acc(0.24 * t))?;
                v.text("Listening…", (728.0, 216.0, 849.0, 252.0), 12.0, true, WHITE)?;
            }
            v.pet(473.0 + 205.0 * t, 219.0 + 31.0 * t, 31.0 + 37.0 * t)?;
        }
    }
    Ok(())
}

pub fn paint_guide(p: &Painter, size: (f32, f32), dpi: f32, guide: &GuideUi, ui: &UiState) -> Result<()> {
    let l = Layout::new(size, dpi);
    let v = Preview { p, layout: l };
    let step = &STEPS[guide.page];
    let muted = rgba(1.0, 1.0, 1.0, 0.7);
    v.text("NIGHT ISLAND", (48.0, 65.0, 226.0, 95.0), 13.0, true, acc(1.0))?;
    v.text(&format!("{:02} / {:02}", guide.page + 1, STEPS.len()), (828.0, 65.0, 892.0, 95.0), 12.5, true, muted)?;
    for i in 0..STEPS.len() {
        let x = 48.0 + 141.0 * i as f32;
        v.round(
            (x, 104.0, x + 134.0, 108.0),
            2.0,
            if i <= guide.page { acc(0.95) } else { rgba(1.0, 1.0, 1.0, 0.11) },
        )?;
    }
    v.text(step.section, (48.0, 149.0, 396.0, 176.0), 14.0, true, acc(1.0))?;
    p.text_wrapped(
        step.title,
        l.rect((48.0, 178.0, 406.0, 248.0)),
        32.0 * l.scale,
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        WHITE,
    )?;
    p.text_wrapped(
        step.summary,
        l.rect((48.0, 254.0, 406.0, 315.0)),
        15.0 * l.scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        muted,
    )?;
    for (i, &(glyph, title, description)) in step.features.iter().enumerate() {
        let y = 333.0 + i as f32 * 69.0;
        v.round((48.0, y + 4.0, 86.0, y + 42.0), 11.0, acc(0.18))?;
        v.icon(glyph, (48.0, y + 4.0, 86.0, y + 42.0), 16.0)?;
        v.text(title, (100.0, y, 407.0, y + 24.0), 14.0, true, WHITE)?;
        p.text_wrapped(
            description,
            l.rect((100.0, y + 25.0, 407.0, y + 64.0)),
            12.5 * l.scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            muted,
        )?;
    }
    let stage = l.rect((426.0, 149.0, 892.0, 556.0));
    p.fill_round_gradient(stage, 23.0 * l.scale, acc(0.27), rgba(0.075, 0.09, 0.15, 0.77))?;
    p.stroke_round_rect(stage, 23.0 * l.scale, l.scale, acc(0.45))?;
    v.text("Preview", (453.0, 167.0, 680.0, 195.0), 12.0, true, muted)?;
    paint_preview(&v, guide)?;
    if let Some((field, inner)) = guide_wake_rects(size, dpi, guide) {
        v.text("Assistant wake words", (453.0, 342.0, 865.0, 370.0), 14.0, true, WHITE)?;
        paint_line_edit(
            p,
            field,
            inner,
            &guide.wake,
            false,
            "Luna, Taburetka",
            ui.sw.field == Some(Field::GuideWake),
            ui.sw.caret_on,
            l.scale,
        )?;
        let (status, error) = guide.wake_feedback.unwrap_or(("Separate the names with a comma.", false));
        p.text_wrapped(
            status,
            l.rect((453.0, 425.0, 865.0, 477.0)),
            12.5 * l.scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            if error { rgba(1.0, 0.55, 0.51, 1.0) } else { muted },
        )?;
    }
    for (action, label, rect) in controls(guide) {
        guide_button(
            p,
            l.rect(rect),
            l.scale,
            label,
            matches!(action, GuideAction::Next | GuideAction::SaveWake),
            ui.sw.hover == Some(SwHit::Guide(action)),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tour_navigation_and_wake_field_fit_small_windows_and_high_dpi() {
        let mut guide = GuideUi::new("");
        guide.move_by(-1);
        assert_eq!(guide.page, 0);
        for step in 0..STEPS.len() {
            for (size, dpi) in [((760.0, 550.0), 1.0), ((940.0, 680.0), 1.0), ((1880.0, 1360.0), 2.0)] {
                let layout = Layout::new(size, dpi);
                let buttons = controls(&guide);
                for (action, _, r) in &buttons {
                    let r = layout.rect(*r);
                    assert!(r.0 >= 0.0 && r.1 >= 40.0 * dpi && r.2 <= size.0 && r.3 <= size.1);
                    assert_eq!(guide_hit(size, dpi, &guide, (r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0), Some(*action));
                }
                assert!(buttons.iter().any(|(action, _, _)| *action == GuideAction::Skip));
                assert_eq!(guide_wake_rects(size, dpi, &guide).is_some(), guide.last_page());
            }
            assert_eq!(guide.page, step);
            guide.move_by(1);
        }
        assert!(guide.last_page());
        guide.play_demo(false);
        assert_eq!(guide.progress(), 1.0);
        assert!(!guide.animating());
    }

    #[test]
    fn wake_words_keep_both_distinct_names_and_reject_duplicates() {
        let mut guide = GuideUi::new(" Луна, Табуретка ");
        assert_eq!(guide.validated_wake().as_deref(), Some("Луна, Табуретка"));
        guide.wake.set_text("Луна, луна");
        assert!(guide.validated_wake().is_none());
        guide.wake.set_text(" ");
        assert!(guide.validated_wake().is_none());
        guide.wake.set_text(&"а".repeat(25));
        assert!(guide.validated_wake().is_none());
    }
}
