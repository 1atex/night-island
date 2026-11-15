use std::cell::RefCell;
use std::path::PathBuf;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::Shell::{FOLDERID_Pictures, KF_FLAG_CREATE, SHGetKnownFolderPath};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::data::clips;
use crate::ui::markup::{self, Annot, Item, Renderer, Tool, Toolbar};

pub const WM_CAPTURE_DONE: u32 = WM_APP + 9;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Area,
    Picker,
    Ocr,
    Qr,
    Record,
}

pub enum Outcome {
    Screenshot(Option<PathBuf>),
    Color(String),
    Ocr(Vec<u8>, i32, i32),
    Qr(Vec<u8>, i32, i32),
    Record(RECT),
    Failed,
    Cancelled,
}

thread_local! {
    static OUTCOME: RefCell<Option<Outcome>> = const { RefCell::new(None) };
    static OVERLAY: RefCell<Option<Overlay>> = const { RefCell::new(None) };
}

pub fn take_outcome() -> Option<Outcome> {
    OUTCOME.with(|o| o.borrow_mut().take())
}

struct Grab {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *const u8,
    origin: POINT,
    w: i32,
    h: i32,
}

impl Grab {
    fn pixel(&self, x: i32, y: i32) -> (u8, u8, u8) {
        let x = x.clamp(0, self.w - 1) as usize;
        let y = y.clamp(0, self.h - 1) as usize;
        let i = (y * self.w as usize + x) * 4;
        unsafe { (*self.bits.add(i + 2), *self.bits.add(i + 1), *self.bits.add(i)) }
    }

    fn region(&self, r: RECT) -> Option<(Vec<u8>, i32, i32)> {
        let (l, t) = (r.left.clamp(0, self.w), r.top.clamp(0, self.h));
        let (rt, b) = (r.right.clamp(0, self.w), r.bottom.clamp(0, self.h));
        let (w, h) = (rt - l, b - t);
        if w <= 0 || h <= 0 {
            return None;
        }
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in t..b {
            let start = (y as usize * self.w as usize + l as usize) * 4;
            let row = unsafe { std::slice::from_raw_parts(self.bits.add(start), w as usize * 4) };
            out.extend_from_slice(row);
        }
        for px in out.chunks_exact_mut(4) {
            px[3] = 255;
        }
        Some((out, w, h))
    }
}

impl Drop for Grab {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

static HIDE_SELF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_hide_self(on: bool) {
    HIDE_SELF.store(on, std::sync::atomic::Ordering::Relaxed);
}

fn grab_screen(owner: HWND) -> Option<Grab> {
    unsafe {
        let hide = HIDE_SELF.load(std::sync::atomic::Ordering::Relaxed);
        let mut previous = 0u32;
        let _ = GetWindowDisplayAffinity(owner, &mut previous);
        if hide {
            let _ = SetWindowDisplayAffinity(owner, WDA_EXCLUDEFROMCAPTURE);
            let _ = DwmFlush();
            let _ = DwmFlush();
        }

        let origin = POINT { x: GetSystemMetrics(SM_XVIRTUALSCREEN), y: GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let (w, h) = (GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN));

        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0);
        let result = bitmap.ok().filter(|_| !bits.is_null()).map(|bitmap| {
            let old = SelectObject(dc, bitmap.into());
            let _ = BitBlt(dc, 0, 0, w, h, Some(screen), origin.x, origin.y, SRCCOPY | CAPTUREBLT);
            Grab { dc, bitmap, old, bits: bits as *const u8, origin, w, h }
        });
        ReleaseDC(None, screen);
        if result.is_none() {
            let _ = DeleteDC(dc);
        }
        if hide {
            let _ = SetWindowDisplayAffinity(owner, WINDOW_DISPLAY_AFFINITY(previous));
        }
        result
    }
}

fn pictures_dir() -> Option<PathBuf> {
    unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_Pictures, KF_FLAG_CREATE, None).ok()?;
        let dir = PathBuf::from(path.to_string().ok()?);
        windows::Win32::System::Com::CoTaskMemFree(Some(path.0 as *const _));
        Some(dir)
    }
}

