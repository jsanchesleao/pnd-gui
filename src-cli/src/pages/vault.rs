//! Vault page.
//!
//! Sub-modules live in the `vault/` directory alongside this file and are
//! wired in with `#[path]` so the module tree matches the folder structure
//! without requiring a full `vault/mod.rs` conversion.

#[path = "vault/types.rs"]
pub(crate) mod types;

#[path = "vault/crypto.rs"]
pub(crate) mod crypto;

// ── Stub UI (remaining implementation steps) ──────────────────────────────

use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::{App, ACCENT, DIM};

pub fn draw_vault(frame: &mut Frame) {
    let area = frame.area();
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT))
            .title(Span::styled(
                " Vault ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center),
        area,
    );

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "coming soon",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        )))
        .alignment(Alignment::Center),
        inner[0],
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Esc / Backspace / q  back",
            Style::default().fg(DIM),
        )))
        .alignment(Alignment::Center),
        inner[1],
    );
}

pub fn handle_vault(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc | KeyCode::Backspace => app.back(),
        _ => {}
    }
}
