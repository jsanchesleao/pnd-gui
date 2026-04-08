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
    } else {
        match state.focus {
            0 => "Esc back    Tab next field    Enter browse filesystem",
            _ => "Esc back    Tab previous field    Enter run",
        }
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM))).alignment(Alignment::Center),
        c[10],
    );
}
