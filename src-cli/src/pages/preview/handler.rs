//! Key handler for the Preview page.

use crossterm::event::KeyCode;

use crate::{App, Screen};
use crate::file_browser::FileBrowserTarget;
use super::state::PreviewPhase;

/// Handle a keypress on the Preview page.
///
/// Navigation-away keys (Esc, Enter-on-path) are resolved before borrowing
/// `preview`, to avoid overlapping mutable borrows.
pub fn handle_preview(app: &mut App, code: KeyCode) {
    // Block all input while the decryption worker is running.
    if matches!(app.preview.phase, PreviewPhase::Decrypting(_)) {
        return;
    }

    match code {
        KeyCode::Esc => { app.screen = Screen::Menu; return; }
        // Enter on the path field opens the file browser instead of advancing focus.
        KeyCode::Enter if app.preview.focus == 0 => {
            let hint = app.preview.path.clone();
            app.open_file_browser(&hint, FileBrowserTarget::PreviewPath);
            return;
        }
        _ => {}
    }

    let s = &mut app.preview;
    match code {
        KeyCode::Tab | KeyCode::BackTab => s.advance_focus(),
        // Enter on the password field starts decryption immediately.
        KeyCode::Enter if s.focus == 1 => s.start(),
        KeyCode::Char(c) => {
            if s.focus == 0 {
                s.path.push(c);
                s.phase = PreviewPhase::Idle;
            } else {
                s.password.push(c);
            }
        }
        KeyCode::Backspace => {
            if s.focus == 0 {
                s.path.pop();
                s.phase = PreviewPhase::Idle;
            } else {
                s.password.pop();
            }
        }
        _ => {}
    }
}
