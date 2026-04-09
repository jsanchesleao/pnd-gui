//! Key handler for the Vault page.

#[path = "handler/setup.rs"]
mod setup;

#[path = "handler/browse.rs"]
mod browse;

#[path = "handler/overlays.rs"]
mod overlays;

use crossterm::event::KeyCode;

use crate::App;
use super::state::Phase;

pub(crate) fn handle_vault(app: &mut App, code: KeyCode) {
    // Block all input while a background operation is running
    if app.vault.is_opening() || app.vault.is_adding()
        || app.vault.is_previewing() || app.vault.is_exporting()
        || app.vault.is_loading_gallery()
    {
        return;
    }

    match &app.vault.phase {
        Phase::VaultMenu { .. }        => setup::handle_vault_menu(app, code),
        Phase::Locked { .. }           => setup::handle_locked(app, code),
        Phase::Creating { .. }         => setup::handle_creating(app, code),
        Phase::ConfirmCreateDir { .. } => setup::handle_confirm_create_dir(app, code),
        Phase::Browse                  => browse::handle_browse(app, code),
        Phase::Rename { .. }           => overlays::handle_rename(app, code),
        Phase::ConfirmDelete { .. }    => overlays::handle_confirm_delete(app, code),
        Phase::Move { .. }             => overlays::handle_move(app, code),
        Phase::NewFolder { .. }        => overlays::handle_new_folder(app, code),
        Phase::Opening(_)              => {} // blocked above
        Phase::Adding { .. }           => {} // blocked above
        Phase::Previewing { .. }       => {} // blocked above
        Phase::PreviewReady { .. }     => {} // transient — handled by main loop
        Phase::Exporting { .. }        => {} // blocked above
        Phase::LoadingGallery { .. }   => {} // blocked above
        Phase::GalleryReady { .. }     => {} // transient — handled by main loop
    }
}
