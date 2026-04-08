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
            "Decrypt a .lock file and preview its contents",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        ))),
        c[0],
    );

    // [2] path input
    let inner_w = c[2].width.saturating_sub(4) as usize;
    let path_display = {
        let s = if state.focus == 0 { format!("{}|", state.path) } else { state.path.clone() };
        tail_fit(&s, inner_w).to_string()
    };
    let path_label = if state.focus == 0 { "File path  Enter→browse" } else { "File path" };
    frame.render_widget(
        Paragraph::new(path_display.as_str()).block(input_block(path_label, state.focus == 0)),
        c[2],
    );

    // [3] path sub-hint (only visible when path field is focused)
    if state.focus == 0 {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "  type a path, or press Enter to browse the filesystem",
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            )),
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
            };
            frame.render_widget(Paragraph::new(line), c[8]);
        }
        PreviewPhase::Idle => {}
    }

    // [10] hint bar
    let hint = if is_decrypting {
        "please wait…"
    } else {
        match state.focus {
            0 => "Esc back    Tab next field    Enter browse filesystem",
            _ => "Esc back    Tab previous field    Enter preview",
        }
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM))).alignment(Alignment::Center),
        c[10],
    );
}
