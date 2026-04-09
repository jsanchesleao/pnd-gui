//! Key handlers for Browse-mode overlay dialogs.

use crossterm::event::KeyCode;

use crate::App;
use crate::pages::vault::state::Phase;

// ── NewFolder ────────────────────────────────────────────────────────────────

pub(super) fn handle_new_folder(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => { app.vault.phase = Phase::Browse; }
        KeyCode::Enter => app.vault.confirm_new_folder(),
        KeyCode::Char(c) => {
            if let Phase::NewFolder { input, error, .. } = &mut app.vault.phase {
                input.push(c);
                *error = None;
            }
        }
        KeyCode::Backspace => {
            if let Phase::NewFolder { input, error, .. } = &mut app.vault.phase {
                input.pop();
                *error = None;
            }
        }
        _ => {}
    }
}

// ── Rename ────────────────────────────────────────────────────────────────────

pub(super) fn handle_rename(app: &mut App, code: KeyCode) {
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

// ── ConfirmDelete ─────────────────────────────────────────────────────────────

pub(super) fn handle_confirm_delete(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('y') | KeyCode::Enter => app.vault.confirm_delete(),
        KeyCode::Char('n') | KeyCode::Esc   => { app.vault.phase = Phase::Browse; }
        _ => {}
    }
}

// ── Move ──────────────────────────────────────────────────────────────────────

pub(super) fn handle_move(app: &mut App, code: KeyCode) {
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