fn file_name() -> String {
    let t = unsafe { GetLocalTime() };
    format!("Screenshot_{:04}{:02}{:02}_{:02}{:02}{:02}.png", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

pub fn shrink(bgra: &[u8], w: i32, h: i32, max_side: i32) -> (Vec<u8>, i32, i32) {
    let longest = w.max(h);
    if max_side <= 0 || longest <= max_side {
        return (bgra.to_vec(), w, h);
    }
    let (nw, nh) = (((w * max_side) / longest).max(1), ((h * max_side) / longest).max(1));
    let mut out = Vec::with_capacity((nw * nh * 4) as usize);
    for y in 0..nh {
        let (y0, y1) = (y * h / nh, ((y + 1) * h / nh).max(y * h / nh + 1).min(h));
        for x in 0..nw {
            let (x0, x1) = (x * w / nw, ((x + 1) * w / nw).max(x * w / nw + 1).min(w));
            let mut sum = [0u32; 3];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = ((sy * w + sx) * 4) as usize;
                    sum[0] += u32::from(bgra[i]);
                    sum[1] += u32::from(bgra[i + 1]);
                    sum[2] += u32::from(bgra[i + 2]);
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            out.extend_from_slice(&[(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8, 255]);
        }
    }
    (out, nw, nh)
}

pub fn screen_png(owner: HWND, max_side: i32) -> Option<Vec<u8>> {
    let grab = grab_screen(owner)?;
    let (left, top) = (-grab.origin.x, -grab.origin.y);
    let (width, height) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
    let (bgra, w, h) = grab.region(RECT { left, top, right: left + width, bottom: top + height })?;
    let (small, nw, nh) = shrink(&bgra, w, h, max_side);
    encode_png(&small, nw, nh)
}

fn encode_png(bgra: &[u8], w: i32, h: i32) -> Option<Vec<u8>> {
    let rgba: Vec<u8> = bgra.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w as u32, h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    let mut writer = enc.write_header().ok()?;
    writer.write_image_data(&rgba).ok()?;
    writer.finish().ok()?;
    Some(out)
}

fn encode_dib(bgra: &[u8], w: i32, h: i32) -> Vec<u8> {
    let header = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w,
        biHeight: h,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        biSizeImage: (w * h * 4) as u32,
        ..Default::default()
    };
    let mut out = Vec::with_capacity(size_of::<BITMAPINFOHEADER>() + bgra.len());
    out.extend_from_slice(unsafe {
        std::slice::from_raw_parts(&header as *const _ as *const u8, size_of::<BITMAPINFOHEADER>())
    });
    for row in bgra.chunks_exact(w as usize * 4).rev() {
        out.extend_from_slice(row);
    }
    out
}

fn publish_pixels(owner: HWND, bgra: &[u8], w: i32, h: i32) -> Outcome {
    let Some(png) = encode_png(bgra, w, h) else { return Outcome::Failed };
    if !clips::sys::write_image(owner, &encode_dib(bgra, w, h), &png) {
        return Outcome::Failed;
    }
    let path = pictures_dir().map(|d| d.join("Screenshots").join("NightIsland").join(file_name()));
    if let Some(path) = path.clone() {
        std::thread::spawn(move || {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, png);
        });
    }
    Outcome::Screenshot(path)
}

fn publish(owner: HWND, grab: &Grab, rect: RECT) -> Outcome {
    match grab.region(rect) {
        Some((bgra, w, h)) => publish_pixels(owner, &bgra, w, h),
        None => Outcome::Cancelled,
    }
}

fn finish(owner: HWND, outcome: Outcome) {
    OUTCOME.with(|o| *o.borrow_mut() = Some(outcome));
    unsafe {
        let _ = PostMessageW(Some(owner), WM_CAPTURE_DONE, WPARAM(0), LPARAM(0));
    }
}

pub fn fullscreen(owner: HWND) {
    let Some(grab) = grab_screen(owner) else { return finish(owner, Outcome::Failed) };
    let outcome = unsafe {
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let r = info.rcMonitor;
            let rect = RECT {
                left: r.left - grab.origin.x,
                top: r.top - grab.origin.y,
                right: r.right - grab.origin.x,
                bottom: r.bottom - grab.origin.y,
            };
            publish(owner, &grab, rect)
        } else {
            Outcome::Failed
        }
    };
    finish(owner, outcome);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Selecting,
    Markup,
}

