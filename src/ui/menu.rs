use std::cell::RefCell;
use std::sync::Once;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_METRICS,
};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
    CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST,
    MONITORINFO, MonitorFromPoint, ReleaseDC, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_DOWN, VK_ESCAPE, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_RETURN, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, GetWindowRect, HWND_TOPMOST,
    IDC_ARROW, LoadCursorW, MSG, MsgWaitForMultipleObjects, PM_REMOVE, PeekMessageW, PostQuitMessage, QS_ALLINPUT,
    RegisterClassW, SW_SHOW, SWP_NOSIZE, SetForegroundWindow, SetWindowPos, ShowWindow, TranslateMessage, ULW_ALPHA,
    UpdateLayeredWindow, WA_INACTIVE, WM_ACTIVATE, WM_KEYDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_QUIT, WM_RBUTTONUP,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{Result, w};

use crate::ui::gfx::{Gpu, Painter, rgba};
use crate::ui::theme;

pub struct Entry {
    pub id: usize,
    pub label: String,
    pub glyph: Option<char>,
    pub danger: bool,
}

impl Entry {
    pub fn item(id: usize, label: &str) -> Self {
        Self { id, label: label.to_string(), glyph: None, danger: false }
    }

    pub fn glyph(mut self, glyph: char) -> Self {
        self.glyph = Some(glyph);
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn divider() -> Self {
        Self { id: 0, label: String::new(), glyph: None, danger: false }
    }
}

const ITEM_H: f32 = 32.0;
const DIVIDER_H: f32 = 9.0;
const PAD: f32 = 6.0;
const FONT: f32 = 13.5;
const MIN_W: f32 = 190.0;

struct Menu {
    entries: Vec<Entry>,
    scale: f32,
    rows: Vec<(f32, f32, f32, f32)>,
    margin: f32,
    size: (i32, i32),
    hover: Option<usize>,
    dirty: bool,
    done: Option<Option<usize>>,
}

thread_local! {
    static MENU: RefCell<Option<Menu>> = const { RefCell::new(None) };
}

fn measure(gpu: &Gpu, text: &str, size: f32) -> f32 {
    let measured = || -> Result<f32> {
        unsafe {
            let format = gpu.dwrite().CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("en-us"),
            )?;
            let wide: Vec<u16> = text.encode_utf16().collect();
            let layout = gpu.dwrite().CreateTextLayout(&wide, &format, 600.0 * size, 100.0 * size)?;
            let mut m = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut m)?;
            Ok(m.widthIncludingTrailingWhitespace.ceil())
        }
    };
    measured().unwrap_or(text.chars().count() as f32 * size * 0.6)
}

extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEMOVE => {
            let (x, y) = (lparam.0 as i16 as f32, (lparam.0 >> 16) as i16 as f32);
            MENU.with_borrow_mut(|m| {
                if let Some(m) = m {
                    let at = m
                        .rows
                        .iter()
                        .enumerate()
                        .position(|(i, r)| m.entries[i].id != 0 && x >= r.0 && x < r.2 && y >= r.1 && y < r.3);
                    if at != m.hover {
                        m.hover = at;
                        m.dirty = true;
                    }
                }
            });
            LRESULT(0)
        }
        WM_LBUTTONUP | WM_RBUTTONUP => {
            MENU.with_borrow_mut(|m| {
                if let Some(m) = m
                    && let Some(i) = m.hover
                {
                    m.done = Some(Some(m.entries[i].id));
                }
            });
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let key = wparam.0 as u16;
            MENU.with_borrow_mut(|m| {
                let Some(m) = m else { return };
                let n = m.entries.len();
                let step = |from: Option<usize>, dir: isize| -> Option<usize> {
                    let mut i = from.map_or(if dir > 0 { -1 } else { n as isize }, |i| i as isize);
                    for _ in 0..n {
                        i = (i + dir).rem_euclid(n as isize);
                        if m.entries[i as usize].id != 0 {
                            return Some(i as usize);
                        }
                    }
                    None
                };
                match key {
                    k if k == VK_ESCAPE.0 => m.done = Some(None),
                    k if k == VK_DOWN.0 => {
                        m.hover = step(m.hover, 1);
                        m.dirty = true;
                    }
                    k if k == VK_UP.0 => {
                        m.hover = step(m.hover, -1);
                        m.dirty = true;
                    }
                    k if k == VK_RETURN.0 => {
                        if let Some(i) = m.hover {
                            m.done = Some(Some(m.entries[i].id));
                        }
                    }
                    _ => {}
                }
            });
            LRESULT(0)
        }
        WM_ACTIVATE if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE => {
            MENU.with_borrow_mut(|m| {
                if let Some(m) = m
                    && m.done.is_none()
                {
                    m.done = Some(None);
                }
            });
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn create_window() -> HWND {
    static CLASS: Once = Once::new();
    unsafe {
        let Ok(instance) = GetModuleHandleW(None) else { return HWND::default() };
        CLASS.call_once(|| {
            let class = WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                lpszClassName: w!("NightIsland.Menu"),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                ..Default::default()
            };
            RegisterClassW(&class);
        });
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("NightIsland.Menu"),
            w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .unwrap_or_default()
    }
}

