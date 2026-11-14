use super::{Hit, Painter, Rect, State, SwitcherStyle, contains, rgba};
use std::ops::Range;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::DirectWrite::{DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_CENTER};
use windows::core::Result;

#[derive(Clone, Copy)]
pub(super) struct Strip {
    pub size: (i32, i32),
    pub capacity: usize,
    pub carousel: bool,
    pub vertical: bool,
    scale: f32,
    pitch: f32,
}

impl Strip {
    pub fn new(work: RECT, scale: f32, count: usize, style: SwitcherStyle) -> Self {
        let vertical = matches!(style, SwitcherStyle::SideLeft | SwitcherStyle::SideRight);
        let carousel = style == SwitcherStyle::Carousel || vertical;
        let room = (work.right - work.left) as f32 * 0.92 / scale;
        let avail_h = (work.bottom - work.top) as f32 / scale;
        let (width, height, capacity, pitch) = if vertical {
            let width = (room * 0.48).clamp(232.0, 350.0).min(room);
            let max_h = (avail_h * 0.90).min(610.0);
            let pitch = 42.0;
            let header = (width * 0.48 + 118.0).min(max_h - 3.0 * pitch).max(104.0);
            let available = ((max_h - header) / pitch).floor().max(1.0) as usize;
            let desired = (count.max(1) / 2 * 2 + 1).min(7);
            let capacity = (desired.min(available) - 1) / 2 * 2 + 1;
            (width, (header + capacity as f32 * pitch).min(max_h), capacity, pitch)
        } else {
            let pitch = if carousel { 56.0 } else { 68.0 };
            let width = if carousel { 648.0 } else { (count.min(10) as f32 * pitch + 68.0).max(236.0) }.min(room);
            let available = (((width - 68.0) / pitch).floor() as usize).max(1);
            let capacity = if carousel { (available.min(9) - 1) / 2 * 2 + 1 } else { available.min(count.max(1)) };
            let height = if carousel { (avail_h * 0.61).min(470.0) } else { 144.0_f32.min(avail_h * 0.92) };
            (width, height, capacity, pitch)
        };
        Self {
            size: ((width * scale).round() as i32, (height * scale).round() as i32),
            capacity,
            carousel,
            vertical,
            scale,
            pitch: pitch * scale,
        }
    }

    pub fn target(self, selected: usize, len: usize) -> f32 {
        if self.carousel {
            selected as f32 - (self.capacity / 2) as f32
        } else {
            selected.saturating_sub(self.capacity / 2).min(len.saturating_sub(self.capacity)) as f32
        }
    }

    pub fn range(self, offset: f32, len: usize) -> Range<usize> {
        let start = offset.floor() as usize;
        start.min(len)..((offset.ceil() + self.capacity as f32) as usize).min(len)
    }

    fn viewport(self) -> Rect {
        let s = self.scale;
        if self.vertical {
            let bottom = self.size.1 as f32 - 32.0 * s;
            return (18.0 * s, bottom - self.pitch * self.capacity as f32, self.size.0 as f32 - 18.0 * s, bottom);
        }
        let left = (self.size.0 as f32 - self.capacity as f32 * self.pitch) * 0.5;
        let top = if self.carousel { self.size.1 as f32 - 76.0 * s } else { 17.0 * s };
        (left, top, left + self.capacity as f32 * self.pitch, top + self.pitch)
    }

    pub(super) fn tile(self, index: f32, offset: f32) -> Rect {
        let (l, t, r, b) = self.viewport();
        if self.vertical {
            let y = t + (index - offset) * self.pitch;
            return (
                l + 2.0 * self.scale,
                y + 2.0 * self.scale,
                r - 2.0 * self.scale,
                y + self.pitch - 2.0 * self.scale,
            );
        }
        let x = l + (index - offset) * self.pitch;
        let inset = 3.0 * self.scale;
        (x + inset, t + inset, x + self.pitch - inset, b - inset)
    }

