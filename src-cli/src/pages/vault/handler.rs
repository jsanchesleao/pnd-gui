//! Key handler for the Vault page.

use crossterm::event::KeyCode;

use crate::{App, Screen};
use crate::file_browser::FileBrowserTarget;
use super::state::{PanelFocus, Phase};

pub(crate) fn handle_vault(app: &mut App, code: KeyCode) {
    // Block all input while the vault is being opened, created, or adding files
    if app.vault.is_opening() || app.vault.is_adding() { return; }

    match &app.vault.phase {
        Phase::VaultMenu { .. }       => handle_vault_menu(app, code),
        Phase::Locked { .. }          => handle_locked(app, code),
        Phase::Creating { .. }        => handle_creating(app, code),
        Phase::ConfirmCreateDir { .. } => handle_confirm_create_dir(app, code),
        Phase::Browse                 => handle_browse(app, code),
        Phase::Rename { .. }          => handle_rename(app, code),
        Phase::ConfirmDelete { .. }   => handle_confirm_delete(app, code),
        Phase::Move { .. }            => handle_move(app, code),
        Phase::Opening(_)             => {} // blocked above
        Phase::Adding { .. }          => {} // blocked above
    }
}

// ── VaultMenu ───────────────────────────────────────────────────────────────

fn handle_vault_menu(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Char('h') => { app.screen = Screen::Menu; }
        KeyCode::Up | KeyCode::Char('k') => app.vault.menu_up(),
        KeyCode::Down | KeyCode::Char('j') => app.vault.menu_down(),
        KeyCode::Enter | KeyCode::Char('l') => app.vault.menu_select(),
        _ => {}
    }
}

// ── Locked ─────────────────────────────────────────────────────────────────

fn handle_locked(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => { app.vault.phase = Phase::VaultMenu { cursor: 0 }; return; }
        KeyCode::Enter if locked_focus(app) == 0 => {
            let hint = locked_path(app).to_string();
            app.open_file_browser_dir(&hint, FileBrowserTarget::VaultDir);
            return;
        }
        _ => {}
    }

    match code {
        KeyCode::Tab | KeyCode::BackTab => app.vault.advance_focus(),
        KeyCode::Enter => app.vault.start_unlock(),
        KeyCode::Char(c) => {
            if let Phase::Locked { vault_path, password, focus, error, .. } = &mut app.vault.phase {
                if *focus == 0 { vault_path.push(c); *error = None; }
                else           { password.push(c); }
            }
        }
        KeyCode::Backspace => {
            if let Phase::Locked { vault_path, password, focus, error, .. } = &mut app.vault.phase {
                if *focus == 0 { vault_path.pop(); *error = None; }
                else           { password.pop(); }
            }
        }
        _ => {}
    }
}

// ── ConfirmCreateDir overlay ────────────────────────────────────────────────

fn handle_confirm_create_dir(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('y') | KeyCode::Enter => app.vault.confirm_create_dir(),
        KeyCode::Char('n') | KeyCode::Esc   => app.vault.cancel_create_dir(),
        _ => {}
    }
}

// ── Creating ────────────────────────────────────────────────────────────────

fn creating_focus(app: &App) -> usize {
    match &app.vault.phase {
        Phase::Creating { focus, .. } => *focus,
        _ => 0,
    }
}

fn creating_path(app: &App) -> &str {
    match &app.vault.phase {
        Phase::Creating { vault_path, .. } => vault_path.as_str(),
        _ => "",
    }
}

