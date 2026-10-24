use crate::island::*;

pub(in crate::island) const WM_CLIPBOARDUPDATE: u32 = 0x031D;
pub(in crate::island) const WM_MOUSELEAVE: u32 = 0x02A3;
pub(in crate::island) const WM_SHOW: u32 = WM_APP + 2;

pub(in crate::island) fn point_from(lparam: LPARAM) -> (i32, i32) {
    (lparam.0 as i16 as i32, (lparam.0 >> 16) as i16 as i32)
}

pub(in crate::island) extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_MOUSEMOVE => {
            let (x, y) = point_from(lparam);
            let left_down = wparam.0 & 0x0001 != 0;
            let mut drag = None;
            with_app(|a| {
                a.on_mouse_move(x, y);
                a.hover_at(x, y);
                if left_down {
                    a.drag_select(x, y);
                    a.drag_read(x, y);
                    a.drag_slider(x);
                    a.drag_volume(x);
                    a.drag_seek(x);
                    a.drag_island();
                }
                drag = a.take_drag(x, y, left_down);
            });
            if let Some(path) = drag {
                dnd::drag_out(&path);
                with_app(|a| a.on_mouse_leave());
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let delta = (wparam.0 >> 16) as i16 as i32;
            with_app(|a| a.wheel(delta));
            LRESULT(0)
        }
        WM_CLIPBOARDUPDATE => {
            with_app(|a| a.set_timer(Timer::Clip, 40));
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            with_app(|a| {
                a.pointer.drag_from = None;
                a.pointer.selecting = false;
                a.pointer.reading = false;
                a.pointer.vol_drag = false;
                a.finish_note_drag();
                a.mouse_up();
                a.finish_seek();
                a.finish_slider();
            });
            LRESULT(0)
        }
        dnd::WM_DND_ENTER => {
            with_app(|a| a.on_dnd_enter());
            LRESULT(0)
        }
        dnd::WM_DND_LEAVE => {
            with_app(|a| a.on_dnd_leave());
            LRESULT(0)
        }
        dnd::WM_DND_DROP => {
            with_app(|a| a.on_dnd_drop());
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = point_from(lparam);
            with_app(|a| a.mouse_down(x, y));
            let mut dialog = false;
            with_app(|a| dialog = std::mem::take(&mut a.want_dock_dialog));
            if dialog {
                with_app(|a| a.popup_open = true);
                let picked = pick_file(hwnd);
                with_app(|a| a.finish_dock_dialog(picked));
            }
            let mut chat_dialog = false;
            with_app(|a| chat_dialog = std::mem::take(&mut a.want_chat_dialog));
            if chat_dialog {
                with_app(|a| a.popup_open = true);
                let picked = pick_file(hwnd);
                with_app(|a| a.finish_chat_dialog(picked));
            }
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            let (x, y) = point_from(lparam);
            let mut target = None;
            with_app(|a| target = a.dock_target_at(x, y));
            if let Some((index, path)) = target {
                with_app(|a| a.popup_open = true);
                let choice = dock_popup(hwnd, std::path::Path::new(&path).exists());
                with_app(|a| {
                    a.popup_open = false;
                    a.apply_dock_choice(choice, index, &path);
                });
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            with_app(|a| a.on_mouse_leave());
            LRESULT(0)
        }
        WM_TIMER => {
            with_app(|a| a.on_timer(wparam.0));
            LRESULT(0)
        }
        WM_HOTKEY => {
            match Hotkey::from_id(wparam.0 as i32) {
                Some(Hotkey::Toggle) => with_app(|a| {
                    a.restore_pill();
                    a.toggle();
                }),
                Some(Hotkey::Clipboard) => with_app(|a| {
                    a.restore_pill();
                    a.open_tab(view::Tab::Clipboard);
                }),
                Some(Hotkey::PlainPaste) => with_app(|a| a.plain_paste()),
                Some(Hotkey::Command) => with_app(|a| {
                    a.restore_pill();
                    a.toggle_command();
                }),
                Some(Hotkey::HidePill) => with_app(|a| a.hide_pill_for_a_while()),
                Some(Hotkey::Mic) => with_app(|a| a.toggle_mic()),
                Some(Hotkey::ShotArea) => with_app(|a| a.capture(capture::Mode::Area)),
                Some(Hotkey::ShotFull) => with_app(|a| capture::fullscreen(a.hwnd)),
                Some(Hotkey::Picker) => with_app(|a| a.capture(capture::Mode::Picker)),
                Some(Hotkey::Ocr) => with_app(|a| a.capture(capture::Mode::Ocr)),
                Some(Hotkey::Qr) => with_app(|a| a.capture(capture::Mode::Qr)),
                Some(Hotkey::Rec) => with_app(|a| a.toggle_recording()),
                Some(Hotkey::Dictate) => with_app(|a| a.toggle_dictation()),
                _ => {}
            }
            LRESULT(0)
        }
        privacy::WM_PRIVACY => {
            with_app(|a| a.on_privacy());
            LRESULT(0)
        }
        downloads::WM_DOWNLOAD => {
            with_app(|a| a.on_downloads());
            LRESULT(0)
        }
        recorder::WM_RECORDED => {
            with_app(|a| a.on_recorded());
            LRESULT(0)
        }
        recognize::WM_RECOGNIZED => {
            with_app(|a| a.on_recognized());
            LRESULT(0)
        }
        capture::WM_CAPTURE_DONE => {
            with_app(|a| a.on_capture_done());
            LRESULT(0)
        }
        weather::WM_GEO => {
            with_app(|a| a.on_geo());
            LRESULT(0)
        }
        agy::WM_AGY => {
            with_app(|a| a.on_agy());
            LRESULT(0)
        }
        overlay::WM_OVERLAY => {
            with_app(|a| a.on_overlay(wparam.0));
            LRESULT(0)
        }
        discord::WM_DISCORD => {
            with_app(|a| a.on_discord());
            LRESULT(0)
        }
        spotify::WM_SPOTIFY => {
            with_app(|a| a.on_spotify());
            LRESULT(0)
        }
        weather::WM_WEATHER => {
            with_app(|a| a.on_weather());
            LRESULT(0)
        }
        lockkeys::WM_LOCKKEY => {
            let vk = wparam.0;
            with_app(|a| a.on_lockkey(vk));
            LRESULT(0)
        }
        WM_POWERBROADCAST => {
            if wparam.0 as u32 == PBT_APMPOWERSTATUSCHANGE {
                with_app(|a| a.on_power());
            }
            LRESULT(1)
        }
        update::WM_UPDATE => {
            with_app(|a| a.on_update());
            LRESULT(0)
        }
        everything::WM_FILES => {
            with_app(|a| a.on_files());
            LRESULT(0)
        }
        steam::WM_STEAM => {
            with_app(|a| a.on_steam());
            LRESULT(0)
        }
        agents::WM_AGENT => {
            with_app(|a| a.on_agent());
            LRESULT(0)
        }
        metrics::WM_METRICS => {
            with_app(|a| a.on_metrics());
            LRESULT(0)
        }
        dictation::WM_VOICE => {
            with_app(|a| a.on_voice());
            LRESULT(0)
        }
        ai::WM_TRANSCRIBED => {
            with_app(|a| a.on_transcribed());
            LRESULT(0)
        }
        ai::WM_CHAT => {
            with_app(|a| a.on_chat());
            LRESULT(0)
        }
        ai::WM_COMMAND => {
            with_app(|a| a.on_voice_command());
            LRESULT(0)
        }
        vpn::WM_VPN => {
            with_app(|a| a.on_vpn());
            LRESULT(0)
        }
        steam_art::WM_STEAM_ART => {
            with_app(|a| a.on_steam_art());
            LRESULT(0)
        }
        steam::WM_STEAM_SWITCH => {
            with_app(|a| a.on_steam_switch());
            LRESULT(0)
        }
        ai::WM_SORT => {
            with_app(|a| a.on_sort());
            LRESULT(0)
        }
        ai::WM_AI => {
            with_app(|a| a.on_ai());
            LRESULT(0)
        }
        lyrics::WM_LYRICS => {
            with_app(|a| a.on_lyrics());
            LRESULT(0)
        }
        media::WM_MEDIA => {
            with_app(|a| a.on_media());
            LRESULT(0)
        }
        system::WM_RADIOS => {
            with_app(|a| a.on_radios(wparam.0));
            LRESULT(0)
        }
        WM_COPYDATA => {
            let cds = unsafe { (lparam.0 as *const windows::Win32::System::DataExchange::COPYDATASTRUCT).as_ref() };
            if let Some(cds) = cds
                && !cds.lpData.is_null()
                && cds.cbData <= 4 * 1024 * 1024
            {
                let bytes = unsafe { std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize) };
                if everything::on_copydata(wparam.0, cds.dwData, bytes) {
                    return LRESULT(1);
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            with_app(|a| a.on_keydown(wparam.0 as u16));
            LRESULT(0)
        }
        WM_CHAR => {
            with_app(|a| a.on_char(wparam.0 as u16));
            LRESULT(0)
        }
        WM_ACTIVATE if (wparam.0 & 0xFFFF) as u32 == WA_INACTIVE => {
            with_app(|a| {
                if demo::active("NI_DEBUG_CMD") {
                    return;
                }
                if a.ui.cmd.open {
                    a.close_command(false);
                } else {
                    a.end_edit(false);
                }
            });
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let (mut ibeam, mut hand) = (false, false);
            with_app(|a| {
                ibeam = a.cursor_over_note();
                hand = a.ui.hand;
            });
            if (ibeam || hand) && (lparam.0 & 0xFFFF) as u32 == HTCLIENT {
                let shape = if ibeam { IDC_IBEAM } else { IDC_HAND };
                unsafe { SetCursor(LoadCursorW(None, shape).ok()) };
                return LRESULT(1);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_SHOW => {
            with_app(|a| a.peek());
            LRESULT(0)
        }
        tray::WM_TRAY => {
            match tray::click(lparam) {
                tray::Click::Primary => with_app(|a| a.toggle()),
                tray::Click::Secondary => match tray::menu(hwnd) {
                    Some(tray::ID_EXIT) => unsafe {
                        let _ = DestroyWindow(hwnd);
                    },
                    Some(id) => with_app(|a| a.apply_menu_choice(id)),
                    None => {}
                },
                tray::Click::None => {}
            }
            LRESULT(0)
        }
        m if m != 0 && m == TASKBAR_CREATED.load(Ordering::Relaxed) => {
            tray::add(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            with_app(|a| a.save_note());
            dnd::unregister(hwnd);
            clips::sys::unlisten(hwnd);
            tray::remove(hwnd);
            std::process::exit(0);
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub(in crate::island) fn dock_popup(hwnd: HWND, exists: bool) -> Option<usize> {
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, DestroyMenu, MF_SEPARATOR, MF_STRING, SetForegroundWindow, TPM_NONOTIFY,
        TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_NULL,
    };
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return None };
        let item = |id: usize, text: &str| {
            let label = windows::core::HSTRING::from(crate::ui::i18n::tr(text).as_ref());
            let _ = AppendMenuW(menu, MF_STRING, id, windows::core::PCWSTR(label.as_ptr()));
        };
        if exists {
            item(1, "Show in folder");
            item(2, "Run as administrator");
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        }
        item(3, "Remove from the dock");
        let mut pt = windows::Win32::Foundation::POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY, pt.x, pt.y, None, hwnd, None);
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        (cmd.0 > 0).then_some(cmd.0 as usize)
    }
}

pub(in crate::island) fn pick_file(owner: HWND) -> Option<String> {
    use windows::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    let mut buffer = [0u16; 1024];
    let filter: Vec<u16> = "All files\0*.*\0\0".encode_utf16().collect();
    let mut ofn = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: windows::core::PCWSTR(filter.as_ptr()),
        lpstrFile: windows::core::PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
        ..Default::default()
    };
    unsafe { GetOpenFileNameW(&mut ofn).as_bool() }.then(|| {
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..len])
    })
}
