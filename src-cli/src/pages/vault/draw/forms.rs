//! Draw functions for the pre-browse vault forms: menu, unlock, and create-new.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
};

use crate::{ACCENT, DIM, FAILURE, SUCCESS};
use crate::pages::widgets::{input_block, outer_block, tail_fit};
use super::util::path_input_hint_spans;

pub(super) fn draw_vault_menu(frame: &mut Frame, cursor: usize) {
    let area = frame.area();
    frame.render_widget(outer_block("Vault"), area);

    let c = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1), // [0] description
            Constraint::Length(2), // [1] spacer
            Constraint::Length(1), // [2] item 0 — Open Vault
            Constraint::Length(1), // [3] item 0 description
            Constraint::Length(1), // [4] spacer
            Constraint::Length(1), // [5] item 1 — New Vault
            Constraint::Length(1), // [6] item 1 description
            Constraint::Min(0),    // [7] filler
            Constraint::Length(1), // [8] hint
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "Select an action:",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        )),
        c[0],
    );

    let items: &[(&str, &str)] = &[
        ("Open Vault",  "Unlock and browse an existing encrypted vault"),
        ("New Vault",   "Create a new empty vault in a folder you choose"),
    ];

    for (i, (label, desc)) in items.iter().enumerate() {
        let (row_label, row_desc) = if i == 0 { (c[2], c[3]) } else { (c[5], c[6]) };
        let selected = cursor == i;

        let prefix = if selected { "▶ " } else { "  " };
        let label_style = if selected {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let desc_style = if selected {
            Style::default().fg(SUCCESS)
        } else {
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC)
        };

        frame.render_widget(
            Paragraph::new(Span::styled(format!("{prefix}{label}"), label_style)),
            row_label,
        );
        frame.render_widget(
            Paragraph::new(Span::styled(format!("    {desc}"), desc_style)),
            row_desc,
        );
    }

    frame.render_widget(
        Paragraph::new(Span::styled(
            "↑↓ / jk  navigate    Enter  select    Esc/h  back to main menu",
            Style::default().fg(DIM),
        ))
        .alignment(Alignment::Center),
        c[8],
    );
}

pub(super) fn draw_locked(
    frame: &mut Frame,
    vault_path: &str,
    password: &str,
    focus: usize,
    path_edit_mode: bool,
    error: Option<&str>,
    is_opening: bool,
) {
    let area = frame.area();
    frame.render_widget(outer_block("Vault"), area);

    let c = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1), // [0] info
            Constraint::Length(1), // [1] blank
            Constraint::Length(3), // [2] vault path
            Constraint::Length(1), // [3] path hint
            Constraint::Length(1), // [4] blank
            Constraint::Length(3), // [5] password
            Constraint::Length(1), // [6] blank
            Constraint::Length(1), // [7] status label
            Constraint::Length(1), // [8] status / error
            Constraint::Min(0),    // [9] filler
            Constraint::Length(1), // [10] hint bar
        ])
        .split(area);

    // [0] info
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Open an encrypted vault folder",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        ))),
        c[0],
    );

    // [2] vault path
    let path_focused = focus == 0;
    let inner_w = c[2].width.saturating_sub(4) as usize;
    let path_display = if path_focused && path_edit_mode {
        tail_fit(&format!("{vault_path}|"), inner_w).to_string()
    } else if vault_path.is_empty() {
        String::new()
    } else {
        tail_fit(vault_path, inner_w).to_string()
    };
    let path_label = if path_focused && path_edit_mode {
        "Vault folder  (editing)"
    } else {
        "Vault folder"
    };
    let path_paragraph = if vault_path.is_empty() && !path_edit_mode {
        Paragraph::new(Span::styled(
            "(no folder selected)",
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
        frame.render_widget(
            Paragraph::new(Line::from(path_input_hint_spans(path_edit_mode))),
            c[3],
        );
    }

    // [5] password
    let masked = "•".repeat(password.len());
    let pass_display = if focus == 1 { format!("{masked}|") } else { masked };
    frame.render_widget(
        Paragraph::new(pass_display.as_str()).block(input_block("Master password", focus == 1)),
        c[5],
    );

    // [7-8] status
    if !is_opening {
        if let Some(err) = error {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    format!("✗  {err}"),
                    Style::default().fg(FAILURE),
                )),
                c[8],
            );
        }
    }

    // [10] hint
    let hint = if path_focused && path_edit_mode {
        "Esc cancel edit    Tab next field"
    } else {
        match focus {
            0 => "Esc back    Tab next field",
            _ => "Esc back    Tab previous field    Enter open vault",
        }
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM))).alignment(Alignment::Center),
        c[10],
    );
}