struct Overlay {
    owner: HWND,
    mode: Mode,
    grab: Grab,
    back_dc: HDC,
    back_bitmap: HBITMAP,
    back_old: HGDIOBJ,
    dim_dc: HDC,
    dim_bitmap: HBITMAP,
    dim_old: HGDIOBJ,
    phase: Phase,
    anchor: POINT,
    cursor: POINT,
    sel: RECT,
    renderer: Option<Renderer>,
    tool: Tool,
    color: usize,
    annots: Vec<Annot>,
    current: Option<Annot>,
    typing: bool,
    bar: Option<Toolbar>,
    dirty: RECT,
}

const DIM_ALPHA: u8 = 150;
const MAGNIFIER_CELLS: i32 = 11;
const MAGNIFIER_ZOOM: i32 = 12;

fn normalized(a: POINT, b: POINT) -> RECT {
    RECT { left: a.x.min(b.x), top: a.y.min(b.y), right: a.x.max(b.x) + 1, bottom: a.y.max(b.y) + 1 }
}

fn union(a: RECT, b: RECT) -> RECT {
    RECT {
        left: a.left.min(b.left),
        top: a.top.min(b.top),
        right: a.right.max(b.right),
        bottom: a.bottom.max(b.bottom),
    }
}

fn inflate(r: RECT, by: i32) -> RECT {
    RECT { left: r.left - by, top: r.top - by, right: r.right + by, bottom: r.bottom + by }
}

fn magnifier_rect(ov: &Overlay) -> RECT {
    let size = MAGNIFIER_CELLS * MAGNIFIER_ZOOM;
    let (mut x, mut y) = (ov.cursor.x + 24, ov.cursor.y + 24);
    if x + size + 8 > ov.grab.w {
        x = ov.cursor.x - 24 - size;
    }
    if y + size + 40 > ov.grab.h {
        y = ov.cursor.y - 24 - size - 32;
    }
    RECT { left: x, top: y, right: x + size, bottom: y + size + 32 }
}

pub fn start(owner: HWND, mode: Mode) -> bool {
    if OVERLAY.with(|o| o.borrow().is_some()) {
        return false;
    }
    let Some(grab) = grab_screen(owner) else {
        finish(owner, Outcome::Failed);
        return false;
    };
    unsafe {
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let class = w!("NightIsland.Capture");
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(overlay_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let screen = GetDC(None);
        let back_dc = CreateCompatibleDC(Some(screen));
        let back_bitmap = CreateCompatibleBitmap(screen, grab.w, grab.h);
        let back_old = SelectObject(back_dc, back_bitmap.into());
        let dim_dc = CreateCompatibleDC(Some(screen));
        let dim_bitmap = CreateCompatibleBitmap(screen, 1, 1);
        let dim_old = SelectObject(dim_dc, dim_bitmap.into());
        let brush = CreateSolidBrush(COLORREF(0));
        FillRect(dim_dc, &RECT { left: 0, top: 0, right: 1, bottom: 1 }, brush);
        let _ = DeleteObject(brush.into());
        ReleaseDC(None, screen);

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let cursor = POINT { x: pt.x - grab.origin.x, y: pt.y - grab.origin.y };
        let (ox, oy, ow, oh) = (grab.origin.x, grab.origin.y, grab.w, grab.h);
        let overlay = Overlay {
            owner,
            mode,
            grab,
            back_dc,
            back_bitmap,
            back_old,
            dim_dc,
            dim_bitmap,
            dim_old,
            phase: Phase::Idle,
            anchor: cursor,
            cursor,
            sel: RECT::default(),
            renderer: Renderer::new(),
            tool: Tool::Arrow,
            color: 0,
            annots: Vec::new(),
            current: None,
            typing: false,
            bar: None,
            dirty: RECT::default(),
        };
        OVERLAY.with(|o| *o.borrow_mut() = Some(overlay));

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class,
            w!("Night Island capture"),
            WS_POPUP,
            ox,
            oy,
            ow,
            oh,
            None,
            None,
            Some(instance.into()),
            None,
        );
        let Ok(hwnd) = hwnd else {
            OVERLAY.with(|o| o.borrow_mut().take());
            finish(owner, Outcome::Failed);
            return false;
        };
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(hwnd));
    }
    true
}

