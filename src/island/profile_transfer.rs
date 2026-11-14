use std::path::{Path, PathBuf};

use windows::Win32::UI::Controls::Dialogs::{
    CommDlgExtendedError, GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR,
    OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::core::{HSTRING, PCWSTR, PWSTR};

use crate::data::profile::{self, Import, Scope};
use crate::island::*;
use crate::ui::i18n::tr;

pub(super) const MESSAGE: u32 = WM_APP + 113;

pub(super) fn post(hwnd: HWND, scope: Scope, action: isize) {
    unsafe {
        let _ = PostMessageW(Some(hwnd), MESSAGE, WPARAM(scope as usize), LPARAM(action));
    }
}

fn pick(owner: HWND, scope: Scope, save: bool) -> std::result::Result<Option<PathBuf>, String> {
    let mut name = vec![0u16; 32768];
    if save {
        for (to, from) in name.iter_mut().zip(scope.filename().encode_utf16()) {
            *to = from;
        }
    }
    let title = HSTRING::from(tr(if save { "Export settings…" } else { "Import settings…" }).as_ref());
    let mut params = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: w!("Night Island (*.json)\0*.json\0"),
        lpstrTitle: PCWSTR(title.as_ptr()),
        lpstrDefExt: w!("json"),
        lpstrFile: PWSTR(name.as_mut_ptr()),
        nMaxFile: name.len() as u32,
        Flags: OFN_EXPLORER
            | OFN_NOCHANGEDIR
            | OFN_PATHMUSTEXIST
            | if save { OFN_OVERWRITEPROMPT } else { OFN_FILEMUSTEXIST },
        ..Default::default()
    };
    let ok = unsafe { if save { GetSaveFileNameW(&mut params) } else { GetOpenFileNameW(&mut params) } };
    if !ok.as_bool() {
        return if unsafe { CommDlgExtendedError() }.0 == 0 {
            Ok(None)
        } else {
            Err(tr("Could not open the file picker").into_owned())
        };
    }
    use std::os::windows::ffi::OsStringExt;
    let length = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    Ok(Some(std::ffi::OsString::from_wide(&name[..length]).into()))
}

fn protected_destination(path: &Path) -> bool {
    let Some(parent) = Settings::path().and_then(|path| path.parent().map(Path::to_owned)) else { return false };
    let Ok(dir) = parent.canonicalize() else { return false };
    let Some(out) = path.parent().and_then(|p| p.canonicalize().ok()) else { return false };
    dir == out
        && path
            .file_name()
            .is_some_and(|n| ["settings.json", "before-import.json"].iter().any(|s| n.eq_ignore_ascii_case(s)))
}

fn transfer(hwnd: HWND, scope: Scope, action: isize) -> std::result::Result<Option<String>, String> {
    let Some(owner) = with_app_value(|a| a.settings_win.unwrap_or(hwnd)) else { return Ok(None) };
    let save = action == 0;
    let backup = profile::backup_path().map_err(|e| e.to_string())?;
    let path = if action == 2 {
        backup.clone()
    } else {
        let Some(path) = pick(owner, scope, save)? else { return Ok(None) };
        path
    };
    let Some(current) = with_app_value(|a| a.settings.clone()) else { return Ok(None) };
    if save {
        if protected_destination(&path) {
            return Err(tr("Choose another file name").into_owned());
        }
        profile::export(&path, &current, scope).map_err(|e| e.to_string())?;
        return Ok(Some(format!("{} · {}", tr("Settings exported"), path.display())));
    }
    let incoming = Import::read(&path, scope).map_err(|e| tr(e).into_owned())?;
    let message = match scope {
        Scope::Island => "Replace Island settings and restart the app?",
        Scope::Dock => "Replace the dock settings and pinned shortcuts?",
        Scope::Top => "Replace the top panel settings and visible blocks?",
    };
    let mut prompt = format!(
        "{}\n{}\n\n{}",
        tr(message),
        path.file_name().unwrap_or_default().to_string_lossy(),
        tr("You can restore the previous settings in General.")
    );
    let next = incoming.apply(&current);
    if scope == Scope::Island {
        for (label, old, new) in [
            ("AI server", &current.ai.openai_base, &next.ai.openai_base),
            ("Speech server", &current.ai.voice_base, &next.ai.voice_base),
            ("Voice server", &current.assistant.tts_base, &next.assistant.tts_base),
        ] {
            if old != new {
                prompt.push_str(&format!("\n{} → {}", tr(label), new));
            }
        }
    }
    let answer = unsafe {
        MessageBoxW(
            Some(owner),
            &HSTRING::from(prompt),
            w!("Night Island"),
            MB_OKCANCEL | MB_ICONQUESTION | MB_DEFBUTTON2,
        )
    };
    if answer != IDOK {
        return Ok(None);
    }
    if scope == Scope::Island && with_app_value(|a| a.recording.is_some()).unwrap_or(true) {
        return Err(tr("Finish the screen recording before restoring the full profile").into_owned());
    }
    let Some(current) = with_app_value(|a| a.settings.clone()) else { return Ok(None) };
    let next = match scope {
        Scope::Dock => Settings { desktop_dock: next.desktop_dock, ..current.clone() },
        Scope::Top => Settings { top_panel: next.top_panel, ..current.clone() },
        Scope::Island => next,
    };
    std::fs::create_dir_all(backup.parent().ok_or_else(|| "Profile folder unavailable".to_owned())?)
        .map_err(|e| e.to_string())?;
    profile::export(&backup, &current, Scope::Island).map_err(|e| e.to_string())?;
    next.save_checked().map_err(|e| e.to_string())?;
    with_app(|app| {
        app.settings = next;
        app.ui.cfg = app.settings.clone();
        match scope {
            Scope::Dock => app.configure_desktop_dock(),
            Scope::Top => app.configure_top_panel(),
            Scope::Island => {}
        }
    });
    if scope == Scope::Island {
        if std::env::current_exe()
            .and_then(|exe| std::process::Command::new(exe).arg("--restore-settings").spawn())
            .is_err()
        {
            return Err(tr("Settings saved. Restart Island to apply them.").into_owned());
        }
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        return Ok(None);
    }
    Ok(Some(tr("Settings imported").into_owned()))
}

pub(super) fn handle(hwnd: HWND, id: usize, action: isize) {
    let Some(scope) = [Scope::Island, Scope::Dock, Scope::Top].get(id).copied() else { return };
    let result = match transfer(hwnd, scope, action) {
        Ok(None) => return,
        Ok(Some(message)) => Ok(message),
        Err(error) => Err(error),
    };
    let ok = result.is_ok();
    with_app(|app| {
        app.ui.sw.transfer_status = Some((scope, result.unwrap_or_else(|e| e), ok));
        app.ui.sw.has_settings_backup = profile::backup_path().is_ok_and(|path| path.is_file());
        app.render_settings_win();
    });
}