    pub fn preview(self) -> Rect {
        let s = self.scale;
        let bottom = if self.vertical { self.viewport().1 - 60.0 * s } else { self.size.1 as f32 - 122.0 * s };
        (20.0 * s, 20.0 * s, self.size.0 as f32 - 20.0 * s, bottom)
    }

    fn caption(self) -> Rect {
        let s = self.scale;
        let y = if self.carousel { self.preview().3 + 3.0 * s } else { self.viewport().3 + 5.0 * s };
        (23.0 * s, y, self.size.0 as f32 - 23.0 * s, y + 36.0 * s)
    }

    fn zoom(self) -> Rect {
        let (_, t, r, b) = self.caption();
        (r - 76.0 * self.scale, t + 4.0 * self.scale, r - 34.0 * self.scale, b - 4.0 * self.scale)
    }

    fn close(self) -> Rect {
        let (_, t, r, b) = self.caption();
        (r - 29.0 * self.scale, t + 3.0 * self.scale, r, b - 3.0 * self.scale)
    }

    fn arrow(self, next: bool) -> Rect {
        let (l, t, r, b) = self.viewport();
        if self.vertical {
            let y = if next { b } else { t - 21.0 * self.scale };
            return (l, y, r, y + 21.0 * self.scale);
        }
        let x = if next { r } else { l - 26.0 * self.scale };
        (x, t, x + 26.0 * self.scale, b)
    }

    pub fn hit(self, point: (f32, f32), offset: f32, len: usize, selected: usize) -> Option<Hit> {
        if selected < len && contains(self.close(), point) {
            return Some(Hit::Close(selected));
        }
        if self.carousel && contains(self.zoom(), point) {
            return Some(Hit::ActualSize);
        }
        if self.carousel && selected < len && contains(self.preview(), point) {
            return Some(Hit::Window(selected));
        }
        if len > self.capacity {
            for next in [false, true] {
                if contains(self.arrow(next), point) {
                    return Some(if next { Hit::Next } else { Hit::Prev });
                }
            }
        }
        if contains(self.viewport(), point) {
            return self.range(offset, len).find(|&i| contains(self.tile(i as f32, offset), point)).map(Hit::Window);
        }
        None
    }
}

