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
pub(super) fn supports_kitty() -> bool {
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
fn transmit_kitty(out: &mut impl io::Write, rgba: &[u8], width: u32, height: u32) -> io::Result<()> {
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
    disable_raw_mode()?;

    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;

    Ok(())
}

/// Write `bytes` to a temp file with the correct extension and open it with
/// xdg-open asynchronously. The temp file is intentionally leaked in `/tmp`
/// so the system viewer has time to read it.
pub(super) fn open_with_xdg(bytes: &[u8], ext: &str) -> Result<(), String> {
    let mut tmp = Builder::new()
        .prefix("pnd-preview-")
        .suffix(&format!(".{ext}"))
        .tempfile()
        .map_err(|e| e.to_string())?;

    tmp.write_all(bytes).map_err(|e| e.to_string())?;
    tmp.flush().map_err(|e| e.to_string())?;
    let (_, path) = tmp.keep().map_err(|e| e.to_string())?;

    Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("xdg-open failed: {e}"))?;

    Ok(())
}