fn paint_body(p: &Painter, m: &Menu) -> Result<()> {
    let scale = m.scale;
    let margin = m.margin;
    let body = (margin, margin, m.size.0 as f32 - margin, m.size.1 as f32 - margin);
    let (r, g, b) = theme::panel();
    let lift = |c: f32| c + (1.0 - c) * 0.08;
    let radius = 12.0 * scale;
    for i in 0..6 {
        let grow = i as f32 * 1.6 * scale;
        let a = 0.06 * (1.0 - i as f32 / 6.0);
        p.fill_round_rect(
            (body.0 - grow, body.1 - grow + 3.0 * scale, body.2 + grow, body.3 + grow + 3.0 * scale),
            radius + grow,
            rgba(0.0, 0.0, 0.0, a),
        )?;
    }
    p.fill_round_rect(body, radius, rgba(lift(r), lift(g), lift(b), 0.98))?;
    p.stroke_round_rect(body, radius, 1.0, rgba(1.0, 1.0, 1.0, 0.12))?;
    let (ar, ag, ab) = theme::accent();
    for (i, e) in m.entries.iter().enumerate() {
        let row = m.rows[i];
        if e.id == 0 {
            let y = (row.1 + row.3) / 2.0;
            p.fill_rect((row.0 + 8.0 * scale, y, row.2 - 8.0 * scale, y + 1.0), rgba(1.0, 1.0, 1.0, 0.1))?;
            continue;
        }
        let hot = m.hover == Some(i);
        if hot {
            let tint = if e.danger { rgba(0.95, 0.3, 0.3, 0.22) } else { rgba(ar, ag, ab, 0.28) };
            p.fill_round_rect(row, 8.0 * scale, tint)?;
        }
        let colour =
            if e.danger { rgba(1.0, 0.55, 0.55, 1.0) } else { rgba(1.0, 1.0, 1.0, if hot { 1.0 } else { 0.9 }) };
        if let Some(glyph) = e.glyph {
            p.glyph(glyph, (row.0 + 6.0 * scale, row.1, row.0 + 34.0 * scale, row.3), 15.0 * scale, colour)?;
        }
        p.text(
            &e.label,
            (row.0 + 38.0 * scale, row.1, row.2 - 10.0 * scale, row.3),
            FONT * scale,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            colour,
        )?;
    }
    Ok(())
}

fn render(gpu: &Gpu, hwnd: HWND) {
    let Some(size) = MENU.with_borrow(|m| m.as_ref().map(|m| m.size)) else { return };
    unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size.0,
                biHeight: -size.1,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let dib = CreateDIBSection(Some(mem), &info, DIB_RGB_COLORS, &mut bits, None, 0);
        let Ok(dib) = dib else {
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            return;
        };
        SelectObject(mem, dib.into());
        let painted = MENU.with_borrow(|m| {
            let Some(m) = m else { return Ok(()) };
            let mut result = Ok(());
            let status = gpu.paint_dc(mem, size, |p| result = paint_body(p, m));
            status.and(result)
        });
        if painted.is_ok() {
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let _ = UpdateLayeredWindow(
                hwnd,
                Some(screen),
                None,
                Some(&SIZE { cx: size.0, cy: size.1 }),
                Some(mem),
                Some(&POINT { x: 0, y: 0 }),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
        }
        let _ = DeleteDC(mem);
        let _ = DeleteObject(dib.into());
        ReleaseDC(None, screen);
    }
    MENU.with_borrow_mut(|m| {
        if let Some(m) = m {
            m.dirty = false;
        }
    });
}

