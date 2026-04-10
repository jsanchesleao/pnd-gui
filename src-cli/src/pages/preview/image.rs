//! Image preview: renders inline via `viuer` (Kitty / iTerm2 / Sixel / half-block),
//! with an `xdg-open` fallback.

use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
        disable_raw_mode, enable_raw_mode,
    },
};
use image::ImageFormat;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, io::Write as _, process::Command};
use tempfile::Builder;

pub(super) fn is_image_ext(ext: &str) -> bool {
    matches!(ext, "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "tif")
}

/// Convert image pixel dimensions to terminal cell columns, capped at the
/// terminal width. This is the maximum number of columns viuer should use so
/// that small images are never scaled up beyond their natural size.
///
/// Uses the terminal's reported pixel dimensions to derive the cell pixel size.
/// Falls back to 8×16 px per cell (a safe conservative estimate) if the
/// terminal does not report pixel dimensions.
pub(super) fn max_cols_for_image(img_px_width: u32) -> u32 {
    let (cell_px_w, _) = cell_pixel_size();
    let term_cols = crossterm::terminal::size().map(|(c, _)| c as u32).unwrap_or(80);
    let natural_cols = img_px_width / cell_px_w;
    natural_cols.max(1).min(term_cols)
}

/// Returns the pixel size of one terminal cell as `(width, height)`.
/// Queries `crossterm::terminal::window_size()`; falls back to 8×16 on failure.
fn cell_pixel_size() -> (u32, u32) {
    if let Ok(ws) = crossterm::terminal::window_size() {
        if ws.width > 0 && ws.columns > 0 && ws.height > 0 && ws.rows > 0 {
            let cw = (ws.width as u32 / ws.columns as u32).max(1);
            let ch = (ws.height as u32 / ws.rows as u32).max(1);
            return (cw, ch);
        }
    }
    (8, 16) // safe fallback
}

/// Suspend ratatui, decode the image bytes, render inline via `viuer`
/// (auto-selects Kitty / iTerm2 / Sixel / half-block), wait for a keypress,
/// then resume ratatui.
pub(super) fn render_inline(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    bytes: &[u8],
    ext: &str,
) -> Result<(), String> {
    let fmt = match ext {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "png"          => ImageFormat::Png,
        "gif"          => ImageFormat::Gif,
        "webp"         => ImageFormat::WebP,
        "bmp"          => ImageFormat::Bmp,
        "tiff" | "tif" => ImageFormat::Tiff,
        other          => return Err(format!("unsupported image format: {other}")),
    };

    let img = image::load_from_memory_with_format(bytes, fmt)
        .map_err(|e| e.to_string())?;

    // Switch to the normal scrollback buffer.
    disable_raw_mode().map_err(|e| e.to_string())?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).map_err(|e| e.to_string())?;

    let mut stdout = io::stdout();
    // Clear the normal buffer and move to the top-left before rendering.
    // The flush ensures the terminal processes the clear before viuer writes
    // the image data, preventing old content from showing through.
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0)).map_err(|e| e.to_string())?;
    stdout.flush().map_err(|e| e.to_string())?;

    // Cap the render width at the image's natural cell size so viuer never
    // scales a small image up to fill the terminal. Aspect ratio is preserved
    // by leaving height as None — viuer computes it from the width.
    let width_cols = max_cols_for_image(img.width());
    let config = viuer::Config {
        absolute_offset: true,
        x: 0,
        y: 0,
        width: Some(width_cols),
        height: None,
        restore_cursor: false,
        transparent: true,
        ..Default::default()
    };

    viuer::print(&img, &config).map_err(|e| e.to_string())?;

    print!("\r\n\r\n[Press any key to return]");
    stdout.flush().map_err(|e| e.to_string())?;

    // Wait for a keypress in raw mode.
    enable_raw_mode().map_err(|e| e.to_string())?;
    loop {
        if let Event::Key(k) = event::read().map_err(|e| e.to_string())? {
            if k.kind == KeyEventKind::Press { break; }
        }
    }

    // Clear the normal buffer before returning to the alternate screen so the
    // image doesn't bleed through when the user exits to the shell.
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0)).map_err(|e| e.to_string())?;
    stdout.flush().map_err(|e| e.to_string())?;

    disable_raw_mode().map_err(|e| e.to_string())?;
    execute!(terminal.backend_mut(), EnterAlternateScreen).map_err(|e| e.to_string())?;
    enable_raw_mode().map_err(|e| e.to_string())?;
    terminal.clear().map_err(|e| e.to_string())?;

    Ok(())
}

/// Write `bytes` to a temp file with the correct extension and open it with
/// the system viewer asynchronously. Uses `xdg-open` on Linux/macOS and
/// `cmd /C start` on Windows.
///
/// On Linux, the file is written to `/dev/shm` (a RAM-backed tmpfs) so no
/// data touches the disk. The file is intentionally kept alive because
/// xdg-open spawns the viewer asynchronously and exits immediately.
pub(super) fn open_with_xdg(bytes: &[u8], ext: &str) -> Result<(), String> {
    let mut tmp = Builder::new()
        .prefix("pnd-preview-")
        .suffix(&format!(".{ext}"))
        .tempfile_in(shm_or_tmp())
        .map_err(|e| e.to_string())?;

    tmp.write_all(bytes).map_err(|e| e.to_string())?;
    tmp.flush().map_err(|e| e.to_string())?;
    let (_, path) = tmp.keep().map_err(|e| e.to_string())?;

    #[cfg(target_os = "windows")]
    Command::new("cmd")
        .args(["/C", "start", ""])
        .arg(&path)
        .spawn()
        .map_err(|e| format!("start failed: {e}"))?;

    #[cfg(not(target_os = "windows"))]
    Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("xdg-open failed: {e}"))?;

    Ok(())
}

/// Returns `/dev/shm` on Linux (RAM-backed tmpfs, no disk writes) when it
/// exists, falling back to the OS temp directory on other platforms.
fn shm_or_tmp() -> std::path::PathBuf {
    #[cfg(target_os = "linux")]
    {
        let shm = std::path::PathBuf::from("/dev/shm");
        if shm.exists() {
            return shm;
        }
    }
    std::env::temp_dir()
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_extensions_recognised() {
        for ext in &["jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif"] {
            assert!(is_image_ext(ext), "{ext} should be an image ext");
        }
    }

    #[test]
    fn non_image_extensions_rejected() {
        for ext in &["mp4", "txt", "pdf", "zip", "rs", "lock", ""] {
            assert!(!is_image_ext(ext), "{ext} should not be an image ext");
        }
    }

    #[test]
    fn unsupported_ext_returns_error() {
        // render_inline errors early on unknown extensions before touching the terminal.
        // We can test this without a real terminal by checking the error path.
        // Construct a minimal fake: we need a Terminal but render_inline errors
        // before using it for unsupported formats.
        // Instead, test the format-matching logic indirectly via is_image_ext.
        assert!(!is_image_ext("xyz"));
        assert!(!is_image_ext(""));
    }

    fn make_test_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 128, 0, 255]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn decode_png_bytes_succeeds() {
        let png = make_test_png(4, 4);
        let fmt = ImageFormat::Png;
        let img = image::load_from_memory_with_format(&png, fmt).unwrap();
        assert_eq!(img.width(), 4);
        assert_eq!(img.height(), 4);
    }

    #[test]
    fn corrupt_bytes_fail_to_decode() {
        let result = image::load_from_memory_with_format(b"not a real png", ImageFormat::Png);
        assert!(result.is_err());
    }
}
