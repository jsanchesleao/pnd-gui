//! Yazi integration: run yazi as a chooser when it is installed.
//!
//! yazi supports a `--chooser-file <path>` flag that writes the user's selection
//! (one path per line) to a file when they confirm. We use this to integrate
//! yazi as a drop-in replacement for the built-in TUI file browser.

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, path::{Path, PathBuf}, process::Command, sync::OnceLock};

use crate::file_browser::{FileBrowserEvent, FileBrowserTarget};

// ── Availability check ─────────────────────────────────────────────────────

static YAZI_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Returns `true` if `yazi` is found in `$PATH`. Result is cached after the
/// first call so repeated checks are free.
pub(crate) fn yazi_available() -> bool {
    *YAZI_AVAILABLE.get_or_init(|| {
        std::env::var_os("PATH").is_some_and(|path_var| {
            std::env::split_paths(&path_var).any(|dir| dir.join("yazi").is_file())
        })
    })
}

// ── Pending pick ───────────────────────────────────────────────────────────

/// Describes a file-pick that should be fulfilled by yazi on the next loop
/// iteration (so that the main loop can pass a `&mut Terminal` reference).
pub(crate) struct YaziPick {
    pub(crate) target: FileBrowserTarget,
    pub(crate) start_dir: Option<PathBuf>,
    /// `true` → expect multiple selections (vault add-files).
    /// When `false`, only the first selected path is used.
    pub(crate) multi: bool,
}

// ── Runner ─────────────────────────────────────────────────────────────────

/// Suspend ratatui, launch yazi in chooser mode, restore ratatui, and return
/// the user's selection as a [`FileBrowserEvent`].
///
/// Returns `None` only on hard I/O errors (e.g. cannot create the temp file).
pub(crate) fn run_yazi(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    start_dir: Option<&Path>,
    multi: bool,
) -> Option<FileBrowserEvent> {
    // yazi writes selected paths (one per line) to this temp file.
    let tmp = tempfile::NamedTempFile::new().ok()?;
    let tmp_path = tmp.path().to_path_buf();

    // Hand terminal control to yazi.
    disable_raw_mode().ok()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok()?;

    let mut cmd = Command::new("yazi");
    if let Some(dir) = start_dir {
        cmd.arg(dir);
    }
    cmd.arg("--chooser-file").arg(&tmp_path);
    let _ = cmd.status();

    // Restore ratatui regardless of how yazi exited.
    let _ = execute!(terminal.backend_mut(), EnterAlternateScreen);
    let _ = enable_raw_mode();
    let _ = terminal.clear();

    // Parse paths written by yazi.
    let content = std::fs::read_to_string(&tmp_path).unwrap_or_default();
    let paths: Vec<PathBuf> = content
        .lines()
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect();

    Some(if paths.is_empty() {
        FileBrowserEvent::Cancelled
    } else if multi {
        FileBrowserEvent::MultiSelected(paths)
    } else {
        FileBrowserEvent::Selected(paths.into_iter().next().unwrap())
    })
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yazi_available_does_not_panic() {
        // Just verify the function runs without panicking; we can't assert the
        // result because yazi may or may not be installed in the test environment.
        let _ = yazi_available();
    }
}
