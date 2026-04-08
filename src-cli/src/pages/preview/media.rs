//! Media preview via mpv.

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, io::Write as _, process::Command};
use tempfile::Builder;

pub(super) fn is_media_ext(ext: &str) -> bool {
    matches!(
        ext,
        // video
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv" | "m4v" | "ts" | "ogv"
        // audio
        | "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" | "opus" | "wma"
    )
}

/// Write `bytes` to a temp file and play it with mpv, blocking until playback ends.
///
/// Ratatui is suspended for the duration so mpv can own the terminal.
/// Returns:
/// - `Ok(true)`  — mpv was found and ran (playback finished normally or the user quit)
/// - `Ok(false)` — mpv was not found on `$PATH`
/// - `Err(...)`  — I/O failure writing the temp file or re-entering the alternate screen
pub(super) fn open_with_mpv(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    bytes: &[u8],
    ext: &str,
) -> io::Result<bool> {
    let mut tmp = Builder::new()
        .prefix("pnd-preview-")
        .suffix(&format!(".{ext}"))
        .tempfile()?;

    tmp.write_all(bytes)?;
    tmp.flush()?;
    // Persist the file so mpv can open it; it will be cleaned up on the next OS reboot.
    let (_, path) = tmp.keep().map_err(|e| io::Error::other(e.to_string()))?;

    // Hand terminal control to mpv.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    let status = Command::new("mpv").arg(&path).status();

    // Restore ratatui regardless of whether mpv succeeded.
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;

    match status {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
        Ok(_) => Ok(true),
    }
}
