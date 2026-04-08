mod draw;
mod handler;
mod image;
mod media;
mod state;

pub(crate) use state::{PreviewPhase, PreviewResult, PreviewState};
pub use draw::draw_preview;
pub use handler::handle_preview;

use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, mem};

/// Called from the main event loop before `terminal.draw` whenever the phase
/// is `PendingRender`. Pulls the decrypted bytes out, classifies the file type,
/// and dispatches to the appropriate renderer.
pub(crate) fn render_preview(
    state: &mut PreviewState,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) {
    let (bytes, ext) = match mem::replace(&mut state.phase, PreviewPhase::Idle) {
        PreviewPhase::PendingRender { bytes, ext } => (bytes, ext),
        other => { state.phase = other; return; }
    };

    let result = if image::is_image_ext(&ext) {
        if image::supports_kitty() {
            // Decode at the terminal's actual pixel resolution so the image never
            // overflows the screen and the RGBA buffer stays as small as possible.
            let (max_w, max_h) = image::terminal_pixel_size();
            match image::decode_rgba(&bytes, &ext, max_w, max_h) {
                Err(e) => PreviewResult::RenderFailed(e),
                Ok((rgba, w, h)) => match image::render_kitty(terminal, &rgba, w, h) {
                    Ok(()) => PreviewResult::KittyShown,
                    Err(e) => PreviewResult::RenderFailed(e.to_string()),
                },
            }
        } else {
            // xdg-open receives the original encrypted bytes; no RGBA decode needed.
            match image::open_with_xdg(&bytes, &ext) {
                Ok(()) => PreviewResult::XdgOpened,
                Err(e) => PreviewResult::RenderFailed(e),
            }
        }
    } else if media::is_media_ext(&ext) {
        match media::open_with_mpv(terminal, &bytes, &ext) {
            Ok(true)  => PreviewResult::MpvOpened,
            Ok(false) => PreviewResult::MpvNotInstalled,
            Err(e)    => PreviewResult::RenderFailed(e.to_string()),
        }
    } else {
        PreviewResult::NotSupported
    };

    state.phase = PreviewPhase::Done(result);
    state.path.clear();
    state.password.clear();
    state.focus = 0;
}
