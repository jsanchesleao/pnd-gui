//! Media preview via mpv.

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, io::Write as _, process::{Command, Stdio}, thread};

pub(super) fn is_media_ext(ext: &str) -> bool {
    matches!(
        ext,
        // video
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv" | "m4v" | "ts" | "ogv"
        // audio
        | "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" | "opus" | "wma"
    )
}

/// Map a file extension to the libavformat format name that mpv expects when
/// reading from stdin (where container probing by seeking is unavailable).
fn lavf_format(ext: &str) -> &str {
    match ext {
        "mkv"       => "matroska",
        "ogv"       => "ogg",
        "ts"        => "mpegts",
        "wmv" | "wma" => "asf",
        "m4v" | "m4a" => "mp4",
        other       => other,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_extensions_recognised() {
        for ext in &["mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "m4v", "ts", "ogv"] {
            assert!(is_media_ext(ext), "{ext} should be a media ext");
        }
    }

    #[test]
    fn audio_extensions_recognised() {
        for ext in &["mp3", "flac", "wav", "ogg", "m4a", "aac", "opus", "wma"] {
            assert!(is_media_ext(ext), "{ext} should be a media ext");
        }
    }

    #[test]
    fn non_media_extensions_rejected() {
        for ext in &["jpg", "png", "txt", "pdf", "zip", "rs", "lock", ""] {
            assert!(!is_media_ext(ext), "{ext} should not be a media ext");
        }
    }

    #[test]
    fn lavf_format_mappings() {
        assert_eq!(lavf_format("mkv"),  "matroska");
        assert_eq!(lavf_format("ogv"),  "ogg");
        assert_eq!(lavf_format("ts"),   "mpegts");
        assert_eq!(lavf_format("wmv"),  "asf");
        assert_eq!(lavf_format("wma"),  "asf");
        assert_eq!(lavf_format("m4v"),  "mp4");
        assert_eq!(lavf_format("m4a"),  "mp4");
        assert_eq!(lavf_format("mp4"),  "mp4");
        assert_eq!(lavf_format("mp3"),  "mp3");
        assert_eq!(lavf_format("flac"), "flac");
    }
}

/// Pipe `bytes` into mpv via stdin and block until playback ends.
///
/// Ratatui is suspended for the duration so mpv can own the terminal.
/// Returns:
/// - `Ok(true)`  — mpv was found and ran (playback finished normally or the user quit)
/// - `Ok(false)` — mpv was not found on `$PATH`
/// - `Err(...)`  — I/O failure piping bytes or re-entering the alternate screen
pub(super) fn open_with_mpv(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    bytes: &[u8],
    ext: &str,
) -> io::Result<bool> {
    // Hand terminal control to mpv.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    let spawn_result = Command::new("mpv")
        .arg(format!("--demuxer-lavf-format={}", lavf_format(ext)))
        .arg("-") // read from stdin
        .stdin(Stdio::piped())
        .spawn();

    let run_result = match spawn_result {
        Ok(mut child) => {
            let bytes_owned = bytes.to_vec();
            let mut stdin = child.stdin.take().expect("stdin was piped");
            // Write in a separate thread so we can wait() on the child concurrently.
            let writer = thread::spawn(move || stdin.write_all(&bytes_owned));
            let status = child.wait();
            let _ = writer.join(); // ignore broken-pipe when the user quits early
            status.map(|_| true)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    };

    // Restore ratatui regardless of whether mpv succeeded.
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;

    // Windows fallback when mpv is absent — `cmd /C start` needs a real file path,
    // so we fall back to a temp file only in this narrow case.
    #[cfg(target_os = "windows")]
    if matches!(run_result, Ok(false)) {
        use tempfile::Builder;
        let mut tmp = Builder::new()
            .prefix("pnd-preview-")
            .suffix(&format!(".{ext}"))
            .tempfile()?;
        tmp.write_all(bytes)?;
        tmp.flush()?;
        let (_, path) = tmp.keep().map_err(|e| io::Error::other(e.to_string()))?;
        Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(&path)
            .spawn()?;
        return Ok(true);
    }

    run_result
}
