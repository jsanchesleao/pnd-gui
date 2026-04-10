//! Password acquisition shared by all CLI entry points.
//!
//! `PND_PASSWORD` env var is honoured for scripting; a warning is always
//! printed to stderr when it is used so the user knows the env var was active.
//! Falls back to a hidden terminal prompt via `rpassword`.

use std::process;

/// Return the password to use for a crypto operation.
///
/// Priority:
/// 1. `PND_PASSWORD` environment variable (warns on stderr).
/// 2. Interactive hidden prompt via `rpassword`.
///
/// Exits with code 2 if the interactive prompt fails (e.g. no TTY available).
pub(crate) fn read_password() -> String {
    if let Ok(pw) = std::env::var("PND_PASSWORD") {
        eprintln!("warning: using password from PND_PASSWORD environment variable");
        return pw;
    }
    rpassword::prompt_password("Password: ").unwrap_or_else(|e| {
        eprintln!("error: could not read password: {e}");
        process::exit(2);
    })
}