pub(super) fn draw_creating(
    frame: &mut Frame,
    vault_path: &str,
    blobs_dir: &str,
    password: &str,
    focus: usize,
    path_edit_mode: bool,
    error: Option<&str>,
) {
    let area = frame.area();
    frame.render_widget(outer_block("Vault — New"), area);

    let c = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1), // [0] info
            Constraint::Length(1), // [1] blank
            Constraint::Length(3), // [2] vault folder
            Constraint::Length(1), // [3] path hint
            Constraint::Length(1), // [4] blank
            Constraint::Length(3), // [5] blobs subfolder
            Constraint::Length(1), // [6] blobs hint
            Constraint::Length(1), // [7] blank
            Constraint::Length(3), // [8] password
            Constraint::Length(1), // [9] blank
            Constraint::Length(1), // [10] error
            Constraint::Min(0),    // [11] filler
            Constraint::Length(1), // [12] hint bar
        ])
        .split(area);

    // [0] info
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Create a new encrypted vault in an existing folder",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        ))),
        c[0],
    );

    // [2] vault folder
    let inner_w = c[2].width.saturating_sub(4) as usize;
    let path_display = if focus == 0 && path_edit_mode {
        tail_fit(&format!("{vault_path}|"), inner_w).to_string()
    } else {
        tail_fit(vault_path, inner_w).to_string()
    };
    let path_label = if focus == 0 && path_edit_mode {
        "Vault folder  (editing)"
    } else {
        "Vault folder"
    };
    frame.render_widget(
        Paragraph::new(path_display.as_str()).block(input_block(path_label, focus == 0)),
        c[2],
    );
    if focus == 0 {
        frame.render_widget(
            Paragraph::new(Line::from(path_input_hint_spans(path_edit_mode))),
            c[3],
        );
    }

    // [5] blobs subfolder
    let blobs_display = if focus == 1 { format!("{blobs_dir}|") } else { blobs_dir.to_string() };
    frame.render_widget(
        Paragraph::new(blobs_display.as_str())
            .block(input_block("Blobs subfolder (optional)", focus == 1)),
        c[5],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "  leave empty to store blobs alongside the index",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        )),
        c[6],
    );

    // [8] password
    let masked = "•".repeat(password.len());
    let pass_display = if focus == 2 { format!("{masked}|") } else { masked };
    frame.render_widget(
        Paragraph::new(pass_display.as_str()).block(input_block("Master password", focus == 2)),
        c[8],
    );

    // [10] error
    if let Some(err) = error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("✗  {err}"),
                Style::default().fg(FAILURE),
            )),
            c[10],
        );
    }

    // [12] hint bar
    let hint = match (focus, path_edit_mode) {
        (0, true)  => "Enter confirm    Esc cancel edit    Tab next field",
        (0, false) => "Esc back    Tab next field",
        (1, _)     => "Esc back    Tab next field    Enter skip to password",
        _          => "Esc back    Tab previous field    Enter create vault",
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM))).alignment(Alignment::Center),
        c[12],
    );
}

pub(super) fn draw_opening(frame: &mut Frame, pct: u8) {
    let area = frame.area();
    // Paint over the status rows only, using the same layout as draw_locked
    let c = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(1), // 0
            Constraint::Length(1), // 1
            Constraint::Length(3), // 2
            Constraint::Length(1), // 3
            Constraint::Length(1), // 4
            Constraint::Length(3), // 5
            Constraint::Length(1), // 6
            Constraint::Length(1), // 7
            Constraint::Length(1), // 8
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "  Unlocking vault…",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        )),
        c[7],
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(ACCENT).bg(DIM))
            .ratio(pct as f64 / 100.0)
            .label(format!("{pct}%")),
        c[8],
    );
}
