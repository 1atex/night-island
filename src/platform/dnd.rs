use std::cell::RefCell;
use std::ffi::c_void;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{HWND, LPARAM, POINTL, S_OK, WPARAM};
use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, STGMEDIUM, TYMED_HGLOBAL};
use windows::Win32::System::Ole::{
    CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_NONE, DoDragDrop, IDropSource, IDropSource_Impl,
    IDropTarget, IDropTarget_Impl, OleInitialize, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop,
};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::{
    DragQueryFileW, HDROP, ILClone, ILFindLastID, ILFree, ILRemoveLastID, SHCreateDataObject, SHParseDisplayName,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{BOOL, HRESULT, Ref, Result, implement};

pub const WM_DND_ENTER: u32 = WM_APP + 4;
pub const WM_DND_LEAVE: u32 = WM_APP + 5;
pub const WM_DND_DROP: u32 = WM_APP + 6;

const MAX_FILES_PER_DROP: u32 = 200;
const MAX_PATH_UNITS: u32 = 32_768;
const DRAGDROP_S_DROP: HRESULT = HRESULT(0x0004_0100);
const DRAGDROP_S_CANCEL: HRESULT = HRESULT(0x0004_0101);
const DRAGDROP_S_USEDEFAULTCURSORS: HRESULT = HRESULT(0x0004_0102);

thread_local! {
    static TEXT_DROP: RefCell<Option<crate::data::pocket::PocketItem>> = const { RefCell::new(None) };
    static TEXT_DRAG: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static DROPPED: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

pub const WM_FILE_OVER: u32 = WM_APP + 126;
pub const WM_FILE_LEAVE: u32 = WM_APP + 127;
pub const WM_FILE_PIN: u32 = WM_APP + 128;
thread_local! {
    static PINS: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

pub fn take_pins() -> Vec<PathBuf> {
    PINS.take()
}

pub struct FileRegistration(HWND);

impl FileRegistration {
    pub fn new(hwnd: HWND, dock: HWND) -> Option<Self> {
        unsafe {
            OleInitialize(None).ok()?;
            let target: IDropTarget =
                DockTarget { dock: dock.0 as isize, window: hwnd.0 as isize, files: std::cell::Cell::new(false) }
                    .into();
            if RegisterDragDrop(hwnd, &target).is_err() {
                windows::Win32::System::Ole::OleUninitialize();
                return None;
            }
        }
        Some(Self(hwnd))
    }
}

impl Drop for FileRegistration {
    fn drop(&mut self) {
        unsafe {
            let _ = RevokeDragDrop(self.0);
            windows::Win32::System::Ole::OleUninitialize();
        }
    }
}

#[implement(IDropTarget)]
struct DockTarget {
    dock: isize,
    window: isize,
    files: std::cell::Cell<bool>,
}

impl DockTarget {
    fn over(&self, pt: &POINTL, effect: *mut DROPEFFECT) -> bool {
        let pin = self.files.get()
            && unsafe {
                windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                    HWND(self.dock as _),
                    WM_FILE_OVER,
                    Some(WPARAM(pt.x as usize)),
                    Some(LPARAM(pt.y as isize)),
                )
                .0 != 0
            }
            && self.window == self.dock;
        if !effect.is_null() {
            unsafe {
                let allowed = *effect;
                *effect = if pin && allowed.contains(DROPEFFECT_LINK) {
                    DROPEFFECT_LINK
                } else if pin && allowed.contains(DROPEFFECT_COPY) {
                    DROPEFFECT_COPY
                } else {
                    DROPEFFECT_NONE
                };
            }
        }
        pin && !effect.is_null() && unsafe { *effect != DROPEFFECT_NONE }
    }
    fn leave(&self) {
        self.files.set(false);
        unsafe {
            let _ = PostMessageW(Some(HWND(self.dock as _)), WM_FILE_LEAVE, WPARAM(self.window as usize), LPARAM(0));
        }
    }
}

impl IDropTarget_Impl for DockTarget_Impl {
    fn DragEnter(
        &self,
        data: Ref<IDataObject>,
        _: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> Result<()> {
        self.files.set(data.as_ref().is_some_and(has_files));
        self.over(pt, effect);
        Ok(())
    }
    fn DragOver(&self, _: MODIFIERKEYS_FLAGS, pt: &POINTL, effect: *mut DROPEFFECT) -> Result<()> {
        self.over(pt, effect);
        Ok(())
    }
    fn DragLeave(&self) -> Result<()> {
        self.leave();
        Ok(())
    }
    fn Drop(&self, data: Ref<IDataObject>, _: MODIFIERKEYS_FLAGS, pt: &POINTL, effect: *mut DROPEFFECT) -> Result<()> {
        if self.over(pt, effect) {
            let paths = data.as_ref().map(read_paths).unwrap_or_default();
            if paths.is_empty() {
                unsafe {
                    *effect = DROPEFFECT_NONE;
                }
            } else {
                PINS.with(|pending| pending.borrow_mut().extend(paths));
                unsafe {
                    let _ = PostMessageW(Some(HWND(self.dock as _)), WM_FILE_PIN, WPARAM(0), LPARAM(0));
                }
            }
        }
        self.leave();
        Ok(())
    }
}

pub fn take_dropped() -> Vec<PathBuf> {
    DROPPED.with(|d| std::mem::take(&mut *d.borrow_mut()))
}

pub fn text_dragging() -> bool {
    TEXT_DRAG.get()
}
pub fn take_text() -> Option<crate::data::pocket::PocketItem> {
    TEXT_DROP.take()
}

fn text_format(id: u16) -> FORMATETC {
    FORMATETC {
        cfFormat: id,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}
fn named_format(name: &str) -> u16 {
    unsafe {
        windows::Win32::System::DataExchange::RegisterClipboardFormatW(&windows::core::HSTRING::from(name)) as u16
    }
}
fn has_text(data: &IDataObject) -> bool {
    unsafe {
        data.QueryGetData(&text_format(13)) == S_OK
            || data.QueryGetData(&text_format(named_format("UniformResourceLocatorW"))) == S_OK
    }
}
fn read_data(data: &IDataObject, id: u16) -> Option<Vec<u8>> {
    unsafe {
        let mut medium = data.GetData(&text_format(id)).ok()?;
        let handle = medium.u.hGlobal;
        let size = windows::Win32::System::Memory::GlobalSize(handle);
        let pointer = windows::Win32::System::Memory::GlobalLock(handle);
        let value = if !pointer.is_null() && size > 0 && size <= 65536 {
            Some(std::slice::from_raw_parts(pointer.cast::<u8>(), size).to_vec())
        } else {
            None
        };
        if !pointer.is_null() {
            let _ = windows::Win32::System::Memory::GlobalUnlock(handle);
        }
        ReleaseStgMedium(&mut medium);
        value
    }
}
fn wide_data(data: &IDataObject, id: u16) -> String {
    read_data(data, id)
        .map(|bytes| {
            String::from_utf16_lossy(
                &bytes
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .take_while(|n| *n != 0)
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default()
}
fn read_text(data: &IDataObject) -> Option<crate::data::pocket::PocketItem> {
    use crate::data::pocket::{PocketItem, web_url};
    let text = wide_data(data, 13).chars().take(4096).collect::<String>();
    let mut url = wide_data(data, named_format("UniformResourceLocatorW")).trim_end_matches('\0').trim().to_string();
    if !web_url(&url) {
        url = if web_url(text.trim()) { text.trim().into() } else { String::new() };
    }
    let html = read_data(data, named_format("HTML Format"))
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    let source = html
        .lines()
        .find_map(|line| line.strip_prefix("SourceURL:"))
        .map(str::trim)
        .filter(|url| web_url(url))
        .unwrap_or("")
        .to_string();
    let title = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(&url)
        .chars()
        .filter(|c| !c.is_control())
        .take(160)
        .collect::<String>();
    let kind = if url.is_empty() {
        "text"
    } else if html.contains("<img ") {
        "image"
    } else {
        "link"
    };
    let id = format!("drop-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos());
    let item = PocketItem { id, kind: kind.into(), title, text, url, source };
    item.valid().then_some(item)
}

fn hdrop_format() -> FORMATETC {
    FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

fn has_files(data: &IDataObject) -> bool {
    unsafe { data.QueryGetData(&hdrop_format()) == S_OK }
}

fn read_paths(data: &IDataObject) -> Vec<PathBuf> {
    let mut out = Vec::new();
    unsafe {
        let Ok(mut medium) = data.GetData(&hdrop_format()) else { return out };
        let hdrop = HDROP(medium.u.hGlobal.0);
        let count = DragQueryFileW(hdrop, u32::MAX, None).min(MAX_FILES_PER_DROP);
        for i in 0..count {
            let len = DragQueryFileW(hdrop, i, None);
            if len == 0 || len > MAX_PATH_UNITS {
                continue;
            }
            let mut buf = vec![0u16; len as usize + 1];
            let n = DragQueryFileW(hdrop, i, Some(&mut buf)) as usize;
            out.push(PathBuf::from(String::from_utf16_lossy(&buf[..n.min(buf.len())])));
        }
        ReleaseStgMedium(&mut medium as *mut STGMEDIUM);
    }
    out
}

#[implement(IDropTarget)]
struct Target {
    hwnd: isize,
    accepted: std::cell::Cell<bool>,
}

impl Target {
    fn post(&self, msg: u32) {
        unsafe {
            let _ = PostMessageW(Some(HWND(self.hwnd as *mut c_void)), msg, WPARAM(0), LPARAM(0));
        }
    }

    fn effect(&self, accept: bool, out: *mut DROPEFFECT) {
        if !out.is_null() {
            unsafe { *out = if accept { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
        }
    }
}

impl IDropTarget_Impl for Target_Impl {
    fn DragEnter(
        &self,
        pdataobj: Ref<IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> Result<()> {
        let files = pdataobj.as_ref().is_some_and(has_files);
        let text = !files && pdataobj.as_ref().is_some_and(has_text);
        let accept = files || text;
        crate::log_debug!("drag enter: files={files}, text={text}");
        TEXT_DRAG.set(text);
        self.accepted.set(accept);
        self.effect(accept, pdweffect);
        if accept {
            self.post(WM_DND_ENTER);
        }
        Ok(())
    }

    fn DragOver(&self, _keys: MODIFIERKEYS_FLAGS, _pt: &POINTL, pdweffect: *mut DROPEFFECT) -> Result<()> {
        self.effect(self.accepted.get(), pdweffect);
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
        self.accepted.set(false);
        TEXT_DRAG.set(false);
        self.post(WM_DND_LEAVE);
        Ok(())
    }

    fn Drop(
        &self,
        pdataobj: Ref<IDataObject>,
        _keys: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> Result<()> {
        let paths = pdataobj.as_ref().map(read_paths).unwrap_or_default();
        let text = if paths.is_empty() { pdataobj.as_ref().and_then(read_text) } else { None };
        crate::log_debug!("drop: files={}, text={}", paths.len(), text.is_some());
        self.effect(!paths.is_empty() || text.is_some(), pdweffect);
        TEXT_DROP.set(text);
        self.accepted.set(false);
        TEXT_DRAG.set(false);
        DROPPED.with(|d| d.borrow_mut().extend(paths));
        self.post(WM_DND_DROP);
        Ok(())
    }
}

pub fn register(hwnd: HWND) {
    unsafe {
        if let Err(error) = OleInitialize(None).ok() {
            crate::log_warn!("OLE initialization failed: {error}");
            return;
        }
        let target: IDropTarget = Target { hwnd: hwnd.0 as isize, accepted: std::cell::Cell::new(false) }.into();
        if let Err(error) = RegisterDragDrop(hwnd, &target) {
            crate::log_warn!("Drop target registration failed: {error}");
        }
    }
}

pub fn unregister(hwnd: HWND) {
    unsafe {
        let _ = RevokeDragDrop(hwnd);
    }
}

#[implement(IDropSource)]
struct Source;

impl IDropSource_Impl for Source_Impl {
    fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
        if escape.as_bool() {
            DRAGDROP_S_CANCEL
        } else if keys.0 & MK_LBUTTON.0 == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _effect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

pub(crate) fn file_data(path: &Path) -> Option<IDataObject> {
    let wide: Vec<u16> = path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut pidl = std::ptr::null_mut();
        if SHParseDisplayName(windows::core::PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None).is_err() || pidl.is_null()
        {
            return None;
        }
        let parent = ILClone(pidl);
        let _ = ILRemoveLastID(Some(parent));
        let child = ILFindLastID(pidl);
        let data: Result<IDataObject> =
            SHCreateDataObject(Some(parent as *const _), Some(&[child as *const _]), None::<&IDataObject>);
        ILFree(Some(parent as *const _));
        ILFree(Some(pidl as *const _));
        data.ok()
    }
}

pub fn drag_out(path: &Path) -> bool {
    let Some(data) = file_data(path) else { return false };
    unsafe {
        let source: IDropSource = Source.into();
        let mut effect = DROPEFFECT_NONE;
        let _ = DoDragDrop(&data, &source, DROPEFFECT_COPY | DROPEFFECT_LINK, &mut effect);
    }
    true
}
