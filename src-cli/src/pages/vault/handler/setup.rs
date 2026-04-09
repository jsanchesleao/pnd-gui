//! Key handlers for the pre-browse vault forms: menu, unlock, create-new.

use crossterm::event::KeyCode;

use crate::{App, Screen};
use crate::file_browser::FileBrowserTarget;
use crate::yazi::yazi_available;
use crate::pages::vault::state::Phase;

// ── VaultMenu ────────────────────────────────────────────────────────────────

pub(super) fn handle_vault_menu(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => { app.screen = Screen::Menu; }
        KeyCode::Up | KeyCode::Char('k') => app.vault.menu_up(),
        KeyCode::Down | KeyCode::Char('j') => app.vault.menu_down(),
        KeyCode::Enter | KeyCode::Char('l') => app.vault.menu_select(),
        _ => {}
    }
}

// ── Locked ────────────────────────────────────────────────────────────────────

pub(super) fn handle_locked(app: &mut App, code: KeyCode) {
    // ── Path field (focus = 0) ─────────────────────────────────────────────
    if locked_focus(app) == 0 {
        if locked_path_edit_mode(app) {
            // Edit mode: accept text input; Enter/Esc/Tab exit edit mode.
            match code {
                KeyCode::Enter | KeyCode::Esc => {
                    if let Phase::Locked { path_edit_mode, .. } = &mut app.vault.phase {
                        *path_edit_mode = false;
                    }
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    if let Phase::Locked { path_edit_mode, .. } = &mut app.vault.phase {
                        *path_edit_mode = false;
                    }
                    app.vault.advance_focus();
                }
                KeyCode::Char(c) => {
                    if let Phase::Locked { vault_path, error, .. } = &mut app.vault.phase {
                        vault_path.push(c);
                        *error = None;
                    }
                }
                KeyCode::Backspace => {
                    if let Phase::Locked { vault_path, error, .. } = &mut app.vault.phase {
                        vault_path.pop();
                        *error = None;
                    }
                }
                _ => {}
            }
        } else {
            // Display mode: Esc goes back, picker shortcuts, Tab advances.
            match code {
                KeyCode::Esc => { app.vault.phase = Phase::VaultMenu { cursor: 0 }; }
                KeyCode::Char('t') => {
                    if let Phase::Locked { path_edit_mode, .. } = &mut app.vault.phase {
                        *path_edit_mode = true;
                    }
                }
                KeyCode::Char('b') => {
                    let hint = locked_path(app).to_string();
                    app.open_builtin_browser_dir(&hint, FileBrowserTarget::VaultDir);
                }
                KeyCode::Char('y') if yazi_available() => {
                    let hint = locked_path(app).to_string();
                    app.open_yazi_picker(&hint, FileBrowserTarget::VaultDir);
                }
                KeyCode::Enter => {
                    let hint = locked_path(app).to_string();
                    app.open_file_browser_dir(&hint, FileBrowserTarget::VaultDir);
                }
                KeyCode::Tab | KeyCode::BackTab => { app.vault.advance_focus(); }
                _ => {}
            }
        }
        return;
    }

    // ── Password field (focus = 1) ─────────────────────────────────────────
    match code {
        KeyCode::Esc => { app.vault.phase = Phase::VaultMenu { cursor: 0 }; }
        KeyCode::Tab | KeyCode::BackTab => app.vault.advance_focus(),
        KeyCode::Enter => app.vault.start_unlock(),
        KeyCode::Char(c) => {
            if let Phase::Locked { password, .. } = &mut app.vault.phase {
                password.push(c);
            }
        }
        KeyCode::Backspace => {
            if let Phase::Locked { password, .. } = &mut app.vault.phase {
                password.pop();
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

fn locked_path_edit_mode(app: &App) -> bool {
    match &app.vault.phase {
        Phase::Locked { path_edit_mode, .. } => *path_edit_mode,
        _ => false,
    }
}

// ── ConfirmCreateDir ─────────────────────────────────────────────────────────

pub(super) fn handle_confirm_create_dir(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('y') | KeyCode::Enter => app.vault.confirm_create_dir(),
        KeyCode::Char('n') | KeyCode::Esc   => app.vault.cancel_create_dir(),
        _ => {}
    }
}

// ── Creating ──────────────────────────────────────────────────────────────────

pub(super) fn handle_creating(app: &mut App, code: KeyCode) {
    let focus = creating_focus(app);
    let path_edit_mode = creating_path_edit_mode(app);

    // Esc: in edit mode exit edit mode; otherwise go back to menu.
    if code == KeyCode::Esc {
        if focus == 0 && path_edit_mode {
            if let Phase::Creating { path_edit_mode, .. } = &mut app.vault.phase {
                *path_edit_mode = false;
            }
        } else {
            app.vault.phase = Phase::VaultMenu { cursor: 1 };
        }
        return;
    }

    // Focus 0, display mode — t/b/y/Enter shortcuts
    if focus == 0 && !path_edit_mode {
        match code {
            KeyCode::Char('t') => {
                if let Phase::Creating { path_edit_mode, .. } = &mut app.vault.phase {
                    *path_edit_mode = true;
                }
                return;
            }
            KeyCode::Char('b') => {
                let hint = creating_path(app).to_string();
                app.open_builtin_browser_dir(&hint, FileBrowserTarget::VaultCreateDir);
                return;
            }
            KeyCode::Char('y') if yazi_available() => {
                let hint = creating_path(app).to_string();
                app.open_yazi_picker(&hint, FileBrowserTarget::VaultCreateDir);
                return;
            }
            KeyCode::Enter => {
                let hint = creating_path(app).to_string();
                app.open_file_browser_dir(&hint, FileBrowserTarget::VaultCreateDir);
                return;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                app.vault.advance_create_focus();
                return;
            }
            _ => return,
        }
    }

    // Focus 0, edit mode — direct typing
    if focus == 0 && path_edit_mode {
        match code {
            KeyCode::Char(c) => {
                if let Phase::Creating { vault_path, error, .. } = &mut app.vault.phase {
                    vault_path.push(c);
                    *error = None;
                }
            }
            KeyCode::Backspace => {
                if let Phase::Creating { vault_path, error, .. } = &mut app.vault.phase {
                    vault_path.pop();
                    *error = None;
                }
            }
            KeyCode::Enter | KeyCode::Tab | KeyCode::BackTab => {
                if let Phase::Creating { path_edit_mode, .. } = &mut app.vault.phase {
                    *path_edit_mode = false;
                }
            }
            _ => {}
        }
        return;
    }

    // Focus 1 and 2
    match code {
        KeyCode::Tab | KeyCode::BackTab => app.vault.advance_create_focus(),
        KeyCode::Enter if focus == 1 => app.vault.advance_create_focus(),
        KeyCode::Enter => app.vault.start_create(),
        KeyCode::Char(c) => {
            if let Phase::Creating { blobs_dir, password, focus, .. } = &mut app.vault.phase {
                match *focus {
                    1 => { blobs_dir.push(c); }
                    _ => { password.push(c); }
                }
            }
        }
        KeyCode::Backspace => {
            if let Phase::Creating { blobs_dir, password, focus, .. } = &mut app.vault.phase {
                match *focus {
                    1 => { blobs_dir.pop(); }
                    _ => { password.pop(); }
                }
            }
        }
        _ => {}
    }
}

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

fn creating_path_edit_mode(app: &App) -> bool {
    match &app.vault.phase {
        Phase::Creating { path_edit_mode, .. } => *path_edit_mode,
        _ => false,
    }
}