fn close(hwnd: HWND, outcome: Outcome) {
    let owner = OVERLAY.with(|o| o.borrow().as_ref().map(|ov| ov.owner));
    let Some(owner) = owner else { return };
    let outcome = match (&outcome, OVERLAY.with(|o| o.borrow_mut().take())) {
        (_, Some(ov)) => {
            let result = match (outcome, ov.mode) {
                (Outcome::Screenshot(_), Mode::Area) => {
                    let burned = ov.renderer.as_ref().and_then(|r| markup::burn(r, ov.grab.dc, ov.sel, &ov.annots));
                    match burned {
                        Some((bgra, w, h)) => publish_pixels(ov.owner, &bgra, w, h),
                        None => publish(ov.owner, &ov.grab, ov.sel),
                    }
                }
                (Outcome::Screenshot(_), Mode::Record) => Outcome::Record(RECT {
                    left: ov.sel.left + ov.grab.origin.x,
                    top: ov.sel.top + ov.grab.origin.y,
                    right: ov.sel.right + ov.grab.origin.x,
                    bottom: ov.sel.bottom + ov.grab.origin.y,
                }),
                (Outcome::Screenshot(_), m @ (Mode::Ocr | Mode::Qr)) => match ov.grab.region(ov.sel) {
                    Some((bgra, w, h)) if m == Mode::Ocr => Outcome::Ocr(bgra, w, h),
                    Some((bgra, w, h)) => Outcome::Qr(bgra, w, h),
                    None => Outcome::Cancelled,
                },
                (Outcome::Color(_), _) => {
                    let (r, g, b) = ov.grab.pixel(ov.cursor.x, ov.cursor.y);
                    let hex = format!("#{r:02X}{g:02X}{b:02X}");
                    clips::sys::write_text(ov.owner, &hex);
                    Outcome::Color(hex)
                }
                (other, _) => other,
            };
            unsafe {
                SelectObject(ov.back_dc, ov.back_old);
                let _ = DeleteObject(ov.back_bitmap.into());
                let _ = DeleteDC(ov.back_dc);
                SelectObject(ov.dim_dc, ov.dim_old);
                let _ = DeleteObject(ov.dim_bitmap.into());
                let _ = DeleteDC(ov.dim_dc);
            }
            result
        }
        (_, None) => outcome,
    };
    unsafe {
        let _ = ReleaseCapture();
        let _ = DestroyWindow(hwnd);
    }
    finish(owner, outcome);
}

fn paint(hwnd: HWND, ov: &Overlay) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let inv = ps.rcPaint;
        let (w, h) = (inv.right - inv.left, inv.bottom - inv.top);
        if w > 0 && h > 0 {
            BitBlt(ov.back_dc, inv.left, inv.top, w, h, Some(ov.grab.dc), inv.left, inv.top, SRCCOPY).ok();
            match ov.mode {
                Mode::Picker => paint_picker(ov),
                _ => paint_area(ov, inv),
            }
            BitBlt(hdc, inv.left, inv.top, w, h, Some(ov.back_dc), inv.left, inv.top, SRCCOPY).ok();
        }
        let _ = EndPaint(hwnd, &ps);
    }
}

