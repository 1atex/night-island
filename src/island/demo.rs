#[cfg(feature = "demo")]
use crate::island::*;

pub(in crate::island) fn active(name: &str) -> bool {
    cfg!(feature = "demo") && std::env::var_os(name).is_some()
}

#[cfg(feature = "demo")]
pub(in crate::island) fn apply(hwnd: HWND) {
    let _ = hwnd;
    if let Ok(spec) = std::env::var("NI_DEBUG_MUSIC") {
        let parts: Vec<&str> = spec.split('|').collect();
        let part = |i: usize| parts.get(i).copied().unwrap_or("").trim().to_string();
        with_app(|a| {
            a.ui.media.playing = true;
            a.ui.media.title = if part(0).is_empty() { "Test track".into() } else { part(0) };
            a.ui.media.artist = part(1);
            a.ui.media.source = part(2);
            if let Ok(file) = std::fs::File::open(part(3)) {
                let decoder = png::Decoder::new(std::io::BufReader::new(file));
                if let Ok(mut reader) = decoder.read_info() {
                    let mut buf = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
                    if let Ok(info) = reader.next_frame(&mut buf)
                        && info.color_type == png::ColorType::Rgba
                        && info.width == info.height
                    {
                        a.ui.media.cover = media::debug_cover(info.width, &buf);
                        if let Some(c) = &a.ui.media.cover {
                            a.ui.media.accent = Some(media::dominant(&c.bgra));
                        }
                    }
                }
            }
            if let (Ok(pos), Ok(len)) = (part(4).parse::<f32>(), part(5).parse::<f32>()) {
                media::debug_timeline(pos, len);
            }
            a.apply_accent();
            a.update_eq();
            a.repaint_collapsed();
            a.repaint_expanded();
        });
    }
    if let Ok(text) = std::env::var("NI_DEBUG_NOTICE") {
        with_app(|a| a.show_notice_for('\u{E722}', &text, 600_000));
    }
    if let Some(secs) = std::env::var("NI_DEBUG_REC").ok().and_then(|v| v.parse::<u32>().ok()) {
        with_app(|a| {
            a.ui.recording_secs = Some(secs);
            a.update_eq();
            a.repaint_collapsed();
        });
    }
    if let Ok(name) = std::env::var("NI_DEBUG_DL") {
        with_app(|a| {
            a.ui.downloading = Some(name);
            a.update_eq();
            a.repaint_collapsed();
        });
    }
    if let Ok(text) = std::env::var("NI_DEBUG_TEXT") {
        with_app(|a| {
            a.ui.text.input.set_text(&text);
            a.repaint_expanded();
        });
    }
    if let Some(n) = std::env::var("NI_DEBUG_TAB").ok().and_then(|v| v.parse::<usize>().ok()) {
        with_app(|a| {
            a.select_tab(view::TABS[n.min(view::TABS.len() - 1)].0);
            a.ui.pinned = true;
            a.expand();
            if let (Ok(list), Some(store)) = (std::env::var("NI_DEBUG_DOCK"), &a.store) {
                for path in list.split(';').filter(|p| !p.is_empty()) {
                    store.dock_add(path);
                }
                a.reload_dock();
                a.repaint_expanded();
            }
            if std::env::var_os("NI_DEBUG_OVERLAY").is_some() {
                let member = |name: &str, muted: bool, speaking: bool| discord::Member {
                    name: name.into(),
                    muted,
                    deaf: false,
                    speaking,
                };
                a.ui.discord.status = discord::Status::Connected;
                a.ui.discord.channel = "General".into();
                a.ui.discord.members =
                    vec![member("Anna", false, true), member("Bob", true, false), member("Zero", false, false)];
                a.settings.discord_overlay = true;
                a.sync_overlay();
            }
            if let Ok(q) = std::env::var("NI_DEBUG_CITY") {
                a.ui.editing = true;
                a.ui.city.input.set_text(&q);
                weather::search(a.hwnd, &q);
            }
            if std::env::var_os("NI_DEBUG_NOTES").is_some()
                && let Some(store) = &a.store
            {
                let work = store.note_add(None, true, "Work");
                let plan = store.note_add(work, false, "Sprint plan");
                store.note_add(work, false, "Meeting notes");
                let old = store.note_add(work, true, "Archive");
                store.note_add(old, false, "2025 goals");
                store.note_add(None, true, "Ideas");
                store.note_add(None, false, "Shopping list");
                if let Some(id) = plan {
                    store.note_set_text(
                        id,
                        "Sprint 14
- settings window
- notes with folders
- reminders picker",
                    );
                }
                a.reload_notes();
                if let Some(id) = plan {
                    a.open_note(id);
                }
                a.ui.notes.expanded.extend(a.ui.notes.items.iter().filter(|i| i.folder).map(|i| i.id));
                a.repaint_expanded();
            }
            if let Ok(group) = std::env::var("NI_DEBUG_SWIN") {
                a.open_settings_window();
                a.ui.sw.group = group.parse().unwrap_or(0);
                if let Ok(q) = std::env::var("NI_DEBUG_SWIN_Q") {
                    a.ui.sw.search = q;
                }
                a.render_settings_win();
            }
            if let Ok(needle) = std::env::var("NI_DEBUG_AGY")
                && let Some(chat) = a.ui.agy.chats.iter().find(|c| c.title.contains(&needle)).cloned()
            {
                a.ui.agy.sel = Some(chat.id);
                a.ui.agy.open = true;
                a.load_history(false);
                if let Ok(text) = std::env::var("NI_DEBUG_AGY_TEXT") {
                    a.ui.agy.input.set_text(&text.replace("\\n", "\n"));
                }
                a.repaint_expanded();
            }
            if let Ok(q) = std::env::var("NI_DEBUG_CMD") {
                a.open_command();
                a.ui.cmd.input.set_text(&q);
                a.refresh_cmd();
                a.repaint_expanded();
            }
            if std::env::var_os("NI_DEBUG_MENU").is_some() {
                a.open_dev_menu();
            }
        });
    }
}
