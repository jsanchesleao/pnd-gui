//! Key handler for the Encrypt/Decrypt page.

use crossterm::event::KeyCode;

use crate::{App, Screen};
use crate::file_browser::FileBrowserTarget;
use crate::yazi::yazi_available;
use super::state::OpStatus;

/// Handle a keypress on the Encrypt/Decrypt page.
///
/// Navigation-away keys are resolved before borrowing `enc_dec`, to avoid
/// overlapping mutable borrows.
pub fn handle_enc_dec(app: &mut App, code: KeyCode) {
    // Block all input while an operation is running.
    if matches!(app.enc_dec.status, OpStatus::Running(_)) {
        return;
    }

    // ── Path field (focus = 0) ─────────────────────────────────────────────
    if app.enc_dec.focus == 0 {
        if app.enc_dec.path_edit_mode {
            // Edit mode: accept text input; Enter/Esc/Tab exit edit mode.
            match code {
                KeyCode::Enter | KeyCode::Esc => {
                    app.enc_dec.path_edit_mode = false;
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    app.enc_dec.path_edit_mode = false;
                    app.enc_dec.advance_focus();
                }
                KeyCode::Char(c) => {
                    app.enc_dec.path.push(c);
                    app.enc_dec.status = OpStatus::Idle;
                }
                KeyCode::Backspace => {
                    app.enc_dec.path.pop();
                    app.enc_dec.status = OpStatus::Idle;
                }
                _ => {}
            }
        } else {
            // Display mode: Esc goes back, picker shortcuts, Tab advances.
            match code {
                KeyCode::Esc => { app.screen = Screen::Menu; }
                KeyCode::Char('t') => { app.enc_dec.path_edit_mode = true; }
                KeyCode::Char('b') => {
                    let hint = app.enc_dec.path.clone();
                    app.open_builtin_browser(&hint, FileBrowserTarget::EncDecPath);
                }
                KeyCode::Char('y') if yazi_available() => {
                    let hint = app.enc_dec.path.clone();
                    app.open_yazi_picker(&hint, FileBrowserTarget::EncDecPath);
                }
                KeyCode::Enter => {
                    let hint = app.enc_dec.path.clone();
                    app.open_file_browser(&hint, FileBrowserTarget::EncDecPath);
                }
                KeyCode::Tab | KeyCode::BackTab => { app.enc_dec.advance_focus(); }
                _ => {}
            }
        }
        return;
    }

    // ── Password field (focus = 1) ─────────────────────────────────────────
    match code {
        KeyCode::Esc => { app.screen = Screen::Menu; }
        KeyCode::Tab | KeyCode::BackTab => { app.enc_dec.advance_focus(); }
        KeyCode::Enter => { app.enc_dec.start(); }
        KeyCode::Char(c) => { app.enc_dec.password.push(c); }
        KeyCode::Backspace => { app.enc_dec.password.pop(); }
        _ => {}
    }
}