fn dim(ov: &Overlay, r: RECT) {
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    if w <= 0 || h <= 0 {
        return;
    }
    let blend = BLENDFUNCTION { BlendOp: 0, BlendFlags: 0, SourceConstantAlpha: DIM_ALPHA, AlphaFormat: 0 };
    unsafe {
        let _ = AlphaBlend(ov.back_dc, r.left, r.top, w, h, ov.dim_dc, 0, 0, 1, 1, blend);
    }
}

fn text(dc: HDC, s: &str, x: i32, y: i32, color: u32) {
    unsafe {
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(color));
        let font = CreateFontW(
            18,
            0,
            0,
            0,
            600,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Segoe UI"),
        );
        let old = SelectObject(dc, font.into());
        let wide: Vec<u16> = s.encode_utf16().collect();
        let _ = TextOutW(dc, x, y, &wide);
        SelectObject(dc, old);
        let _ = DeleteObject(font.into());
    }
}

fn dim_outside(ov: &Overlay, inv: RECT, sel: RECT) {
    let top = RECT { bottom: inv.bottom.min(sel.top), ..inv };
    let bottom = RECT { top: inv.top.max(sel.bottom), ..inv };
    let (mid_t, mid_b) = (inv.top.max(sel.top), inv.bottom.min(sel.bottom));
    let left = RECT { top: mid_t, bottom: mid_b, right: inv.right.min(sel.left), ..inv };
    let right = RECT { top: mid_t, bottom: mid_b, left: inv.left.max(sel.right), ..inv };
    for r in [top, bottom, left, right] {
        dim(ov, r);
    }
}

fn border(ov: &Overlay, sel: RECT) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 2, COLORREF(0xFFB4A6));
        let old_pen = SelectObject(ov.back_dc, pen.into());
        let old_brush = SelectObject(ov.back_dc, GetStockObject(NULL_BRUSH));
        let _ = Rectangle(ov.back_dc, sel.left - 1, sel.top - 1, sel.right + 1, sel.bottom + 1);
        SelectObject(ov.back_dc, old_brush);
        SelectObject(ov.back_dc, old_pen);
        let _ = DeleteObject(pen.into());
    }
}

fn paint_area(ov: &Overlay, inv: RECT) {
    match ov.phase {
        Phase::Idle => {
            dim(ov, inv);
            let hint = match ov.mode {
                Mode::Ocr => "Select the text to read  ·  Esc to cancel",
                Mode::Qr => "Select a QR code  ·  Esc to cancel",
                Mode::Record => "Select the area to record  ·  Esc to cancel",
                _ => "Drag to select an area  ·  Esc to cancel",
            };
            text(ov.back_dc, hint, ov.cursor.x + 20, ov.cursor.y + 20, 0xFFFFFF);
        }
        Phase::Selecting => {
            let sel = normalized(ov.anchor, ov.cursor);
            dim_outside(ov, inv, sel);
            border(ov, sel);
            let label = format!("{} × {}", sel.right - sel.left, sel.bottom - sel.top);
            let ly = if sel.top > 28 { sel.top - 26 } else { sel.bottom + 6 };
            text(ov.back_dc, &label, sel.left + 2, ly, 0xFFFFFF);
        }
        Phase::Markup => {
            dim_outside(ov, inv, ov.sel);
            border(ov, ov.sel);
            let Some(renderer) = &ov.renderer else { return };
            let clip = RECT {
                left: ov.sel.left.max(inv.left),
                top: ov.sel.top.max(inv.top),
                right: ov.sel.right.min(inv.right),
                bottom: ov.sel.bottom.min(inv.bottom),
            };
            let bind = RECT { left: 0, top: 0, right: ov.grab.w, bottom: ov.grab.h };
            renderer.render(
                ov.back_dc,
                bind,
                POINT::default(),
                ov.grab.dc,
                &ov.annots,
                ov.current.as_ref(),
                ov.typing,
                clip,
                ov.bar.as_ref().map(|b| (b, ov.tool, ov.color)),
            );
        }
    }
}

