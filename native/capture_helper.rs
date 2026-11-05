use std::ffi::c_void;

type Hwnd = *mut c_void;

#[repr(C)]
struct Message {
    lparam: isize,
    wparam: usize,
    message: u32,
    hwnd: Hwnd,
}

const REQUEST: &[u16] = &wide(b"NightIsland.Capture.Request\0");
const RESULT: &[u16] = &wide(b"NightIsland.Capture.Result\0");
const ORIGINAL: &[u16] = &wide(b"NightIsland.Capture.Original\0");
const MESSAGE: &[u16] = &wide(b"NightIsland.CapturePrivacy.v1\0");

const fn wide<const N: usize>(input: &[u8; N]) -> [u16; N] {
    let mut output = [0; N];
    let mut i = 0;
    while i < N {
        output[i] = input[i] as u16;
        i += 1;
    }
    output
}

#[link(name = "user32")]
unsafe extern "system" {
    fn CallNextHookEx(hook: *mut c_void, code: i32, wparam: usize, lparam: isize) -> isize;
    fn RegisterWindowMessageW(name: *const u16) -> u32;
    fn GetPropW(hwnd: Hwnd, name: *const u16) -> *mut c_void;
    fn SetPropW(hwnd: Hwnd, name: *const u16, value: *mut c_void) -> i32;
    fn RemovePropW(hwnd: Hwnd, name: *const u16) -> *mut c_void;
    fn SetWindowDisplayAffinity(hwnd: Hwnd, value: u32) -> i32;
    fn GetWindowDisplayAffinity(hwnd: Hwnd, value: *mut u32) -> i32;
    fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
    fn GetAncestor(hwnd: Hwnd, flags: u32) -> Hwnd;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcessId() -> u32;
}

unsafe fn apply(hwnd: Hwnd, enable: bool) -> usize {
    unsafe {
        let mut original = 0;
        if GetWindowDisplayAffinity(hwnd, &mut original) == 0 {
            return 4;
        }
        let saved = GetPropW(hwnd, ORIGINAL.as_ptr()) as usize;
        if enable && original == 0x11 {
            return 3;
        }
        if !enable && saved == 0 {
            return 2;
        }
        if !enable && original != 0x11 {
            RemovePropW(hwnd, ORIGINAL.as_ptr());
            return 2;
        }
        let desired = if enable { 0x11 } else { (saved - 1) as u32 };
        let added = enable && saved == 0;
        if added && SetPropW(hwnd, ORIGINAL.as_ptr(), (original as usize + 1) as _) == 0 {
            return 4;
        }
        if SetWindowDisplayAffinity(hwnd, desired) == 0 {
            if added {
                RemovePropW(hwnd, ORIGINAL.as_ptr());
            }
            return 4;
        }
        let mut actual = 0;
        if GetWindowDisplayAffinity(hwnd, &mut actual) == 0 || actual != desired {
            if SetWindowDisplayAffinity(hwnd, original) != 0 && added {
                RemovePropW(hwnd, ORIGINAL.as_ptr());
            }
            return 4;
        }
        if !enable {
            RemovePropW(hwnd, ORIGINAL.as_ptr());
        }
        if enable { 1 } else { 2 }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn IslandCaptureHook(code: i32, wparam: usize, lparam: isize) -> isize {
    unsafe {
        if code == 0 && lparam != 0 {
            let message = &*(lparam as *const Message);
            if message.message == RegisterWindowMessageW(MESSAGE.as_ptr())
                && message.lparam != 0
                && message.lparam == GetPropW(message.hwnd, REQUEST.as_ptr()) as isize
                && matches!(message.wparam, 0 | 0x11)
                && GetAncestor(message.hwnd, 2) == message.hwnd
            {
                let mut pid = 0;
                GetWindowThreadProcessId(message.hwnd, &mut pid);
                if pid == GetCurrentProcessId() {
                    RemovePropW(message.hwnd, REQUEST.as_ptr());
                    let result = apply(message.hwnd, message.wparam == 0x11);
                    SetPropW(message.hwnd, RESULT.as_ptr(), result as _);
                }
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }
}
