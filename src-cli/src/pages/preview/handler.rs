//! Key handler for the Preview page.

use crossterm::event::KeyCode;

use crate::{App, Screen};
use crate::file_browser::FileBrowserTarget;
use crate::yazi::yazi_available;
use super::state::PreviewPhase;

/// Handle a keypress on the Preview page.
///
/// Navigation-away keys are resolved before borrowing `preview`, to avoid
/// overlapping mutable borrows.
pub fn handle_preview(app: &mut App, code: KeyCode) {
    // Block all input while the decryption worker is running.
    if matches!(app.preview.phase, PreviewPhase::Decrypting(_)) {
        return;
    }

    // ── Path field (focus = 0) ─────────────────────────────────────────────
    if app.preview.focus == 0 {
        if app.preview.path_edit_mode {
            // Edit mode: accept text input; Enter/Esc/Tab exit edit mode.
            match code {
                KeyCode::Enter | KeyCode::Esc => {
                    app.preview.path_edit_mode = false;
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    app.preview.path_edit_mode = false;
                    app.preview.advance_focus();
                }
                KeyCode::Char(c) => {
                    app.preview.path.push(c);
                    app.preview.phase = PreviewPhase::Idle;
                }
                KeyCode::Backspace => {
                    app.preview.path.pop();
                    app.preview.phase = PreviewPhase::Idle;
                }
                _ => {}
            }
        } else {
            // Display mode: Esc goes back, picker shortcuts, Tab advances.
            match code {
                KeyCode::Esc => { app.screen = Screen::Menu; }
                KeyCode::Char('t') => { app.preview.path_edit_mode = true; }
                KeyCode::Char('b') => {
                    let hint = app.preview.path.clone();
                    app.open_builtin_browser(&hint, FileBrowserTarget::PreviewPath);
                }
                KeyCode::Char('y') if yazi_available() => {
                    let hint = app.preview.path.clone();
                    app.open_yazi_picker(&hint, FileBrowserTarget::PreviewPath);
                }
                KeyCode::Enter => {
                    let hint = app.preview.path.clone();
                    app.open_file_browser(&hint, FileBrowserTarget::PreviewPath);
                }
                KeyCode::Tab | KeyCode::BackTab => { app.preview.advance_focus(); }
                _ => {}
            }
        }
        return;
    }

    // ── Password field (focus = 1) ─────────────────────────────────────────
    match code {
        KeyCode::Esc => { app.screen = Screen::Menu; }
        KeyCode::Tab | KeyCode::BackTab => { app.preview.advance_focus(); }
        KeyCode::Enter => { app.preview.start(); }
        KeyCode::Char(c) => { app.preview.password.push(c); }
        KeyCode::Backspace => { app.preview.password.pop(); }
        _ => {}
    }
}