fn paint_picker(ov: &Overlay) {
    let m = magnifier_rect(ov);
    let size = MAGNIFIER_CELLS * MAGNIFIER_ZOOM;
    let half = MAGNIFIER_CELLS / 2;
    unsafe {
        SetStretchBltMode(ov.back_dc, COLORONCOLOR);
        let _ = StretchBlt(
            ov.back_dc,
            m.left,
            m.top,
            size,
            size,
            Some(ov.grab.dc),
            ov.cursor.x - half,
            ov.cursor.y - half,
            MAGNIFIER_CELLS,
            MAGNIFIER_CELLS,
            SRCCOPY,
        );
        let frame = CreateSolidBrush(COLORREF(0xFFFFFF));
        FrameRect(ov.back_dc, &inflate(RECT { right: m.left + size, bottom: m.top + size, ..m }, 1), frame);
        let centre = RECT {
            left: m.left + half * MAGNIFIER_ZOOM,
            top: m.top + half * MAGNIFIER_ZOOM,
            right: m.left + (half + 1) * MAGNIFIER_ZOOM,
            bottom: m.top + (half + 1) * MAGNIFIER_ZOOM,
        };
        FrameRect(ov.back_dc, &centre, frame);
        let _ = DeleteObject(frame.into());

        let (r, g, b) = ov.grab.pixel(ov.cursor.x, ov.cursor.y);
        let swatch = CreateSolidBrush(COLORREF(u32::from(r) | u32::from(g) << 8 | u32::from(b) << 16));
        let sw = RECT { left: m.left, top: m.top + size + 6, right: m.left + 22, bottom: m.top + size + 28 };
        FillRect(ov.back_dc, &sw, swatch);
        let _ = DeleteObject(swatch.into());
        let hex = format!("#{r:02X}{g:02X}{b:02X}");
        let plate = CreateSolidBrush(COLORREF(0x202020));
        FillRect(
            ov.back_dc,
            &RECT { left: m.left + 26, top: m.top + size + 6, right: m.left + size, bottom: m.top + size + 28 },
            plate,
        );
        let _ = DeleteObject(plate.into());
        text(ov.back_dc, &hex, m.left + 32, m.top + size + 6, 0xFFFFFF);
    }
}

fn refresh(hwnd: HWND, ov: &mut Overlay) {
    let fresh = match (ov.mode, ov.phase) {
        (Mode::Area | Mode::Ocr | Mode::Qr | Mode::Record, Phase::Selecting) => {
            inflate(normalized(ov.anchor, ov.cursor), 40)
        }
        (Mode::Area, Phase::Markup) => {
            let bar = ov.bar.as_ref().map_or(ov.sel, |b| b.rect);
            inflate(union(ov.sel, bar), 10)
        }
        (Mode::Area | Mode::Ocr | Mode::Qr | Mode::Record, Phase::Idle) => {
            inflate(RECT { left: ov.cursor.x, top: ov.cursor.y, right: ov.cursor.x + 320, bottom: ov.cursor.y + 44 }, 4)
        }
        (Mode::Picker, _) => inflate(magnifier_rect(ov), 4),
        (Mode::Ocr | Mode::Qr | Mode::Record, Phase::Markup) => inflate(ov.sel, 10),
    };
    let area = union(ov.dirty, fresh);
    ov.dirty = fresh;
    unsafe {
        let _ = InvalidateRect(Some(hwnd), Some(&area), false);
    }
}