fn handle_creating(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.vault.phase = Phase::VaultMenu { cursor: 1 };
            return;
        }
        KeyCode::Enter if creating_focus(app) == 0 => {
            let hint = creating_path(app).to_string();
            app.open_file_browser_dir(&hint, FileBrowserTarget::VaultCreateDir);
            return;
        }
        KeyCode::Enter if creating_focus(app) == 1 => {
            app.vault.advance_create_focus();
            return;
        }
        _ => {}
    }

    match code {
        KeyCode::Tab | KeyCode::BackTab => app.vault.advance_create_focus(),
        KeyCode::Enter => app.vault.start_create(),
        KeyCode::Char(c) => {
            if let Phase::Creating { vault_path, blobs_dir, password, focus, error, .. } = &mut app.vault.phase {
                match *focus {
                    0 => { vault_path.push(c); *error = None; }
                    1 => { blobs_dir.push(c); }
                    _ => { password.push(c); }
                }
            }
        }
        KeyCode::Backspace => {
            if let Phase::Creating { vault_path, blobs_dir, password, focus, error, .. } = &mut app.vault.phase {
                match *focus {
                    0 => { vault_path.pop(); *error = None; }
                    1 => { blobs_dir.pop(); }
                    _ => { password.pop(); }
                }
            }
        }
        _ => {}
    }
}

fn locked_focus(app: &App) -> usize {
    match &app.vault.phase {
        Phase::Locked { focus, .. } => *focus,
        _ => 0,
    }
}

fn locked_path(app: &App) -> &str {
    match &app.vault.phase {
        Phase::Locked { vault_path, .. } => vault_path.as_str(),
        _ => "",
    }
}

// ── Browse ─────────────────────────────────────────────────────────────────

fn handle_browse(app: &mut App, code: KeyCode) {
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
        KeyCode::Esc => {
            navigate_up_or_lock(app);
        }
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
            }
            // Future: file preview hook goes here
        }
        KeyCode::Char('h') | KeyCode::Backspace | KeyCode::Left | KeyCode::Esc => {
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
        KeyCode::Char('r') => app.vault.enter_rename(),
        KeyCode::Char('d') => app.vault.enter_delete(),
        KeyCode::Char('x') => app.vault.cut_selection(),
        KeyCode::Char('p') => app.vault.paste(),
        KeyCode::Char('m') => app.vault.enter_move(),
        KeyCode::Char('s') => app.vault.save(),
        _ => {}
    }
}

/// Go up one folder, or if at root, lock the vault.
fn navigate_up_or_lock(app: &mut App) {
    let at_root = app.vault.browse.as_ref()
        .map(|b| b.current_path.is_empty())
        .unwrap_or(true);

    if at_root {
        app.vault.lock();
    } else if let Some(b) = &mut app.vault.browse {
        b.navigate_up();
    }
}

// ── Rename overlay ─────────────────────────────────────────────────────────

fn handle_rename(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => { app.vault.phase = Phase::Browse; }
        KeyCode::Enter => app.vault.confirm_rename(),
        KeyCode::Char(c) => {
            if let Phase::Rename { input, .. } = &mut app.vault.phase { input.push(c); }
        }
        KeyCode::Backspace => {
            if let Phase::Rename { input, .. } = &mut app.vault.phase { input.pop(); }
        }
        _ => {}
    }
}

// ── ConfirmDelete overlay ───────────────────────────────────────────────────

fn handle_confirm_delete(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('y') | KeyCode::Enter => app.vault.confirm_delete(),
        KeyCode::Char('n') | KeyCode::Esc   => { app.vault.phase = Phase::Browse; }
        _ => {}
    }
}

// ── Move overlay ────────────────────────────────────────────────────────────

fn handle_move(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => { app.vault.phase = Phase::Browse; }
        KeyCode::Enter => app.vault.confirm_move(),
        KeyCode::Up | KeyCode::Char('k') => {
            if let Phase::Move { tree_cursor, .. } = &mut app.vault.phase {
                if *tree_cursor > 0 { *tree_cursor -= 1; }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let folder_count = app.vault.browse.as_ref()
                .map(|b| b.all_folders.len())
                .unwrap_or(0);
            if let Phase::Move { tree_cursor, .. } = &mut app.vault.phase {
                if folder_count > 0 && *tree_cursor < folder_count - 1 {
                    *tree_cursor += 1;
                }
            }
        }
        _ => {}
    }
}
