//! ZIP image gallery preview.
//!
//! Renders each image inline via `viuer` (Kitty / iTerm2 / Sixel / half-block)
//! with keyboard navigation. Falls back to xdg-open on render error.

use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
               disable_raw_mode, enable_raw_mode},
};
use image::ImageFormat;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, io::{Read, Write as _}};

pub(crate) enum GalleryOutcome {
    /// Gallery was displayed inline; carries the total image count.
    Shown(usize),
    /// ZIP opened with the system file handler (xdg-open).
    XdgOpened,
    /// ZIP contained no recognisable image files.
    NoImages,
}

/// Entry point. Tries inline rendering first; falls back to xdg-open on error.
pub(super) fn show_gallery(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    bytes: &[u8],
) -> io::Result<GalleryOutcome> {
    let entries = extract_images(bytes)?;
    if entries.is_empty() {
        return Ok(GalleryOutcome::NoImages);
    }
    match show_images_inline(terminal, &entries) {
        Ok(outcome) => Ok(outcome),
        Err(_) => {
            super::image::open_with_xdg(bytes, "zip")
                .map_err(io::Error::other)?;
            Ok(GalleryOutcome::XdgOpened)
        }
    }
}

// ── ZIP extraction ─────────────────────────────────────────────────────────

pub(crate) fn is_image_entry(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let ext = std::path::Path::new(&lower)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    matches!(ext, "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "tif")
}

fn entry_ext(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    std::path::Path::new(&lower)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string()
}

/// Decompress all image entries from `bytes` into memory, sorted alphabetically by name.
fn extract_images(bytes: &[u8]) -> io::Result<Vec<(String, Vec<u8>)>> {
    let cursor = io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)?;
    let mut entries = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if !entry.is_file() {
            continue;
        }
        let name = entry.name().to_string();
        if !is_image_entry(&name) {
            continue;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        entries.push((name, buf));
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
}

// ── Inline gallery ─────────────────────────────────────────────────────────

/// Display a pre-loaded slice of `(filename, raw image bytes)` as an interactive
/// inline gallery. Suspends ratatui, runs the navigation loop, then resumes.
///
/// `viuer` auto-selects the best image protocol for the current terminal
/// (Kitty, iTerm2, Sixel, or half-block characters).
pub(crate) fn show_images_inline(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    images: &[(String, Vec<u8>)],
) -> io::Result<GalleryOutcome> {
    if images.is_empty() {
        return Ok(GalleryOutcome::NoImages);
    }

    let count = images.len();
    let mut idx = 0usize;

    // Suspend ratatui so we can draw directly to the normal screen buffer.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    let (term_cols, term_rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let viuer_config = viuer::Config {
        absolute_offset: true,
        x: 0,
        y: 0,
        width: Some(term_cols as u32),
        height: Some(term_rows.saturating_sub(4) as u32),
        restore_cursor: false,
        transparent: true,
        ..Default::default()
    };

    loop {
        let (name, img_bytes) = &images[idx];
        let ext = entry_ext(name);

        let mut stdout = io::stdout();
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;

        let fmt = match ext.as_str() {
            "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
            "png"          => Some(ImageFormat::Png),
            "gif"          => Some(ImageFormat::Gif),
            "webp"         => Some(ImageFormat::WebP),
            "bmp"          => Some(ImageFormat::Bmp),
            "tiff" | "tif" => Some(ImageFormat::Tiff),
            _              => None,
        };

        match fmt.and_then(|f| image::load_from_memory_with_format(img_bytes, f).ok()) {
            Some(img) => {
                let _ = viuer::print(&img, &viuer_config);
            }
            None => {
                write!(stdout, "[Could not decode {name}]")?;
            }
        }

        // Navigation hint line below the image.
        let hint = if count == 1 {
            format!("\r\n\r\n  (1/1)  {name}    [Esc/q] exit\r\n")
        } else {
            format!(
                "\r\n\r\n  ({}/{})  {name}    [←/h] prev   [→/l] next   [Esc/q] exit\r\n",
                idx + 1,
                count,
            )
        };
        write!(stdout, "{hint}")?;
        stdout.flush()?;

        // Read the next keypress in raw mode.
        enable_raw_mode()?;
        let action = loop {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    break k.code;
                }
            }
        };
        disable_raw_mode()?;

        match action {
            KeyCode::Left | KeyCode::Char('h') => {
                if idx > 0 { idx -= 1; }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if idx + 1 < count { idx += 1; }
            }
            KeyCode::Esc | KeyCode::Char('q') => break,
            _ => {}
        }
    }

    // Clear the normal screen before switching back to the alternate screen.
    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    stdout.flush()?;

    // Resume ratatui.
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;

    Ok(GalleryOutcome::Shown(count))
}
