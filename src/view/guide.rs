use std::time::{Duration, Instant};

use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

use crate::view::*;

const DEMO_TIME: Duration = Duration::from_millis(430);
const VOICE_DEMO_TIME: Duration = Duration::from_millis(3600);
const SCENE: Rect = (426.0, 163.0, 892.0, 556.0);
const PET_DOCK: Rect = (462.0, 215.0, 657.0, 254.0);
const PET_DOCKED: (f32, f32) = (488.5, 234.5);
const PET_RELEASED: (f32, f32) = (712.0, 284.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuideAction {
    Next,
    Back,
    Chapter(usize),
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
    pet_drag: Option<PetDrag>,
    pet_drop: Option<(Instant, (f32, f32))>,
    pub wake: TextEdit,
    pub wake_feedback: Option<(&'static str, bool)>,
}

struct PetDrag {
    pointer_offset: (f32, f32),
    center: (f32, f32),
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

    pub fn has_demo(&self) -> bool {
        self.page != 1
    }

    pub fn last_page(&self) -> bool {
        self.page + 1 == STEPS.len()
    }

    pub fn move_by(&mut self, offset: isize) {
        self.open_page(self.page.saturating_add_signed(offset).min(STEPS.len() - 1));
    }

    pub fn open_page(&mut self, page: usize) {
        if page != self.page && page < STEPS.len() {
            self.page = page;
            self.expanded = false;
            self.transition = None;
            self.pet_drag = None;
            self.pet_drop = None;
        }
    }

    pub fn play_demo(&mut self, animate: bool) {
        self.pet_drop = None;
        let replay = matches!(self.page, 3 | 4);
        let from = if replay { 0.0 } else { self.progress() };
        self.expanded = replay || !self.expanded;
        self.transition = animate.then(|| (Instant::now(), from));
    }

    fn progress(&self) -> f32 {
        let target = f32::from(self.expanded);
        self.transition.map_or(target, |(started, from)| from + (target - from) * demo_ease(started))
    }

    fn pet_center(&self) -> (f32, f32) {
        if let Some(drag) = &self.pet_drag {
            return drag.center;
        }
        if let Some((started, from)) = self.pet_drop {
            return mix_point(from, mix_point(PET_DOCKED, PET_RELEASED, f32::from(self.expanded)), demo_ease(started));
        }
        mix_point(PET_DOCKED, PET_RELEASED, self.progress())
    }

    pub fn begin_pet_drag(&mut self, size: (f32, f32), dpi: f32, point: (f32, f32)) -> bool {
        if !self.visible || !self.last_page() {
            return false;
        }
        let point = Layout::new(size, dpi).local(point);
        let (x, y) = self.pet_center();
        let radius = 20.0 + 15.0 * self.progress();
        if !inside((x - radius, y - radius, x + radius, y + radius), point.0, point.1) {
            return false;
        }
        self.pet_drop = None;
        self.pet_drag = Some(PetDrag { pointer_offset: (x - point.0, y - point.1), center: (x, y) });
        true
    }

    pub fn move_pet(&mut self, size: (f32, f32), dpi: f32, point: (f32, f32)) -> bool {
        let Some(drag) = self.pet_drag.as_mut() else { return false };
        let (x, y) = Layout::new(size, dpi).local(point);
        drag.center =
            ((x + drag.pointer_offset.0).clamp(481.0, 827.0), (y + drag.pointer_offset.1).clamp(230.0, 310.0));
        true
    }

    pub fn drop_pet(&mut self, animate: bool) -> bool {
        let Some(drag) = self.pet_drag.take() else { return false };
        let (x, y) = drag.center;
        let from = self.progress();
        self.expanded = !inside(PET_DOCK, x, y);
        let now = Instant::now();
        self.pet_drop = animate.then_some((now, (x, y)));
        self.transition = animate.then_some((now, from));
        true
    }

    fn voice_phase(&self) -> usize {
        if !self.expanded {
            return 0;
        }
        let elapsed = self.transition.map_or(VOICE_DEMO_TIME, |(started, _)| started.elapsed());
        match elapsed.as_millis() {
            0..=999 => 1,
            1000..=1899 => 2,
            1900..=3599 => 3,
            _ => 4,
        }
    }

    pub fn animating(&self) -> bool {
        let duration = if self.page == 4 { VOICE_DEMO_TIME } else { DEMO_TIME };
        self.visible && self.transition.is_some_and(|(started, _)| started.elapsed() < duration)
    }
}

fn demo_ease(started: Instant) -> f32 {
    let t = (started.elapsed().as_secs_f32() / DEMO_TIME.as_secs_f32()).min(1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix_point(from: (f32, f32), to: (f32, f32), t: f32) -> (f32, f32) {
    (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
}

struct Step {
    chapter: &'static str,
    scene: &'static str,
    title: &'static str,
    summary: &'static str,
    features: [(char, &'static str, &'static str); 3],
}

const STEPS: [Step; 6] = [
    Step {
        chapter: "The island",
        scene: "Your desktop",
        title: "An island on your desktop",
        summary: "Music, messages and a little companion. All from a small island at the edge of your screen.",
        features: [
            ('\u{E8A7}', "Open on hover", "Move the pointer over the island to open it. Move away to close it."),
            ('\u{E718}', "Keep it open", "Use the pin when you want the panel to stay open."),
            ('\u{E713}', "Make it yours", "Adjust the position, shape and animation in Settings."),
        ],
    },
    Step {
        chapter: "Tools",
        scene: "Home preview",
        title: "The tools you use daily",
        summary: "Start on Home. Open More to find the rest of your tools and change the tab bar.",
        features: [
            ('\u{E189}', "Music and sound", "Control playback, seek through a track and adjust app volume."),
            ('\u{E77F}', "Clipboard and notes", "Find something you copied or jot down a reminder."),
            ('\u{E713}', "Your shortcuts", "Edit the tiles on Home to keep the actions you use within reach."),
        ],
    },
    Step {
        chapter: "Desktop",
        scene: "Desktop preview",
        title: "Organize your desktop",
        summary: "Choose what sits around the island, from favorite apps to the system tray.",
        features: [
            ('\u{E7F4}', "Bottom dock", "Pin and reorder apps. Hover to preview their windows."),
            ('\u{E8A9}', "Top panel", "Choose the tray, active app and system controls you want to see."),
            ('\u{E9D9}', "Screen edges", "Set panels to appear on hover and hide them in fullscreen apps."),
        ],
    },
    Step {
        chapter: "Notifications",
        scene: "Notification preview",
        title: "Notifications at a glance",
        summary: "Windows notifications can appear as animated cards beside the island.",
        features: [
            ('\u{EA8F}', "Connect Windows notifications", "Open the Notifications tab to grant access."),
            ('\u{E8A7}', "Read at your pace", "Hover to pause the card, then open the full message."),
            ('\u{E7ED}', "Choose where alerts appear", "Use Windows Do not disturb to quiet system banners."),
        ],
    },
    Step {
        chapter: "Assistant",
        scene: "Voice preview",
        title: "Talk to the Assistant",
        summary: "Ask a question, control music or set a reminder. Start with a model and speech recognition.",
        features: [
            ('\u{E99A}', "Connect a model", "Add a model provider in Settings, under Assistant."),
            ('\u{E720}', "Set up speech", "Choose how speech is recognized and how answers are spoken."),
            ('\u{EA18}', "Choose permissions", "Decide which actions run directly and which ask first."),
        ],
    },
    Step {
        chapter: "Pet",
        scene: "Meet Spark",
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

    fn local(self, (x, y): (f32, f32)) -> (f32, f32) {
        ((x - self.x) / self.scale, (y - self.y) / self.scale)
    }

    fn rect(self, (l, t, r, b): Rect) -> Rect {
        (self.x + l * self.scale, self.y + t * self.scale, self.x + r * self.scale, self.y + b * self.scale)
    }
}

fn controls(guide: &GuideUi) -> Vec<(GuideAction, &'static str, Rect)> {
    use GuideAction::*;
    let mut buttons = vec![
        (Skip, "Skip tour", (48.0, 601.0, 176.0, 647.0)),
        (
            Next,
            match guide.page {
                0 => "Start the tour",
                _ if guide.last_page() => "Finish tour",
                _ => "Next",
            },
            (714.0, 601.0, 892.0, 647.0),
        ),
    ];
    if guide.page > 0 {
        buttons.push((Back, "Back", (592.0, 601.0, 702.0, 647.0)));
    }
    for (i, step) in STEPS.iter().enumerate() {
        let x = 48.0 + 142.0 * i as f32;
        buttons.push((Chapter(i), step.chapter, (x, 100.0, x + 134.0, 136.0)));
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
        4 => buttons.extend(
            split((Demo, "Try a voice command"), (OpenAssistant, "Set up the Assistant"))
                .map(|((a, label), r)| (a, label, r)),
        ),
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
    controls(guide)
        .into_iter()
        .find(|(_, _, rect)| inside(layout.rect(*rect), x, y))
        .map(|(action, _, _)| action)
        .or_else(|| {
            let demo = match guide.page {
                0 => (460.0, 223.0, 858.0, 470.0),
                2 => (504.0, 378.0, 815.0, 439.0),
                _ => return None,
            };
            inside(layout.rect(demo), x, y).then_some(GuideAction::Demo)
        })
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
        self.pose_pet(x, y, size, crate::ui::characters::Reaction::Idle, 0.0)
    }

    fn pose_pet(&self, x: f32, y: f32, size: f32, reaction: crate::ui::characters::Reaction, mouth: f32) -> Result<()> {
        use crate::ui::characters::{Character, Pose, draw_with_design};
        let r = self.layout.rect((x, y, x + size, y + size));
        let saved = self.p.push_offset(r.0, r.1);
        let result = draw_with_design(
            self.p,
            Character::Spark,
            size * self.layout.scale,
            &Pose {
                motion: crate::ui::pet_motion::Motion { mouth, ..Default::default() },
                t: 0.0,
                reaction,
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

    fn center(&self, label: &str, rect: Rect, size: f32, color: D2D1_COLOR_F) -> Result<()> {
        self.p.text(
            label,
            self.layout.rect(rect),
            size * self.layout.scale,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            color,
        )
    }

    fn glow(&self, center: (f32, f32), radii: (f32, f32), color: D2D1_COLOR_F) -> Result<()> {
        let (x, y, _, _) = self.layout.rect((center.0, center.1, center.0, center.1));
        let s = self.layout.scale;
        self.p.fill_ellipse_gradient((x, y, radii.0 * s, radii.1 * s), color, rgba(color.r, color.g, color.b, 0.0))
    }

    fn curve(&self, points: [(f32, f32); 4], width: f32, color: D2D1_COLOR_F) -> Result<()> {
        self.p.bezier(
            points.map(|(x, y)| {
                let (x, y, _, _) = self.layout.rect((x, y, x, y));
                (x, y)
            }),
            width * self.layout.scale,
            color,
        )
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
        self.icon('\u{E189}', (x + 29.0, y + 64.0, x + 80.0, y + 115.0), 24.0)?;
        self.text("Now playing", (x + 96.0, y + 62.0, x + 356.0, y + 82.0), 10.5, false, rgba(1.0, 1.0, 1.0, 0.67))?;
        self.text("Midnight City", (x + 96.0, y + 81.0, x + 274.0, y + 108.0), 16.0, true, WHITE)?;
        self.text("M83", (x + 96.0, y + 108.0, x + 274.0, y + 123.0), 11.0, false, rgba(1.0, 1.0, 1.0, 0.64))?;
        self.round((x + 306.0, y + 77.0, x + 344.0, y + 115.0), 19.0, rgba(1.0, 1.0, 1.0, 0.16))?;
        self.icon('\u{E769}', (x + 306.0, y + 77.0, x + 344.0, y + 115.0), 14.0)?;
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
            let width = 252.0 + 138.0 * t;
            let x = 659.0 - width / 2.0;
            let y = 283.0 - 57.0 * t;
            let height = 48.0 + 176.0 * t;
            v.glow((659.0, y + height * 0.72), (187.0, 85.0), rgba(0.01, 0.02, 0.07, 0.87))?;
            if t < 1.0 {
                let center = 659.0;
                let base = y + height - 2.0;
                let depth = (1.0 - t) * (5.0 + 22.0 * t);
                let point = |x, y| {
                    let (x, y, _, _) = l.rect((x, y, x, y));
                    (x, y)
                };
                p.fill_curves(
                    point(center - 72.0, base),
                    &[
                        [point(center - 34.0, base), point(center - 26.0, base + depth), point(center, base + depth)],
                        [point(center + 26.0, base + depth), point(center + 34.0, base), point(center + 72.0, base)],
                    ],
                    rgba(0.015, 0.018, 0.023, 1.0),
                )?;
            }
            v.round((x, y, x + width, y + height), 24.0, rgba(0.015, 0.018, 0.023, 1.0))?;
            if t >= 0.45 {
                p.push_clip(l.rect((x + 15.0, y + 8.0, x + width - 15.0, y + height - 12.0)));
                let result = v.home(464.0, y);
                p.pop_clip();
                result?;
            } else {
                v.pet(x + 10.0, y + 5.0, 40.0)?;
                v.text("21:08", (x + 62.0, y, x + 136.0, y + 48.0), 17.0, true, WHITE)?;
                v.icon('\u{F800}', (x + 133.0, y, x + 161.0, y + 48.0), 16.0)?;
                v.text("18°", (x + 165.0, y, x + 199.0, y + 48.0), 13.0, true, WHITE)?;
                v.icon('\u{E76C}', (x + 209.0, y, x + 239.0, y + 48.0), 11.0)?;
            }
            if guide.page == 0 && t < 0.1 {
                v.center("Tap the island to open it", (469.0, 363.0, 849.0, 393.0), 14.0, WHITE)?;
                v.center(
                    "On your desktop it opens on hover",
                    (469.0, 394.0, 849.0, 416.0),
                    12.0,
                    rgba(1.0, 1.0, 1.0, 0.71),
                )?;
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
            if t > 0.0 {
                let top = 378.0 - 106.0 * t;
                v.island(578.0, top, 163.0, 94.0 * t)?;
                if t > 0.72 {
                    v.round((588.0, top + 10.0, 731.0, top + 60.0), 8.0, acc(0.23))?;
                    v.icon('\u{E189}', (588.0, top + 10.0, 731.0, top + 60.0), 19.0)?;
                    v.center("Midnight City", (588.0, top + 62.0, 731.0, top + 90.0), 11.5, WHITE)?;
                }
            }
            v.round((504.0, 378.0, 815.0, 439.0), 18.0, rgba(0.06, 0.075, 0.12, 0.96))?;
            for (i, glyph) in
                ['\u{E8B7}', '\u{E774}', '\u{E189}', '\u{E8F2}', '\u{E70B}', '\u{E713}'].into_iter().enumerate()
            {
                let x = 516.0 + i as f32 * 48.0;
                let tones = [
                    rgba(0.52, 0.38, 0.20, 0.92),
                    rgba(0.26, 0.38, 0.66, 0.92),
                    acc(0.68 + 0.32 * t),
                    rgba(0.30, 0.25, 0.60, 0.92),
                    rgba(0.20, 0.45, 0.42, 0.92),
                    rgba(0.30, 0.33, 0.40, 0.92),
                ];
                v.round((x, 389.0, x + 37.0, 425.0), 9.0, tones[i])?;
                v.icon(glyph, (x, 389.0, x + 37.0, 425.0), 17.0)?;
            }
            v.center(
                "Tap the dock to preview a window",
                (453.0, 447.0, 865.0, 475.0),
                12.0,
                rgba(1.0, 1.0, 1.0, 0.73),
            )?;
        }
        3 => {
            let width = 191.0 + 169.0 * t;
            let height = 38.0 + 124.0 * t;
            let x = 659.0 - width / 2.0;
            v.glow((659.0, 306.0), (194.0, 99.0), acc(0.17 + 0.16 * t))?;
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
                v.icon('\u{EA8F}', (x + 12.0, 247.0, x + 45.0, 285.0), 14.0)?;
                v.text("Night Island", (x + 49.0, 247.0, x + 174.0, 285.0), 12.0, true, WHITE)?;
                v.center("Send yourself a sample card", (453.0, 365.0, 865.0, 397.0), 14.0, WHITE)?;
            }
        }
        4 => {
            use crate::ui::characters::Reaction;
            let phase = guide.voice_phase();
            let elapsed = guide.transition.map_or(0.0, |(started, _)| started.elapsed().as_secs_f32());
            let mouth = if phase == 3 { 0.18 + 0.69 * (elapsed * 16.0).sin().abs() } else { 0.0 };
            v.island(474.0, 220.0, 370.0, 235.0)?;
            v.pose_pet(
                490.0,
                230.0,
                46.0,
                match phase {
                    0 => Reaction::Idle,
                    3 | 4 => Reaction::Done,
                    _ => Reaction::Attention,
                },
                mouth,
            )?;
            v.text("Assistant", (548.0, 232.0, 824.0, 259.0), 16.0, true, WHITE)?;
            v.text(
                "Example only · microphone off",
                (548.0, 259.0, 826.0, 277.0),
                11.0,
                false,
                rgba(1.0, 1.0, 1.0, 0.64),
            )?;
            for (i, (icon, label)) in [('\u{E720}', "Listening…"), ('\u{E99A}', "Thinking…"), ('\u{E8F2}', "Speaking")]
                .into_iter()
                .enumerate()
            {
                let x = 495.0 + i as f32 * 109.0;
                let active = phase == i + 1;
                v.round((x, 288.0, x + 101.0, 350.0), 12.0, acc(if active { 0.42 } else { 0.11 }))?;
                v.icon(icon, (x + 7.0, 290.0, x + 94.0, 322.0), 15.0)?;
                p.text(
                    label,
                    l.rect((x + 3.0, 322.0, x + 98.0, 347.0)),
                    11.0 * l.scale,
                    DWRITE_FONT_WEIGHT_SEMI_BOLD,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    rgba(1.0, 1.0, 1.0, if active { 1.0 } else { 0.63 }),
                )?;
            }
            v.round((495.0, 366.0, 823.0, 434.0), 12.0, rgba(1.0, 1.0, 1.0, 0.075))?;
            v.text("Remind me in ten minutes.", (510.0, 371.0, 809.0, 399.0), 13.0, false, rgba(1.0, 1.0, 1.0, 0.71))?;
            if phase >= 3 {
                v.text("I’ll remind you in ten minutes.", (510.0, 399.0, 809.0, 426.0), 13.0, true, WHITE)?;
            } else if phase > 0 {
                for i in 0..12 {
                    let x = 512.0 + 6.0 * i as f32;
                    let h = 4.0 + 12.0 * (elapsed * 13.0 - i as f32 * 0.7).sin().abs();
                    v.round((x, 413.0 - h / 2.0, x + 3.0, 413.0 + h / 2.0), 1.5, acc(0.75))?;
                }
            }
        }
        _ => {
            v.island(462.0, 215.0, 195.0, 39.0)?;
            if guide.pet_drag.is_some() {
                p.stroke_round_rect(l.rect(PET_DOCK), 19.5 * l.scale, l.scale, acc(0.75))?;
            }
            v.text("21:08       18°", (520.0, 215.0, 644.0, 254.0), 12.0, true, WHITE)?;
            if t > 0.12 {
                v.round((713.0, 216.0, 857.0, 252.0), 14.0, acc(0.24 * t))?;
                v.text("Listening…", (728.0, 216.0, 849.0, 252.0), 12.0, true, WHITE)?;
            }
            let (x, y) = guide.pet_center();
            let size = 31.0 + 37.0 * t;
            v.pet(x - size / 2.0, y - size / 2.0, size)?;
            v.text("Drag Spark to try it", (454.0, 317.0, 865.0, 337.0), 11.5, false, rgba(1.0, 1.0, 1.0, 0.72))?;
        }
    }
    Ok(())
}

fn paint_stage(v: &Preview<'_>, guide: &GuideUi, hover: bool) -> Result<()> {
    let p = v.p;
    let l = v.layout;
    let stage = l.rect(SCENE);
    let hero = guide.page == 0;
    p.fill_round_gradient(stage, 26.0 * l.scale, rgba(0.065, 0.082, 0.15, 1.0), rgba(0.032, 0.037, 0.059, 1.0))?;
    v.glow((648.0, 321.0), (214.0, 142.0), acc(if hero { 0.29 } else { 0.15 }))?;
    v.glow((772.0, 399.0), (115.0, 99.0), rgba(0.16, 0.41, 0.69, 0.16))?;
    p.push_clip(l.rect((440.0, 217.0, 878.0, 479.0)));
    let backdrop = (|| -> Result<()> {
        for i in 0..7 {
            let y = i as f32 * 13.0;
            v.curve(
                [(444.0, 389.0 + y), (551.0, 232.0 + y), (706.0, 513.0 + y), (875.0, 295.0 + y)],
                1.0,
                acc(if hero { 0.23 } else { 0.067 }),
            )?;
        }
        for (x, y, radius) in [
            (473.0, 298.0, 1.0),
            (504.0, 244.0, 1.5),
            (591.0, 263.0, 1.0),
            (724.0, 238.0, 1.5),
            (810.0, 258.0, 1.0),
            (842.0, 319.0, 1.5),
            (479.0, 420.0, 1.5),
            (831.0, 439.0, 1.0),
        ] {
            v.round((x - radius, y - radius, x + radius, y + radius), radius, acc(if hero { 0.57 } else { 0.35 }))?;
        }
        Ok(())
    })();
    p.pop_clip();
    backdrop?;
    p.stroke_round_rect(stage, 26.0 * l.scale, l.scale, rgba(0.7, 0.74, 1.0, 0.25))?;
    v.text(STEPS[guide.page].scene, (453.0, 176.0, 701.0, 209.0), 13.0, true, WHITE)?;
    v.round((744.0, 180.0, 865.0, 205.0), 12.5, rgba(1.0, 1.0, 1.0, if hover { 0.13 } else { 0.065 }))?;
    v.round((757.0, 190.0, 763.0, 196.0), 3.0, acc(0.98))?;
    v.text("Demo mode", (772.0, 180.0, 857.0, 205.0), 10.5, true, rgba(1.0, 1.0, 1.0, 0.75))?;
    paint_preview(v, guide)
}

pub fn paint_guide(p: &Painter, size: (f32, f32), dpi: f32, guide: &GuideUi, ui: &UiState) -> Result<()> {
    let l = Layout::new(size, dpi);
    let v = Preview { p, layout: l };
    let step = &STEPS[guide.page];
    let muted = rgba(1.0, 1.0, 1.0, 0.7);
    v.round((48.0, 59.0, 77.0, 88.0), 9.0, acc(0.24))?;
    v.icon('\u{E7F4}', (48.0, 59.0, 77.0, 88.0), 14.0)?;
    v.text("Night Island", (87.0, 59.0, 310.0, 88.0), 16.0, true, WHITE)?;
    v.text(&format!("{:02} / {:02}", guide.page + 1, STEPS.len()), (837.0, 59.0, 892.0, 88.0), 12.5, true, muted)?;
    v.text(
        if guide.page == 0 { "Welcome to Night Island" } else { step.chapter },
        (48.0, 163.0, 396.0, 189.0),
        13.0,
        true,
        acc(1.0),
    )?;
    p.text_wrapped(
        step.title,
        l.rect((48.0, 193.0, 405.0, 279.0)),
        if guide.page == 0 { 36.0 * l.scale } else { 32.0 * l.scale },
        DWRITE_FONT_WEIGHT_SEMI_BOLD,
        WHITE,
    )?;
    p.text_wrapped(
        step.summary,
        l.rect((48.0, 281.0, 395.0, 342.0)),
        15.0 * l.scale,
        DWRITE_FONT_WEIGHT_NORMAL,
        muted,
    )?;
    for (i, &(glyph, title, description)) in step.features.iter().enumerate() {
        let y = 350.0 + i as f32 * 67.0;
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
    paint_stage(&v, guide, ui.sw.hover == Some(SwHit::Guide(GuideAction::Demo)))?;
    v.round((48.0, 578.0, 892.0, 579.0), 0.5, rgba(1.0, 1.0, 1.0, 0.12))?;
    if guide.page == 0 {
        v.text("About 2 minutes", (197.0, 601.0, 448.0, 647.0), 12.0, false, muted)?;
    }
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
        let hover = ui.sw.hover == Some(SwHit::Guide(action));
        if let GuideAction::Chapter(page) = action {
            let selected = page == guide.page;
            let (x, top, right, bottom) = rect;
            let color = if selected { WHITE } else { muted };
            v.round(
                rect,
                9.0,
                if selected { acc(0.22) } else { rgba(1.0, 1.0, 1.0, if hover { 0.12 } else { 0.045 }) },
            )?;
            v.text(
                &format!("{:02}", page + 1),
                (x + 10.0, top, x + 30.0, bottom),
                10.5,
                true,
                if selected { acc(1.0) } else { muted },
            )?;
            v.text(label, (x + 31.0, top, right - 5.0, bottom), 12.0, selected, color)?;
            continue;
        }
        guide_button(
            p,
            l.rect(rect),
            l.scale,
            label,
            matches!(action, GuideAction::Next | GuideAction::SaveWake),
            hover,
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
    fn the_example_pet_can_be_dragged_out_and_back_at_high_dpi() {
        let mut guide = GuideUi::new("");
        guide.open_page(5);
        let size = (1880.0, 1360.0);
        let dpi = 2.0;
        let screen = |(x, y): (f32, f32)| (x * dpi, y * dpi);
        assert!(guide.begin_pet_drag(size, dpi, screen(PET_DOCKED)));
        assert!(guide.move_pet(size, dpi, screen(PET_RELEASED)));
        assert!(guide.drop_pet(false));
        assert!(guide.expanded);
        assert_eq!(guide.pet_center(), PET_RELEASED);
        assert!(guide.begin_pet_drag(size, dpi, screen(PET_RELEASED)));
        assert!(guide.move_pet(size, dpi, screen(PET_DOCKED)));
        assert!(guide.drop_pet(false));
        assert!(!guide.expanded);
        assert_eq!(guide.pet_center(), PET_DOCKED);
    }

    #[test]
    fn the_voice_example_can_finish_with_reduced_motion() {
        let mut guide = GuideUi::new("");
        guide.open_page(4);
        assert_eq!(guide.voice_phase(), 0);
        guide.play_demo(false);
        assert_eq!(guide.voice_phase(), 4);
        assert!(!guide.animating());
        guide.play_demo(true);
        assert_eq!(guide.voice_phase(), 1);
        assert!(guide.animating());
        guide.open_page(5);
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
