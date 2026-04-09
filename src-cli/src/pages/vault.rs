//! Vault page — module root.
//!
//! Sub-modules live in the `vault/` directory alongside this file and are
//! wired in with `#[path]` so the module tree matches the folder structure
//! without requiring a full `vault/mod.rs` conversion.

#[path = "vault/types.rs"]
pub(crate) mod types;

#[path = "vault/crypto.rs"]
pub(crate) mod crypto;

#[path = "vault/state.rs"]
pub(crate) mod state;

#[path = "vault/draw.rs"]
mod draw;

#[path = "vault/handler.rs"]
mod handler;

pub(crate) use state::{Phase, VaultState};
pub(crate) use draw::draw_vault;
pub(crate) use handler::handle_vault;

use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, mem};

/// Called from the main event loop when the vault phase is `PreviewReady`.
/// Pulls the decrypted bytes out, dispatches to the existing preview pipeline,
/// and returns the vault to `Browse`, optionally setting a status message on failure.
pub(crate) fn render_vault_preview(
    vault: &mut VaultState,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) {
    let (bytes, ext) = match mem::replace(&mut vault.phase, Phase::Browse) {
        Phase::PreviewReady { bytes, ext } => (bytes, ext),
        other => { vault.phase = other; return; }
    };

    // Reuse the preview page's rendering pipeline via a temporary PreviewState.
    use crate::pages::preview::{PreviewPhase, PreviewResult, PreviewState};
    let mut tmp = PreviewState::new();
    tmp.phase = PreviewPhase::PendingRender { bytes, ext };
    crate::pages::preview::render_preview(&mut tmp, terminal);

    // Show an error status if rendering failed; successful previews speak for themselves.
    let msg: String = match &tmp.phase {
        PreviewPhase::Done(r) => match r {
            PreviewResult::NotSupported       => "Unsupported file type — no previewer available".into(),
            PreviewResult::MpvNotInstalled    => "Install mpv to preview media files".into(),
            PreviewResult::RenderFailed(e)    => format!("Preview failed: {e}"),
            PreviewResult::WrongPassword      => "Decryption error (wrong key?)".into(),
            PreviewResult::IoError(e)         => format!("I/O error: {e}"),
            // Successful: KittyShown, XdgOpened, MpvOpened, GalleryShown, GalleryXdgOpened, TextShown
            _                                 => String::new(),
        },
        _ => String::new(),
    };

    if let Some(b) = &mut vault.browse {
        if !msg.is_empty() { b.set_status(msg); }
    }
}

/// Called from the main event loop when the vault phase is `GalleryReady`.
/// Pulls the decrypted images out, runs the interactive gallery, and returns
/// the vault to `Browse`, optionally setting a status message.
pub(crate) fn render_vault_gallery(
    vault: &mut VaultState,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) {
    let images = match mem::replace(&mut vault.phase, Phase::Browse) {
        Phase::GalleryReady { images } => images,
        other => { vault.phase = other; return; }
    };

    use crate::pages::preview::gallery::GalleryOutcome;
    let msg: String = if crate::pages::preview::image::supports_kitty() {
        match crate::pages::preview::gallery::show_images_kitty(terminal, &images) {
            Ok(GalleryOutcome::Shown(n)) => format!("Gallery: {n} image(s) shown"),
            Ok(GalleryOutcome::NoImages) => "No images to display".into(),
            Ok(GalleryOutcome::XdgOpened) => String::new(),
            Err(e) => format!("Gallery error: {e}"),
        }
    } else {
        "Gallery requires a Kitty-compatible terminal".into()
    };

    if let Some(b) = &mut vault.browse {
        if !msg.is_empty() { b.set_status(msg); }
    }
}
