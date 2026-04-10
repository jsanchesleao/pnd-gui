mod draw;
pub(crate) mod gallery;
mod handler;
pub(crate) mod image;
mod media;
mod state;
mod text;

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
        // viuer auto-detects the best protocol (Kitty, iTerm2, Sixel, half-block)
        // and falls back to an error only when the terminal is truly incapable.
        match image::render_inline(terminal, &bytes, &ext) {
            Ok(()) => PreviewResult::InlineShown,
            Err(_) => match image::open_with_xdg(&bytes, &ext) {
                Ok(()) => PreviewResult::XdgOpened,
                Err(e) => PreviewResult::RenderFailed(e),
            },
        }
    } else if media::is_media_ext(&ext) {
        match media::open_with_mpv(terminal, &bytes, &ext) {
            Ok(true)  => PreviewResult::MpvOpened,
            Ok(false) => PreviewResult::MpvNotInstalled,
            Err(e)    => PreviewResult::RenderFailed(e.to_string()),
        }
    } else if ext == "zip" {
        match gallery::show_gallery(terminal, &bytes) {
            Ok(gallery::GalleryOutcome::Shown(n))  => PreviewResult::GalleryShown(n),
            Ok(gallery::GalleryOutcome::XdgOpened) => PreviewResult::GalleryXdgOpened,
            Ok(gallery::GalleryOutcome::NoImages)   => PreviewResult::NotSupported,
            Err(e) => PreviewResult::RenderFailed(e.to_string()),
        }
    } else if text::is_text_ext(&ext) {
        text::show_text(&bytes, &ext, terminal)
    } else {
        PreviewResult::NotSupported
    };

    state.phase = PreviewPhase::Done(result);
    state.path.clear();
    state.password.clear();
    state.focus = 0;
}