impl State {
    pub(super) fn paint_strip(&self, p: &Painter, strip: Strip) -> Result<()> {
        let s = strip.scale;
        let len = self.items.len();
        let Some(item) = self.items.get(self.selected) else { return Ok(()) };
        let offset = self
            .focus_motion
            .follow(2, self.strip_target, self.strip_target, 155)
            .clamp(strip.target(0, len), strip.target(len - 1, len));
        self.strip_offset.set(offset);
        let focus = self.focus_motion.follow(1, self.selected as f32, self.selected as f32, 155);
        let (red, green, blue) = crate::ui::theme::accent();
        let accent = rgba(0.36 + 0.64 * red, 0.36 + 0.64 * green, 0.36 + 0.64 * blue, 1.0);
        if strip.carousel {
            let area = strip.preview();
            p.fill_round_rect(area, 13.0 * s, rgba(0.06, 0.06, 0.07, 1.0))?;
            if self.thumbnails.first().is_none_or(Option::is_none) {
                p.text(
                    if item.window.minimized { "Window minimized" } else { "Preview unavailable" },
                    area,
                    14.0 * s,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_TEXT_ALIGNMENT_CENTER,
                    rgba(1.0, 1.0, 1.0, 0.72),
                )?;
            }
            let zoom = strip.zoom();
            p.fill_round_rect(zoom, 8.0 * s, rgba(1.0, 1.0, 1.0, if self.actual_size { 0.24 } else { 0.09 }))?;
            p.text(
                "1:1",
                zoom,
                12.0 * s,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                rgba(1.0, 1.0, 1.0, 0.94),
            )?;
        }
        let close = strip.close();
        if self.hover == Some(Hit::Close(self.selected)) {
            p.fill_round_rect(close, 8.0 * s, rgba(0.90, 0.28, 0.28, 0.35))?;
        }
        p.glyph('\u{E711}', close, 10.0 * s, rgba(1.0, 1.0, 1.0, 0.72))?;
        let (l, t, _, b) = strip.caption();
        let count_right = if strip.carousel { strip.zoom().0 - 6.0 * s } else { close.0 - 5.0 * s };
        let count = (count_right - 54.0 * s, t, count_right, b);
        let title = (l, t, count.0 - 6.0 * s, b);
        p.push_clip(title);
        let result = p.text_raw(&item.window.title, title, 15.0 * s);
        p.pop_clip();
        result?;
        p.text(
            &format!("{} / {}", self.selected + 1, len),
            count,
            12.0 * s,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER,
            rgba(1.0, 1.0, 1.0, 0.70),
        )?;

        p.push_clip(strip.viewport());
        let ring = strip.tile(focus, offset);
        let draw = || -> Result<()> {
            p.fill_round_rect(ring, 14.0 * s, rgba(red, green, blue, 0.21))?;
            p.stroke_round_rect(ring, 14.0 * s, 1.4 * s, accent)?;
            for i in strip.range(offset, len) {
                let (left, top, right, bottom) = strip.tile(i as f32, offset);
                let distance = (i as f32 - focus).abs().min(1.0);
                let side = (strip.pitch * (0.71 - 0.12 * distance)).round();
                let x = if strip.vertical { left + 10.0 * s } else { (left + right - side) * 0.5 }.round();
                let y = ((top + bottom - side) * 0.5).round();
                let image = (x, y, x + side, y + side);
                if strip.vertical {
                    let title = ((left + strip.pitch + 8.0 * s).round(), top, right - 8.0 * s, bottom);
                    p.text_raw(&self.items[i].window.title, title, 13.0 * s)?;
                }
                if let Some(icon) = &self.items[i].icon {
                    p.image(image, 3.0 * s, icon.side, &icon.bgra)?;
                } else {
                    p.glyph('\u{E737}', image, side * 0.68, rgba(1.0, 1.0, 1.0, 0.78))?;
                }
            }
            Ok(())
        }();
        p.pop_clip();
        draw?;
        if len > strip.capacity {
            let arrows = if strip.vertical { ['\u{E70E}', '\u{E70D}'] } else { ['\u{E76B}', '\u{E76C}'] };
            for (next, glyph) in [false, true].into_iter().zip(arrows) {
                p.glyph(glyph, strip.arrow(next), 11.0 * s, rgba(1.0, 1.0, 1.0, 0.76))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_fit_the_monitor_and_every_selected_icon_can_be_clicked() {
        for (w, h, dpi) in
            [(1920, 1080, 1.0), (1920, 1080, 1.25), (1280, 720, 1.5), (1080, 1920, 2.0), (3840, 2160, 2.0)]
        {
            let work = RECT { left: -w, top: -h, right: 0, bottom: 0 };
            for style in
                [SwitcherStyle::Carousel, SwitcherStyle::Icons, SwitcherStyle::SideLeft, SwitcherStyle::SideRight]
            {
                for len in [1, 2, 9, 37] {
                    let strip = Strip::new(work, dpi, len, style);
                    assert!(strip.size.0 <= w && strip.size.1 <= h);
                    for selected in 0..len {
                        let offset = strip.target(selected, len);
                        assert!(strip.range(offset, len).contains(&selected));
                        let (l, t, r, b) = strip.tile(selected as f32, offset);
                        if strip.vertical {
                            assert!((t + b - strip.viewport().1 - strip.viewport().3).abs() < 1.0);
                        } else if strip.carousel {
                            assert!((l + r - strip.size.0 as f32).abs() < 1.0);
                        }
                        assert!(l >= 0.0 && t >= 0.0 && r <= strip.size.0 as f32 && b <= strip.size.1 as f32);
                        assert!(
                            strip.hit(((l + r) * 0.5, (t + b) * 0.5), offset, len, selected)
                                == Some(Hit::Window(selected))
                        );
                    }
                }
            }
        }
    }
}
