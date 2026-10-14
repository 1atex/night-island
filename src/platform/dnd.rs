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
    static DROPPED: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

pub fn take_dropped() -> Vec<PathBuf> {
    DROPPED.with(|d| std::mem::take(&mut *d.borrow_mut()))
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
        let accept = pdataobj.as_ref().is_some_and(has_files);
        self.effect(accept, pdweffect);
        if accept {
            self.post(WM_DND_ENTER);
        }
        Ok(())
    }

    fn DragOver(&self, _keys: MODIFIERKEYS_FLAGS, _pt: &POINTL, pdweffect: *mut DROPEFFECT) -> Result<()> {
        self.effect(true, pdweffect);
        Ok(())
    }

    fn DragLeave(&self) -> Result<()> {
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
        self.effect(!paths.is_empty(), pdweffect);
        DROPPED.with(|d| d.borrow_mut().extend(paths));
        self.post(WM_DND_DROP);
        Ok(())
    }
}

pub fn register(hwnd: HWND) {
    unsafe {
        let _ = OleInitialize(None);
        let target: IDropTarget = Target { hwnd: hwnd.0 as isize }.into();
        let _ = RegisterDragDrop(hwnd, &target);
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

pub fn drag_out(path: &Path) -> bool {
    let wide: Vec<u16> = path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut pidl = std::ptr::null_mut();
        if SHParseDisplayName(windows::core::PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None).is_err() || pidl.is_null()
        {
            return false;
        }
        let parent = ILClone(pidl);
        let _ = ILRemoveLastID(Some(parent));
        let child = ILFindLastID(pidl);
        let data: Result<IDataObject> =
            SHCreateDataObject(Some(parent as *const _), Some(&[child as *const _]), None::<&IDataObject>);
        ILFree(Some(parent as *const _));
        ILFree(Some(pidl as *const _));
        let Ok(data) = data else { return false };

        let source: IDropSource = Source.into();
        let mut effect = DROPEFFECT_NONE;
        let _ = DoDragDrop(&data, &source, DROPEFFECT_COPY | DROPEFFECT_LINK, &mut effect);
    }
    true
}
