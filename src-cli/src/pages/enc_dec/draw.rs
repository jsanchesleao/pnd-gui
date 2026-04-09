//! Draw function for the Encrypt/Decrypt page.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
};

use crate::{ACCENT, DIM, FAILURE, SUCCESS};
use crate::pages::widgets::{input_block, outer_block, tail_fit};
use crate::yazi::yazi_available;
use super::state::{EncDecState, OpStatus};

pub fn draw_enc_dec(frame: &mut Frame, state: &EncDecState) {
    let area = frame.area();
    frame.render_widget(outer_block("Encrypt / Decrypt"), area);

    let c = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1), // [0]  mode label
            Constraint::Length(1), // [1]  blank
            Constraint::Length(3), // [2]  path input
            Constraint::Length(1), // [3]  path sub-hint
            Constraint::Length(1), // [4]  blank
            Constraint::Length(3), // [5]  password input
            Constraint::Length(1), // [6]  blank
            Constraint::Length(1), // [7]  progress label  /  blank
            Constraint::Length(1), // [8]  progress gauge  /  status text
            Constraint::Min(0),    // [9]  filler
            Constraint::Length(1), // [10] hint
        ])
        .split(area);

    // [0] mode label
    let (mode_label, mode_color) = if state.is_decrypt() {
        ("Decrypt", Color::Cyan)
    } else {
        ("Encrypt", Color::LightYellow)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Mode: ", Style::default().fg(DIM)),
            Span::styled(mode_label, Style::default().fg(mode_color).add_modifier(Modifier::BOLD)),
            Span::styled(
                if state.is_decrypt() { "  (.lock suffix detected)" } else { "  (no .lock suffix)" },
                Style::default().fg(DIM),
            ),
        ])),
        c[0],
    );

    // [2] path input
    let path_focused = state.focus == 0;
    let inner_w = c[2].width.saturating_sub(4) as usize;
    let path_display = if path_focused && state.path_edit_mode {
        // Edit mode: show path with typing cursor
        tail_fit(&format!("{}|", state.path), inner_w).to_string()
    } else if state.path.is_empty() {
        String::new() // placeholder rendered separately below
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

    // [7] + [8]: progress bar while running, status text otherwise
    let is_running = matches!(state.status, OpStatus::Running(_));
    match &state.status {
        OpStatus::Running(pct) => {
            let action = if state.is_decrypt() { "Decrypting" } else { "Encrypting" };
            frame.render_widget(
                Paragraph::new(Span::styled(
                    format!("  {action}…"),
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
        other => {
            // [7] stays blank; [8] shows the result
            let status_line = match other {
                OpStatus::Idle => Line::from(""),
                OpStatus::Success(msg) => Line::from(Span::styled(
                    format!("✓  {msg}"),
                    Style::default().fg(SUCCESS),
                )),
                OpStatus::Failure(msg) => Line::from(Span::styled(
                    format!("✗  {msg}"),
                    Style::default().fg(FAILURE),
                )),
                OpStatus::Running(_) => unreachable!(),
            };
            frame.render_widget(Paragraph::new(status_line), c[8]);
        }
    }

    // [10] hint (context-sensitive)
    let hint = if is_running {
        "please wait…"
    } else if state.focus == 0 && state.path_edit_mode {
        "Esc cancel edit    Tab next field"
    } else {
        match state.focus {
            0 => "Esc back    Tab next field",
            _ => "Esc back    Tab previous field    Enter run",
        }
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM))).alignment(Alignment::Center),
        c[10],
    );
}
