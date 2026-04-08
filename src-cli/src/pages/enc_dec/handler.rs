//! Key handler for the Encrypt/Decrypt page.

use crossterm::event::KeyCode;

use crate::{App, Screen};
use crate::file_browser::FileBrowserTarget;
use super::state::OpStatus;

/// Handle a keypress on the Encrypt/Decrypt page.
///
/// Navigation-away keys (Esc, Enter-on-path) are resolved before borrowing
/// `enc_dec`, to avoid overlapping mutable borrows.
pub fn handle_enc_dec(app: &mut App, code: KeyCode) {
    // Block all input while an operation is running.
    if matches!(app.enc_dec.status, OpStatus::Running(_)) {
        return;
    }

    match code {
        KeyCode::Esc => { app.screen = Screen::Menu; return; }
        // Enter on the path field opens the file browser instead of advancing focus.
        KeyCode::Enter if app.enc_dec.focus == 0 => {
            let hint = app.enc_dec.path.clone();
            app.open_file_browser(&hint, FileBrowserTarget::EncDecPath);
            return;
        }
        _ => {}
    }

    let s = &mut app.enc_dec;
    match code {
        KeyCode::Tab | KeyCode::BackTab => s.advance_focus(),
        // Enter on the password field runs the operation immediately.
        KeyCode::Enter if s.focus == 1 => s.start(),
        KeyCode::Char(c) => {
            if s.focus == 0 {
                s.path.push(c);
                s.status = OpStatus::Idle;
            } else {
                s.password.push(c);
            }
        }
        KeyCode::Backspace => {
            if s.focus == 0 { s.path.pop(); s.status = OpStatus::Idle; }
            else { s.password.pop(); }
        }
        _ => {}
    }
}
