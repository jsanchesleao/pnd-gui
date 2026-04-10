//! Plain-text file preview.
//!
//! Tries `bat` first (syntax-highlighted output with its built-in pager).
//! Falls back to a scrollable ratatui viewer when `bat` is not installed.

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::{io, io::Write as _, process::{Command, Stdio}, thread, time::Duration};

use crate::DIM;
use super::state::PreviewResult;

pub(super) fn is_text_ext(ext: &str) -> bool {
    matches!(
        ext,
        "txt" | "md" | "markdown"
        | "json" | "jsonc" | "json5"
        | "yml" | "yaml"
        | "toml" | "ini" | "cfg" | "conf" | "env"
        | "log" | "csv"
        | "xml" | "html" | "htm" | "svg"
        | "rs" | "py" | "js" | "ts" | "jsx" | "tsx"
        | "sh" | "bash" | "zsh" | "fish"
        | "css" | "scss" | "less"
        | "sql" | "graphql" | "gql"
        | "c" | "cpp" | "h" | "hpp"
        | "java" | "go" | "rb" | "php" | "swift" | "kt" | "lua"
        | "diff" | "patch"
    )
}

/// Preview `bytes` as text. Uses `bat` for syntax-highlighted output if available,
/// otherwise falls back to a scrollable ratatui viewer.
pub(super) fn show_text(
    bytes: &[u8],
    ext: &str,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> PreviewResult {
    if bat_available() {
        match show_with_bat(terminal, bytes, ext) {
            Ok(()) => return PreviewResult::TextShown(0),
            Err(e) => return PreviewResult::RenderFailed(e.to_string()),
        }
    }

    let text = String::from_utf8_lossy(bytes);
    match show_with_ratatui(terminal, &text, ext) {
        Ok(line_count) => PreviewResult::TextShown(line_count),
        Err(e) => PreviewResult::RenderFailed(e.to_string()),
    }
}

// ── bat shim ───────────────────────────────────────────────────────────────

fn bat_available() -> bool {
    Command::new("bat")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn show_with_bat(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    bytes: &[u8],
    ext: &str,
) -> io::Result<()> {
    // Suspend ratatui — bat (with its default `less` pager) takes over the terminal.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    // Pipe bytes via stdin; --file-name gives bat the extension for syntax detection.
    let mut child = Command::new("bat")
        .args(["--paging=always", "-p", "--file-name"])
        .arg(format!("file.{ext}"))
        .arg("-") // read from stdin
        .stdin(Stdio::piped())
        .spawn()?;

    let bytes_owned = bytes.to_vec();
    let mut stdin = child.stdin.take().expect("stdin was piped");
    let writer = thread::spawn(move || stdin.write_all(&bytes_owned));
    child.wait().ok();
    let _ = writer.join(); // ignore broken-pipe when the user quits early

    // Resume ratatui.
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;

    Ok(())
}

// ── ratatui fallback ───────────────────────────────────────────────────────

fn show_with_ratatui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    text: &str,
    ext: &str,
) -> io::Result<usize> {
    let lines: Vec<&str> = text.lines().collect();
    let line_count = lines.len();
    let mut scroll: usize = 0;

    loop {
        let size = terminal.size()?;
        // Subtract top/bottom borders (2) + footer line (1) + a small margin (1).
        let visible = size.height.saturating_sub(4) as usize;

        terminal.draw(|frame| {
            let area = frame.area();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(1)])
                .split(area);

            // Clamp scroll so the last page fills the viewport.
            if line_count > visible {
                scroll = scroll.min(line_count - visible);
            } else {
                scroll = 0;
            }

            let end = (scroll + visible).min(line_count);
            let visible_lines: Vec<Line> = lines[scroll..end]
                .iter()
                .map(|l| Line::from(*l))
                .collect();

            let title = format!(
                " .{ext}  {}/{line_count} ",
                scroll + 1,
            );
            let block = Block::default()
                .title(Span::styled(title, Style::default().add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(DIM));

            frame.render_widget(Paragraph::new(visible_lines).block(block), chunks[0]);

            let hint = "  ↑/k up   ↓/j down   PgUp/PgDn   g top   G end   q exit";
            frame.render_widget(
                Paragraph::new(Span::styled(hint, Style::default().fg(DIM))),
                chunks[1],
            );
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    let step = (size_visible(terminal) as usize).max(1);
                    match k.code {
                        KeyCode::Esc | KeyCode::Char('q') => break,
                        KeyCode::Up | KeyCode::Char('k') => {
                            scroll = scroll.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            scroll += 1; // clamped on next draw
                        }
                        KeyCode::PageUp => {
                            scroll = scroll.saturating_sub(step);
                        }
                        KeyCode::PageDown => {
                            scroll += step; // clamped on next draw
                        }
                        KeyCode::Char('g') | KeyCode::Home => {
                            scroll = 0;
                        }
                        KeyCode::Char('G') | KeyCode::End => {
                            scroll = line_count; // clamped on next draw
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(line_count)
}

fn size_visible(terminal: &Terminal<CrosstermBackend<io::Stdout>>) -> u16 {
    terminal.size().map(|s| s.height.saturating_sub(4)).unwrap_or(20)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_text_ext ─────────────────────────────────────────────────────────

    #[test]
    fn plain_text_formats_recognised() {
        for ext in &["txt", "md", "markdown", "log", "csv", "diff", "patch"] {
            assert!(is_text_ext(ext), "{ext} should be a text ext");
        }
    }

    #[test]
    fn data_formats_recognised() {
        for ext in &["json", "jsonc", "json5", "yml", "yaml", "toml", "ini", "cfg", "conf", "env"] {
            assert!(is_text_ext(ext), "{ext} should be a text ext");
        }
    }

    #[test]
    fn markup_formats_recognised() {
        for ext in &["xml", "html", "htm", "svg"] {
            assert!(is_text_ext(ext), "{ext} should be a text ext");
        }
    }

    #[test]
    fn code_extensions_recognised() {
        for ext in &[
            "rs", "py", "js", "ts", "jsx", "tsx",
            "sh", "bash", "zsh", "fish",
            "css", "scss", "less",
            "sql", "graphql", "gql",
            "c", "cpp", "h", "hpp",
            "java", "go", "rb", "php", "swift", "kt", "lua",
        ] {
            assert!(is_text_ext(ext), "{ext} should be a text ext");
        }
    }

    #[test]
    fn binary_and_media_extensions_rejected() {
        for ext in &["jpg", "jpeg", "png", "gif", "mp4", "mkv", "mp3",
                     "zip", "gz", "tar", "pdf", "lock", "exe", "bin", ""] {
            assert!(!is_text_ext(ext), "{ext} should not be a text ext");
        }
    }
}
