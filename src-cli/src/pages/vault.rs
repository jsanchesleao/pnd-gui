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

pub(crate) use state::VaultState;
pub(crate) use draw::draw_vault;
pub(crate) use handler::handle_vault;
