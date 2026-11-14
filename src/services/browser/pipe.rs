use super::{
    Command, SHARED, Snapshot,
    model::{Frame, MAX_FRAME, parse},
    update,
};
use std::collections::{HashSet, VecDeque};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, GetLastError, HANDLE, HLOCAL, HWND, INVALID_HANDLE_VALUE,
    LocalFree,
};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
};
use windows::Win32::System::IO::{
    CancelIoEx, CreateIoCompletionPort, GetOverlappedResult, GetQueuedCompletionStatus, OVERLAPPED,
    PostQueuedCompletionStatus,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
    PIPE_WAIT,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::core::{HSTRING, PWSTR};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
struct Descriptor(PSECURITY_DESCRIPTOR);
impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe {
            LocalFree(Some(HLOCAL(self.0.0)));
        }
    }
}

fn sid(process: HANDLE) -> Option<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let token = Handle(token);
        let mut needed = 0;
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut needed);
        if needed == 0 || needed > 65536 {
            return None;
        }
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
        GetTokenInformation(token.0, TokenUser, Some(buffer.as_mut_ptr().cast()), needed, &mut needed).ok()?;
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut text).ok()?;
        let value = text.to_string().ok();
        LocalFree(Some(HLOCAL(text.0.cast())));
        value
    }
}

fn peer(pipe: HANDLE, owner: &str) -> bool {
    unsafe {
        let mut pid = 0;
        if GetNamedPipeClientProcessId(pipe, &mut pid).is_err() {
            return false;
        }
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return false };
        let process = Handle(process);
        let mut path = vec![0u16; 32768];
        let mut size = path.len() as u32;
        if QueryFullProcessImageNameW(process.0, PROCESS_NAME_WIN32, PWSTR(path.as_mut_ptr()), &mut size).is_err() {
            return false;
        }
        let path = String::from_utf16_lossy(&path[..size as usize]);
        path.rsplit('\\').next().is_some_and(|name| name.eq_ignore_ascii_case("NightBrowser.exe"))
            && sid(process.0).as_deref() == Some(owner)
    }
}

pub(super) fn signal(port: usize, key: usize) {
    if port != 0 {
        unsafe {
            let _ = PostQueuedCompletionStatus(HANDLE(port as _), 0, key, None);
        }
    }
}

pub(super) fn start(hwnd: HWND) {
    let Ok(port) = (unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, None, 0, 1) }) else { return };
    let raw = port.0 as usize;
    let owner = hwnd.0 as isize;
    if let Ok(mut shared) = SHARED.lock() {
        shared.port = raw;
        shared.thread = Some(std::thread::spawn(move || {
            let port = Handle(HANDLE(raw as _));
            if run(port.0, owner).is_none() {
                update(owner, Snapshot { error: true, ..Default::default() });
            }
            if let Ok(mut shared) = SHARED.lock() {
                if shared.port == raw {
                    shared.port = 0;
                }
            }
        }));
    }
}

fn completion(port: HANDLE, timeout: u32) -> (bool, u32, usize, *mut OVERLAPPED) {
    let mut count = 0;
    let mut key = usize::MAX;
    let mut ptr = std::ptr::null_mut();
    let ok = unsafe { GetQueuedCompletionStatus(port, &mut count, &mut key, &mut ptr, timeout).is_ok() };
    (ok, count, key, ptr)
}

fn pending(result: windows::core::Result<()>) -> bool {
    result.is_ok() || unsafe { GetLastError() == ERROR_IO_PENDING }
}

struct Connection {
    pipe: Handle,
    accept: Box<OVERLAPPED>,
    read: Box<OVERLAPPED>,
    write: Box<OVERLAPPED>,
    accepting: bool,
    reading: bool,
    writing: bool,
    buffer: Box<[u8; 4096]>,
    output: Vec<u8>,
    queue: VecDeque<Vec<u8>>,
}

impl Connection {
    fn read(&mut self) -> bool {
        self.reading = pending(unsafe { ReadFile(self.pipe.0, Some(&mut *self.buffer), None, Some(&mut *self.read)) });
        self.reading
    }
    fn write(&mut self) -> bool {
        if self.writing || self.queue.is_empty() {
            return true;
        }
        self.output = self.queue.pop_front().unwrap();
        self.writing = pending(unsafe { WriteFile(self.pipe.0, Some(&self.output), None, Some(&mut *self.write)) });
        self.writing
    }
    fn send(&mut self, value: serde_json::Value) -> bool {
        if self.queue.len() >= 16 {
            return false;
        }
        let mut bytes = value.to_string().into_bytes();
        bytes.push(b'\n');
        self.queue.push_back(bytes);
        self.write()
    }
    fn finish(&mut self, port: HANDLE) -> bool {
        unsafe {
            let _ = CancelIoEx(self.pipe.0, None);
            let mut count = 0;
            for (active, ptr) in
                [(self.accepting, &*self.accept), (self.reading, &*self.read), (self.writing, &*self.write)]
            {
                if active {
                    let _ = GetOverlappedResult(self.pipe.0, ptr, &mut count, true);
                }
            }
        }
        let mut quit = false;
        loop {
            let (ok, _, key, ptr) = completion(port, 0);
            if key == 0 {
                quit = true;
            }
            if !ok && ptr.is_null() {
                break;
            }
        }
        quit
    }
}

