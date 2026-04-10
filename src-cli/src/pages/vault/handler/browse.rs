//! Key handlers for the Browse phase (tree panel and list panel).

use crossterm::event::KeyCode;

use crate::App;
use crate::file_browser::FileBrowserTarget;
use crate::pages::vault::state::PanelFocus;

pub(super) fn handle_browse(app: &mut App, code: KeyCode) {
    let panel = app.vault.browse.as_ref().map(|b| b.panel_focus).unwrap_or(PanelFocus::List);

    match panel {
        PanelFocus::Tree => handle_browse_tree(app, code),
        PanelFocus::List => handle_browse_list(app, code),
    }
}

fn handle_browse_tree(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Tab | KeyCode::BackTab => {
            if let Some(b) = &mut app.vault.browse { b.panel_focus = PanelFocus::List; }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(b) = &mut app.vault.browse { b.move_tree_up(); }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(b) = &mut app.vault.browse { b.move_tree_down(); }
        }
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
            if let Some(b) = &mut app.vault.browse {
                b.navigate_tree_cursor();
                b.panel_focus = PanelFocus::List;
            }
        }
        KeyCode::Char('h') | KeyCode::Backspace | KeyCode::Left => {
            navigate_up_or_lock(app);
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            navigate_up_or_lock(app);
        }
        KeyCode::Char('g') => app.vault.start_gallery_for_tree_cursor(),
        KeyCode::Char('G') => app.vault.start_gallery_for_current_path(),
        KeyCode::Char('s') => app.vault.save(),
        _ => {}
    }
}

fn handle_browse_list(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Tab | KeyCode::BackTab => {
            if let Some(b) = &mut app.vault.browse { b.panel_focus = PanelFocus::Tree; }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(b) = &mut app.vault.browse { b.move_list_up(); }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(b) = &mut app.vault.browse { b.move_list_down(); }
        }
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
            let folder = app.vault.browse.as_ref()
                .and_then(|b| b.cursor_folder().map(str::to_string));
            if let Some(name) = folder {
                if let Some(b) = &mut app.vault.browse { b.navigate_into(&name); }
            } else {
                // Cursor is on a file — decrypt and preview it
                let uuid = app.vault.browse.as_ref()
                    .and_then(|b| b.cursor_file_uuid().map(str::to_string));
                if let Some(uuid) = uuid {
                    app.vault.start_preview(&uuid);
                }
            }
        }
        KeyCode::Char('h') | KeyCode::Char('q') | KeyCode::Backspace | KeyCode::Left | KeyCode::Esc => {
            navigate_up_or_lock(app);
        }
        KeyCode::Char(' ') => {
            if let Some(b) = &mut app.vault.browse { b.toggle_selection(); }
        }
        KeyCode::Char('a') => {
            // Select all files in current path
            if let Some(b) = &mut app.vault.browse {
                let all: Vec<String> = b.file_uuids.clone();
                for uuid in all { b.selected_uuids.insert(uuid); }
            }
        }
        KeyCode::Char('A') => {
            if let Some(b) = &mut app.vault.browse { b.selected_uuids.clear(); }
        }
        KeyCode::Char('i') => {
            // Open multi-select file browser to add files to the vault
            let start = std::env::current_dir().ok();
            app.open_file_browser_multi(start.as_deref(), FileBrowserTarget::VaultAddFiles);
        }
        KeyCode::Char('e') => {
            // Export (decrypt to disk) the effective selection
            let uuids = app.vault.browse.as_ref()
                .map(|b| b.effective_selection())
                .unwrap_or_default();
            if !uuids.is_empty() {
                app.vault.pending_export_uuids = uuids;
                app.open_file_browser_dir("", FileBrowserTarget::VaultExportDir);
            }
        }
        KeyCode::Char('g') => {
            // Gallery: show all images recursively under the highlighted folder.
            let folder_full_path = app.vault.browse.as_ref().and_then(|b| {
                b.cursor_folder().map(|name| {
                    if b.current_path.is_empty() {
                        name.to_string()
                    } else {
                        format!("{}/{}", b.current_path, name)
                    }
                })
            });
            if let Some(path) = folder_full_path {
                app.vault.start_folder_gallery(&path);
            }
        }
        KeyCode::Char('o') => {
            if let Some(b) = &mut app.vault.browse {
                b.cycle_sort_key();
                b.refresh();
            }
        }
        KeyCode::Char('O') => {
            if let Some(b) = &mut app.vault.browse {
                b.toggle_sort_dir();
                b.refresh();
            }
        }
        KeyCode::Char('G') => app.vault.start_gallery_for_current_path(),
        KeyCode::Char('n') => app.vault.enter_new_folder(),
        KeyCode::Char('r') => app.vault.enter_rename(),
        KeyCode::Char('d') => app.vault.enter_delete(),
        KeyCode::Char('x') => app.vault.cut_selection(),
        KeyCode::Char('p') => app.vault.paste(),
        KeyCode::Char('m') => app.vault.enter_move(),
        KeyCode::Char('s') => app.vault.save(),
        _ => {}
    }
}

/// Go up one folder level, or lock the vault if already at root.
pub(super) fn navigate_up_or_lock(app: &mut App) {
    let at_root = app.vault.browse.as_ref()
        .map(|b| b.current_path.is_empty())
        .unwrap_or(true);

    if at_root {
        if app.direct_vault_launch {
            app.quit = true;
        } else {
            app.vault.lock();
        }
    } else if let Some(b) = &mut app.vault.browse {
        b.navigate_up();
    }
}
