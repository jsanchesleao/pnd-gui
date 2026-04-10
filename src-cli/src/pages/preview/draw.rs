//! Draw function for the Preview page.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
};

use crate::{ACCENT, DIM, FAILURE, SUCCESS};
use crate::pages::widgets::{input_block, outer_block, tail_fit};
use crate::yazi::yazi_available;
use super::state::{PreviewPhase, PreviewResult, PreviewState};

pub fn draw_preview(frame: &mut Frame, state: &PreviewState) {
    let area = frame.area();
    frame.render_widget(outer_block("Preview"), area);

    let c = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1), // [0]  info line
            Constraint::Length(1), // [1]  blank
            Constraint::Length(3), // [2]  path input
            Constraint::Length(1), // [3]  path sub-hint
            Constraint::Length(1), // [4]  blank
            Constraint::Length(3), // [5]  password input
            Constraint::Length(1), // [6]  blank
            Constraint::Length(1), // [7]  progress label  /  blank
            Constraint::Length(1), // [8]  progress gauge  /  status text
            Constraint::Min(0),    // [9]  filler
            Constraint::Length(1), // [10] hint bar
        ])
        .split(area);

    let is_decrypting = matches!(state.phase, PreviewPhase::Decrypting(_));

    // [0] info line
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Preview a file in-memory; .lock encrypted files are decrypted on the fly",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        ))),
        c[0],
    );

    // [2] path input
    let path_focused = state.focus == 0;
    let inner_w = c[2].width.saturating_sub(4) as usize;
    let path_display = if path_focused && state.path_edit_mode {
        tail_fit(&format!("{}|", state.path), inner_w).to_string()
    } else if state.path.is_empty() {
        String::new()
    } else {
        tail_fit(&state.path, inner_w).to_string()
    };
    let path_label = if path_focused && state.path_edit_mode {
        "File path  (editing)"
    } else {
        "File path"
    };
    let path_paragraph = if state.path.is_empty() && !state.path_edit_mode {
        Paragraph::new(Span::styled(
            "(no file selected)",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        ))
    } else {
        Paragraph::new(path_display.as_str())
    };
    frame.render_widget(
        path_paragraph.block(input_block(path_label, path_focused)),
        c[2],
    );

    // [3] path sub-hint (only visible when path field is focused)
    if path_focused {
        let hint_spans = if state.path_edit_mode {
            vec![
                Span::styled("  Enter", Style::default().fg(ACCENT)),
                Span::styled(" confirm    ", Style::default().fg(DIM)),
                Span::styled("Esc", Style::default().fg(ACCENT)),
                Span::styled(" cancel edit    ", Style::default().fg(DIM)),
                Span::styled("Tab", Style::default().fg(ACCENT)),
                Span::styled(" next field", Style::default().fg(DIM)),
            ]
        } else {
            let mut spans = vec![
                Span::styled("  t", Style::default().fg(ACCENT)),
                Span::styled(" type    ", Style::default().fg(DIM)),
                Span::styled("b", Style::default().fg(ACCENT)),
                Span::styled(" browser    ", Style::default().fg(DIM)),
            ];
            if yazi_available() {
                spans.push(Span::styled("y", Style::default().fg(ACCENT)));
                spans.push(Span::styled(" yazi    ", Style::default().fg(DIM)));
            }
            spans.push(Span::styled("Enter", Style::default().fg(ACCENT)));
            spans.push(Span::styled(" auto-pick", Style::default().fg(DIM)));
            spans
        };
        frame.render_widget(
            Paragraph::new(Line::from(hint_spans)),
            c[3],
        );
    }

    // [5] password input (masked)
    let masked = "•".repeat(state.password.len());
    let pass_display = if state.focus == 1 { format!("{masked}|") } else { masked };
    frame.render_widget(
        Paragraph::new(pass_display.as_str()).block(input_block("Password", state.focus == 1)),
        c[5],
    );

    // [7] + [8]: progress bar while decrypting, status text otherwise
    match &state.phase {
        PreviewPhase::Decrypting(pct) => {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "  Decrypting…",
                    Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
                )),
                c[7],
            );
            frame.render_widget(
                Gauge::default()
                    .gauge_style(Style::default().fg(ACCENT).bg(DIM))
                    .ratio(*pct as f64 / 100.0)
                    .label(format!("{pct}%")),
                c[8],
            );
        }
        PreviewPhase::PendingRender { .. } => {
            // Briefly visible between the worker finishing and render_preview running.
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "  Rendering…",
                    Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
                )),
                c[7],
            );
        }
        PreviewPhase::Done(result) => {
            let line = match result {
                PreviewResult::NotSupported => Line::from(Span::styled(
                    "✗  Preview not available for this file type",
                    Style::default().fg(FAILURE),
                )),
                PreviewResult::WrongPassword => Line::from(Span::styled(
                    "✗  Wrong password or corrupted file",
                    Style::default().fg(FAILURE),
                )),
                PreviewResult::IoError(msg) => Line::from(Span::styled(
                    format!("✗  Error: {msg}"),
                    Style::default().fg(FAILURE),
                )),
                PreviewResult::KittyShown => Line::from(Span::styled(
                    "✓  Image displayed in terminal",
                    Style::default().fg(SUCCESS),
                )),
                PreviewResult::XdgOpened => Line::from(Span::styled(
                    "✓  Opened in system image viewer",
                    Style::default().fg(SUCCESS),
                )),
                PreviewResult::RenderFailed(msg) => Line::from(Span::styled(
                    format!("✗  Render failed: {msg}"),
                    Style::default().fg(FAILURE),
                )),
                PreviewResult::MpvOpened => Line::from(Span::styled(
                    "✓  Playback finished",
                    Style::default().fg(SUCCESS),
                )),
                PreviewResult::MpvNotInstalled => Line::from(Span::styled(
                    "✗  mpv not found — install it to preview media files",
                    Style::default().fg(FAILURE),
                )),
                PreviewResult::GalleryShown(n) => Line::from(Span::styled(
                    format!("✓  Gallery closed ({n} image{})", if *n == 1 { "" } else { "s" }),
                    Style::default().fg(SUCCESS),
                )),
                PreviewResult::GalleryXdgOpened => Line::from(Span::styled(
                    "✓  Opened ZIP in system viewer",
                    Style::default().fg(SUCCESS),
                )),
                PreviewResult::TextShown(n) => Line::from(Span::styled(
                    if *n > 0 {
                        format!("✓  Text viewer closed ({n} lines)")
                    } else {
                        "✓  bat viewer closed".to_string()
                    },
                    Style::default().fg(SUCCESS),
                )),
            };
            frame.render_widget(Paragraph::new(line), c[8]);
        }
        PreviewPhase::Idle => {}
    }

    // [10] hint bar
    let hint = if is_decrypting {
        "please wait…"
    } else if state.focus == 0 && state.path_edit_mode {
        "Esc cancel edit    Tab next field"
    } else {
        match state.focus {
            0 => "Esc back    Tab next field",
            _ => "Esc back    Tab previous field    Enter preview",
        }
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM))).alignment(Alignment::Center),
        c[10],
    );
}
