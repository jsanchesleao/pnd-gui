//! Image preview: Kitty terminal graphics protocol and xdg-open fallback.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
               disable_raw_mode, enable_raw_mode},
};
use image::{ImageFormat, imageops::FilterType};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, io::Write as _, process::Command};
use tempfile::Builder;

pub(super) fn is_image_ext(ext: &str) -> bool {
    matches!(ext, "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "tif")
}

/// Returns true when the running terminal supports the Kitty graphics protocol.
/// Checks `$TERM` and `$TERM_PROGRAM` environment variables.
pub(crate) fn supports_kitty() -> bool {
    let term = std::env::var("TERM").unwrap_or_default();
    let prog = std::env::var("TERM_PROGRAM").unwrap_or_default().to_ascii_lowercase();
    term == "xterm-kitty" || prog == "kitty" || prog == "wezterm"
}

/// Query the terminal's usable pixel area for image display.
///
/// Reserves ≈3 rows for the "Press any key" prompt printed below the image.
/// Falls back to a cell-size estimate (8 × 16 px per cell) when the terminal
/// does not report pixel dimensions, and to 1920 × 1057 on any error.
pub(super) fn terminal_pixel_size() -> (u32, u32) {
    match crossterm::terminal::window_size() {
        Ok(ws) if ws.width > 0 && ws.height > 0 => {
            let cell_h = ws.height as u32 / ws.rows.max(1) as u32;
            let usable_h = (ws.height as u32).saturating_sub(cell_h * 3).max(1);
            (ws.width as u32, usable_h)
        }
        Ok(ws) => {
            let w = (ws.columns as u32 * 8).max(1);
            let h = ((ws.rows.saturating_sub(3)) as u32 * 16).max(1);
            (w, h)
        }
        Err(_) => (1920, 1057),
    }
}

/// Decode image bytes to raw RGBA, scaling down to fit `max_w × max_h`.
pub(super) fn decode_rgba(
    bytes: &[u8],
    ext: &str,
    max_w: u32,
    max_h: u32,
) -> Result<(Vec<u8>, u32, u32), String> {
    let fmt = match ext {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "png"          => ImageFormat::Png,
        "gif"          => ImageFormat::Gif,  // decodes first frame only
        "webp"         => ImageFormat::WebP,
        "bmp"          => ImageFormat::Bmp,
        "tiff" | "tif" => ImageFormat::Tiff,
        other          => return Err(format!("unsupported image format: {other}")),
    };

    let img = image::load_from_memory_with_format(bytes, fmt).map_err(|e| e.to_string())?;

    // Scale down to fit the terminal's usable pixel area.
    let img = {
        let (w, h) = (img.width(), img.height());
        if w > max_w || h > max_h {
            let scale = (max_w as f64 / w as f64).min(max_h as f64 / h as f64);
            let nw = ((w as f64 * scale) as u32).max(1);
            let nh = ((h as f64 * scale) as u32).max(1);
            img.resize(nw, nh, FilterType::Lanczos3)
        } else {
            img
        }
    };

    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok((rgba.into_raw(), w, h))
}

/// Transmit raw RGBA pixels via the Kitty terminal graphics protocol.
///
/// Data is split into 3 072-byte chunks (≤ 4 096 base64 characters each)
/// and sent as APC escape sequences: `\x1b_G<params>;<base64>\x1b\\`.
///
/// Key parameters:
/// - `a=T` — transmit and display immediately
/// - `f=32` — RGBA pixel format (4 bytes per pixel)
/// - `s` / `v` — width / height in pixels (first chunk only)
/// - `m=1` — more chunks follow; `m=0` — this is the last chunk
pub(super) fn transmit_kitty(out: &mut impl io::Write, rgba: &[u8], width: u32, height: u32) -> io::Result<()> {
    const CHUNK: usize = 3072;
    let chunks: Vec<&[u8]> = rgba.chunks(CHUNK).collect();
    let total = chunks.len();

    if total == 0 {
        // Degenerate case: send one empty terminal sequence to keep the protocol clean.
        write!(out, "\x1b_Ga=T,f=32,s={width},v={height},m=0;\x1b\\")?;
        return Ok(());
    }

    for (i, chunk) in chunks.iter().enumerate() {
        let encoded = STANDARD.encode(chunk);
        let more = u8::from(i + 1 < total);
        let params = if i == 0 {
            format!("a=T,f=32,s={width},v={height},m={more}")
        } else {
            format!("m={more}")
        };
        write!(out, "\x1b_G{params};{encoded}\x1b\\")?;
    }
    Ok(())
}

/// Suspend ratatui, render the image with the Kitty protocol, wait for a keypress,
/// then resume ratatui.
pub(super) fn render_kitty(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> io::Result<()> {
    // Leave the alternate screen so the image lands on the normal scrollback buffer.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;

    transmit_kitty(&mut stdout, rgba, width, height)?;
    stdout.flush()?;

    println!("\r\n\r\n[Press any key to return]");
    stdout.flush()?;

    // Wait for a keypress in raw mode, then restore the alternate screen.
    enable_raw_mode()?;
    loop {
        if let Event::Key(k) = event::read()? {
            if k.kind == KeyEventKind::Press { break; }
        }
    }

    // Delete the Kitty image and clear the normal screen before switching back
    // to the alternate screen.  Without this the image remains visible on the
    // normal screen and bleeds through when the user exits to the shell.
    let mut stdout = io::stdout();
    write!(stdout, "\x1b_Ga=d\x1b\\")?;          // Kitty: delete all image placements
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    stdout.flush()?;

    disable_raw_mode()?;

    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;

    Ok(())
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_image_ext ────────────────────────────────────────────────────────

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

    // ── transmit_kitty ──────────────────────────────────────────────────────

    #[test]
    fn transmit_kitty_single_chunk_format() {
        // 64 RGBA bytes (16 pixels) → fits in one chunk (< 3072 bytes)
        let rgba = vec![255u8; 64];
        let mut out = Vec::new();
        transmit_kitty(&mut out, &rgba, 4, 4).unwrap();

        let s = String::from_utf8(out).unwrap();
        // Must start with Kitty APC escape
        assert!(s.starts_with("\x1b_G"), "should start with APC escape");
        // First chunk carries image dimensions
        assert!(s.contains("a=T"), "should have transmit action");
        assert!(s.contains("f=32"), "should have RGBA format");
        assert!(s.contains("s=4"), "should have width=4");
        assert!(s.contains("v=4"), "should have height=4");
        // Single chunk: m=0 (no more)
        assert!(s.contains("m=0"), "single chunk should have m=0");
        // Must end with APC terminator
        assert!(s.ends_with("\x1b\\"), "should end with ST");
    }

    #[test]
    fn transmit_kitty_multi_chunk_has_continuation_flag() {
        // > 3072 bytes raw → multiple chunks
        let rgba = vec![128u8; 3073 * 4]; // definitely more than one chunk
        let mut out = Vec::new();
        transmit_kitty(&mut out, &rgba, 10, 10).unwrap();

        let s = String::from_utf8(out).unwrap();
        // The first chunk should have m=1 (more chunks follow)
        assert!(s.contains("m=1"), "first chunk of many should have m=1");
        // The output must also contain m=0 to close the sequence
        assert!(s.contains("m=0"), "last chunk should have m=0");
    }

    #[test]
    fn transmit_kitty_empty_rgba_does_not_panic() {
        let mut out = Vec::new();
        transmit_kitty(&mut out, &[], 0, 0).unwrap();
        // Degenerate path: one empty sequence is emitted
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("\x1b_G"), "even empty input should emit an APC sequence");
    }

    // ── decode_rgba ─────────────────────────────────────────────────────────

    fn make_test_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 128, 0, 255]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn decode_rgba_small_png() {
        let png = make_test_png(3, 2);
        let (rgba, w, h) = decode_rgba(&png, "png", 1920, 1080).unwrap();
        assert_eq!(w, 3);
        assert_eq!(h, 2);
        assert_eq!(rgba.len(), (3 * 2 * 4) as usize); // RGBA = 4 bytes/pixel
    }

    #[test]
    fn decode_rgba_scales_down_to_fit() {
        let png = make_test_png(100, 100);
        // Constrain to 10x10 — image must be scaled down
        let (_, w, h) = decode_rgba(&png, "png", 10, 10).unwrap();
        assert!(w <= 10, "width {w} should be ≤ 10");
        assert!(h <= 10, "height {h} should be ≤ 10");
    }

    #[test]
    fn decode_rgba_no_upscale_when_fits() {
        let png = make_test_png(4, 4);
        let (_, w, h) = decode_rgba(&png, "png", 1920, 1080).unwrap();
        // Image fits within bounds — dimensions must be unchanged
        assert_eq!(w, 4);
        assert_eq!(h, 4);
    }

    #[test]
    fn decode_rgba_unsupported_ext_returns_error() {
        let result = decode_rgba(b"irrelevant", "xyz", 1920, 1080);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unsupported image format"));
    }

    #[test]
    fn decode_rgba_corrupt_bytes_returns_error() {
        let result = decode_rgba(b"not a real png", "png", 1920, 1080);
        assert!(result.is_err());
    }
}

/// Write `bytes` to a temp file with the correct extension and open it with
/// the system viewer asynchronously. Uses `xdg-open` on Linux/macOS and
/// `cmd /C start` on Windows. The temp file is intentionally leaked so the
/// system viewer has time to read it.
pub(super) fn open_with_xdg(bytes: &[u8], ext: &str) -> Result<(), String> {
    let mut tmp = Builder::new()
        .prefix("pnd-preview-")
        .suffix(&format!(".{ext}"))
        .tempfile()
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