fn with_overlay<R>(f: impl FnOnce(&mut Overlay) -> R) -> Option<R> {
    OVERLAY.with(|o| o.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

fn commit(ov: &mut Overlay) {
    if let Some(a) = ov.current.take()
        && a.is_visible()
    {
        ov.annots.push(a);
    }
    ov.typing = false;
}

enum Next {
    Nothing,
    Confirm,
    Cancel,
}

fn press_markup(hwnd: HWND, ov: &mut Overlay, p: POINT) -> Next {
    if let Some(item) = ov.bar.as_ref().and_then(|b| b.hit(p.x, p.y)) {
        match item {
            Item::Tool(t) => {
                commit(ov);
                ov.tool = t;
            }
            Item::Color(i) => {
                ov.color = i;
                if let Some(c) = ov.current.as_mut() {
                    c.color = i;
                }
            }
            Item::Undo => {
                commit(ov);
                ov.annots.pop();
            }
            Item::Confirm => return Next::Confirm,
            Item::Cancel => return Next::Cancel,
        }
        refresh(hwnd, ov);
        return Next::Nothing;
    }
    if ov.bar.as_ref().is_some_and(|b| b.contains(p.x, p.y)) {
        return Next::Nothing;
    }
    let inside = p.x >= ov.sel.left && p.x < ov.sel.right && p.y >= ov.sel.top && p.y < ov.sel.bottom;
    if !inside {
        commit(ov);
        refresh(hwnd, ov);
        return Next::Nothing;
    }
    commit(ov);
    ov.current = Some(Annot::new(ov.tool, ov.color, p));
    ov.typing = ov.tool == Tool::Text;
    if !ov.typing {
        unsafe {
            SetCapture(hwnd);
        }
    }
    refresh(hwnd, ov);
    Next::Nothing
}

extern "system" fn overlay_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let point = |l: LPARAM| POINT { x: l.0 as i16 as i32, y: (l.0 >> 16) as i16 as i32 };
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            OVERLAY.with(|o| {
                if let Some(ov) = o.borrow().as_ref() {
                    paint(hwnd, ov);
                }
            });
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let p = point(lparam);
            with_overlay(|ov| {
                ov.cursor = p;
                if ov.phase == Phase::Markup
                    && let Some(c) = ov.current.as_mut()
                    && !ov.typing
                {
                    c.b = p;
                }
                refresh(hwnd, ov);
            });
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let p = point(lparam);
            let next = with_overlay(|ov| {
                ov.cursor = p;
                match (ov.mode, ov.phase) {
                    (Mode::Picker, _) => Next::Confirm,
                    (Mode::Area | Mode::Ocr | Mode::Qr | Mode::Record, Phase::Idle) => {
                        ov.anchor = p;
                        ov.phase = Phase::Selecting;
                        unsafe {
                            SetCapture(hwnd);
                        }
                        Next::Nothing
                    }
                    (Mode::Area, Phase::Markup) => press_markup(hwnd, ov, p),
                    _ => Next::Nothing,
                }
            });
            match (next, OVERLAY.with(|o| o.borrow().as_ref().map(|ov| ov.mode))) {
                (Some(Next::Confirm), Some(Mode::Picker)) => close(hwnd, Outcome::Color(String::new())),
                (Some(Next::Confirm), _) => close(hwnd, Outcome::Screenshot(None)),
                (Some(Next::Cancel), _) => close(hwnd, Outcome::Cancelled),
                _ => {}
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let p = point(lparam);
            let finish = with_overlay(|ov| {
                ov.cursor = p;
                match ov.phase {
                    Phase::Selecting => {
                        let sel = normalized(ov.anchor, ov.cursor);
                        unsafe {
                            let _ = ReleaseCapture();
                        }
                        if sel.right - sel.left >= 4 && sel.bottom - sel.top >= 4 {
                            ov.sel = sel;
                            if ov.mode != Mode::Area {
                                return true;
                            }
                            ov.phase = Phase::Markup;
                            ov.bar = Some(markup::toolbar(sel, ov.grab.w, ov.grab.h));
                            ov.dirty = RECT::default();
                        } else {
                            ov.phase = Phase::Idle;
                        }
                        unsafe {
                            let _ = InvalidateRect(Some(hwnd), None, false);
                        }
                    }
                    Phase::Markup if !ov.typing && ov.current.is_some() => {
                        unsafe {
                            let _ = ReleaseCapture();
                        }
                        if let Some(c) = ov.current.as_mut() {
                            c.b = p;
                        }
                        commit(ov);
                        refresh(hwnd, ov);
                    }
                    _ => {}
                }
                false
            });
            if finish == Some(true) {
                close(hwnd, Outcome::Screenshot(None));
            }
            LRESULT(0)
        }
        WM_RBUTTONDOWN => {
            close(hwnd, Outcome::Cancelled);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let vk = wparam.0 as u16;
            let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
            let action = with_overlay(|ov| {
                let markup = ov.mode == Mode::Area && ov.phase == Phase::Markup;
                if vk == VK_ESCAPE.0 {
                    if ov.typing {
                        ov.current = None;
                        ov.typing = false;
                        refresh(hwnd, ov);
                        return Next::Nothing;
                    }
                    return Next::Cancel;
                }
                if markup && vk == VK_RETURN.0 {
                    if ov.typing {
                        commit(ov);
                        refresh(hwnd, ov);
                        return Next::Nothing;
                    }
                    return Next::Confirm;
                }
                if markup && ctrl && vk == b'Z' as u16 {
                    commit(ov);
                    ov.annots.pop();
                    refresh(hwnd, ov);
                }
                Next::Nothing
            });
            match action {
                Some(Next::Cancel) => close(hwnd, Outcome::Cancelled),
                Some(Next::Confirm) => close(hwnd, Outcome::Screenshot(None)),
                _ => {}
            }
            LRESULT(0)
        }
        WM_CHAR => {
            let unit = wparam.0 as u16;
            with_overlay(|ov| {
                if !ov.typing {
                    return;
                }
                if let Some(c) = ov.current.as_mut() {
                    match unit {
                        0x08 => {
                            c.text.pop();
                        }
                        u if u >= 0x20 && u != 0x7F => {
                            if let Some(ch) = char::from_u32(u as u32) {
                                c.push_char(ch);
                            }
                        }
                        _ => {}
                    }
                    refresh(hwnd, ov);
                }
            });
            LRESULT(0)
        }
        WM_ACTIVATE if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE => {
            close(hwnd, Outcome::Cancelled);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(w: i32, h: i32, bgr: [u8; 3]) -> Vec<u8> {
        (0..w * h).flat_map(|_| [bgr[0], bgr[1], bgr[2], 255]).collect()
    }

    #[test]
    fn a_small_picture_is_left_alone() {
        let src = flat(8, 4, [1, 2, 3]);
        let (out, w, h) = shrink(&src, 8, 4, 100);
        assert_eq!((w, h, out), (8, 4, src));
    }

    #[test]
    fn a_big_picture_is_averaged_down_with_its_proportions() {
        let src = flat(1920, 1080, [10, 20, 30]);
        let (out, w, h) = shrink(&src, 1920, 1080, 960);
        assert_eq!((w, h), (960, 540));
        assert_eq!(out.len(), 960 * 540 * 4);
        assert!(out.chunks_exact(4).all(|p| p == [10, 20, 30, 255]));
        let (_, w, h) = shrink(&flat(100, 1000, [0, 0, 0]), 100, 1000, 10);
        assert_eq!((w, h), (1, 10));
    }

    #[test]
    fn averaging_blends_neighbours() {
        let mut src = flat(2, 1, [0, 0, 0]);
        src[4..7].copy_from_slice(&[200, 100, 50]);
        let (out, w, h) = shrink(&src, 2, 1, 1);
        assert_eq!((w, h), (1, 1));
        assert_eq!(&out[..4], &[100, 50, 25, 255]);
    }

    #[test]
    fn nonsense_limits_do_not_panic() {
        let src = flat(4, 4, [9, 9, 9]);
        assert_eq!(shrink(&src, 4, 4, 0).1, 4);
        assert_eq!(shrink(&src, 4, 4, -5).1, 4);
        assert_eq!(shrink(&src, 4, 4, 1).1, 1);
    }

    #[test]
    #[ignore = "needs a desktop"]
    fn the_primary_screen_becomes_a_png() {
        let png = screen_png(HWND::default(), 1280).expect("a screenshot");
        assert_eq!(&png[1..4], b"PNG");
        println!("{} bytes", png.len());
    }
}