fn run(port: HANDLE, hwnd: isize) -> Option<()> {
    let owner = sid(unsafe { GetCurrentProcess() })?;
    let name = HSTRING::from(format!("\\\\.\\pipe\\NightIsland.Browser.v1.{owner}"));
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            &HSTRING::from(format!("D:P(A;;GA;;;{owner})")),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
        .ok()?;
    }
    let descriptor = Descriptor(descriptor);
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0.0,
        bInheritHandle: false.into(),
    };
    loop {
        let pipe = unsafe {
            CreateNamedPipeW(
                &name,
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                4096,
                32768,
                0,
                Some(&attributes),
            )
        };
        if pipe == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut connection = Connection {
            pipe: Handle(pipe),
            accept: Box::default(),
            read: Box::default(),
            write: Box::default(),
            accepting: false,
            reading: false,
            writing: false,
            buffer: Box::new([0; 4096]),
            output: Vec::new(),
            queue: VecDeque::new(),
        };
        unsafe {
            CreateIoCompletionPort(pipe, Some(port), 2, 1).ok()?;
        }
        let accepted = unsafe { ConnectNamedPipe(pipe, Some(&mut *connection.accept)) };
        let ready = accepted.is_err() && unsafe { GetLastError() == ERROR_PIPE_CONNECTED };
        connection.accepting = !ready && pending(accepted);
        if !ready && !connection.accepting {
            return None;
        }
        let mut quit = false;
        let mut connected = ready;
        if connection.accepting {
            loop {
                let (ok, _, key, ptr) = completion(port, u32::MAX);
                if key == 0 {
                    quit = true;
                    break;
                }
                if ptr == &mut *connection.accept {
                    connection.accepting = false;
                    connected = ok;
                    break;
                }
                if !ok {
                    break;
                }
            }
        }
        if !quit && connected && peer(pipe, &owner) {
            quit = session(&mut connection, port, hwnd);
        }
        quit |= connection.finish(port);
        update(hwnd, Snapshot::default());
        if let Ok(mut shared) = SHARED.lock() {
            shared.commands.clear();
        }
        if quit {
            return Some(());
        }
    }
}

fn session(c: &mut Connection, port: HANDLE, hwnd: isize) -> bool {
    if !c.read() {
        return false;
    }
    let mut hello = false;
    let mut input = Vec::new();
    let mut requests = HashSet::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let timeout = if hello {
            u32::MAX
        } else {
            deadline.saturating_duration_since(std::time::Instant::now()).as_millis().min(5000) as u32
        };
        let (ok, count, key, ptr) = completion(port, timeout);
        if key == 0 {
            return true;
        }
        if !ok {
            return false;
        }
        if key == 1 {
            let commands = SHARED.lock().map(|mut shared| std::mem::take(&mut shared.commands)).unwrap_or_default();
            for Command { id, action, request } in commands {
                if !hello || requests.len() >= 16 {
                    continue;
                }
                requests.insert(request.clone());
                if !c.send(serde_json::json!({"v":1,"type":"command","id":id,"action":action,"request":request})) {
                    return false;
                }
            }
        } else if ptr == &mut *c.read {
            c.reading = false;
            if count == 0 {
                return false;
            }
            input.extend_from_slice(&c.buffer[..count as usize]);
            if input.len() >= MAX_FRAME {
                return false;
            }
            while let Some(end) = input.iter().position(|b| *b == b'\n') {
                let frame = parse(&input[..end]);
                input.drain(..=end);
                match frame {
                    Some(Frame::Hello { .. }) if !hello => {
                        hello = true;
                        if !c.send(serde_json::json!({"v":1,"type":"hello","capabilities":["downloads"]})) {
                            return false;
                        }
                        update(hwnd, Snapshot { connected: true, ..Default::default() });
                    }
                    Some(Frame::Downloads { items, .. }) if hello => {
                        update(hwnd, Snapshot { connected: true, items, error: false })
                    }
                    Some(Frame::Result { request, ok, .. }) if hello => {
                        if requests.remove(&request) && !ok {
                            let mut state = super::snapshot();
                            state.error = true;
                            update(hwnd, state);
                        }
                    }
                    _ => return false,
                }
            }
            if !c.read() {
                return false;
            }
        } else if ptr == &mut *c.write {
            c.writing = false;
            if count as usize != c.output.len() || !c.write() {
                return false;
            }
        }
    }
}