pub type WithGpu<'a> = &'a dyn Fn(&mut dyn FnMut(&Gpu));

pub fn show(entries: Vec<Entry>, at: (i32, i32), above: bool, scale: f32, with_gpu: WithGpu) -> Option<usize> {
    if entries.is_empty() || MENU.with_borrow(|m| m.is_some()) {
        return None;
    }
    let margin = (12.0 * scale).ceil();
    let mut widest = 0.0f32;
    with_gpu(&mut |gpu| {
        for e in &entries {
            let label = crate::ui::i18n::tr(&e.label);
            widest = widest.max(measure(gpu, &label, FONT * scale));
        }
    });
    let inner_w = (widest + (38.0 + 14.0) * scale + 2.0 * PAD * scale).max(MIN_W * scale).ceil();
    let mut rows = Vec::with_capacity(entries.len());
    let mut y = margin + PAD * scale;
    for e in &entries {
        let h = if e.id == 0 { DIVIDER_H } else { ITEM_H } * scale;
        rows.push((margin + PAD * scale, y, margin + inner_w - PAD * scale, y + h));
        y += h;
    }
    let inner_h = y + PAD * scale - margin;
    let size = ((inner_w + 2.0 * margin).ceil() as i32, (inner_h + 2.0 * margin).ceil() as i32);
    let first = entries.iter().position(|e| e.id != 0);
    let menu = Menu { entries, scale, rows, margin, size, hover: first, dirty: true, done: None };
    MENU.with_borrow_mut(|m| *m = Some(menu));

    let hwnd = create_window();
    if hwnd.0.is_null() {
        MENU.with_borrow_mut(|m| *m = None);
        return None;
    }
    let (mut x, mut yy) =
        (at.0 - margin as i32, if above { at.1 - size.1 + margin as i32 } else { at.1 - margin as i32 });
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: at.0, y: at.1 }, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(monitor, &mut mi).as_bool() {
            let work = mi.rcWork;
            x = x.clamp(work.left - margin as i32, (work.right - size.0 + margin as i32).max(work.left));
            yy = yy.clamp(work.top - margin as i32, (work.bottom - size.1 + margin as i32).max(work.top));
        }
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), x, yy, 0, 0, SWP_NOSIZE);
    }
    with_gpu(&mut |gpu| render(gpu, hwnd));
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }

    let mut armed = false;
    'outer: loop {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    PostQuitMessage(msg.wParam.0 as i32);
                    MENU.with_borrow_mut(|m| {
                        if let Some(m) = m {
                            m.done.get_or_insert(None);
                        }
                    });
                    break 'outer;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let down = [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON].iter().any(|k| GetAsyncKeyState(k.0 as i32) < 0);
            if !down {
                armed = true;
            } else if armed {
                let (mut pt, mut win) = (POINT::default(), RECT::default());
                let _ = GetCursorPos(&mut pt);
                let _ = GetWindowRect(hwnd, &mut win);
                let m = margin as i32;
                if pt.x < win.left + m || pt.x >= win.right - m || pt.y < win.top + m || pt.y >= win.bottom - m {
                    MENU.with_borrow_mut(|menu| {
                        if let Some(menu) = menu {
                            menu.done.get_or_insert(None);
                        }
                    });
                }
            }
        }
        if MENU.with_borrow(|m| m.as_ref().is_none_or(|m| m.done.is_some())) {
            break;
        }
        if MENU.with_borrow(|m| m.as_ref().is_some_and(|m| m.dirty)) {
            with_gpu(&mut |gpu| render(gpu, hwnd));
        }
        unsafe {
            MsgWaitForMultipleObjects(None, false, 25, QS_ALLINPUT);
        }
    }
    let choice = MENU.with_borrow_mut(|m| m.take()).and_then(|m| m.done.flatten());
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    choice
}
