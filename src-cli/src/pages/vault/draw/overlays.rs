//! Draw functions for all vault overlay popups.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph},
};

use crate::{ACCENT, DIM, FAILURE};
use crate::pages::widgets::input_block;
use crate::pages::vault::state::BrowseState;
use super::util::centered_popup;

/// Overlay: rename a single vault entry.
pub(super) fn draw_rename_overlay(frame: &mut Frame, input: &str, original: &str) {
    let area = centered_popup(frame.area(), 60, 10);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            " Rename ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // original name
            Constraint::Length(1), // blank
            Constraint::Length(3), // input
            Constraint::Length(1), // hint
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Renaming: ", Style::default().fg(DIM)),
            Span::styled(original, Style::default().fg(Color::White)),
        ])),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(format!("{input}|")).block(input_block("New name", true)),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Enter confirm    Esc cancel",
            Style::default().fg(DIM),
        )).alignment(Alignment::Center),
        rows[3],
    );
}

/// Overlay: confirm creating a directory that does not yet exist.
pub(super) fn draw_confirm_create_dir_overlay(frame: &mut Frame, vault_path: &str) {
    let area = centered_popup(frame.area(), 62, 8);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            " Directory not found ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(
            "The following directory does not exist:",
            Style::default().fg(DIM),
        ))
        .alignment(Alignment::Center),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(vault_path, Style::default().fg(Color::White)))
            .alignment(Alignment::Center),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Create it and continue?",
            Style::default().fg(Color::White),
        ))
        .alignment(Alignment::Center),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "y / Enter  yes    n / Esc  no",
            Style::default().fg(DIM),
        ))
        .alignment(Alignment::Center),
        rows[3],
    );
}

/// Overlay: confirm permanent deletion of `count` items.
pub(super) fn draw_confirm_delete_overlay(frame: &mut Frame, count: usize) {
    let area = centered_popup(frame.area(), 50, 7);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(FAILURE))
        .title(Span::styled(
            " Confirm Delete ",
            Style::default().fg(FAILURE).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    let noun = if count == 1 { "item" } else { "items" };
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("Delete {count} {noun}? This cannot be undone."),
            Style::default().fg(Color::White),
        )).alignment(Alignment::Center),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Index entry and blob files will be removed from disk.",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        )).alignment(Alignment::Center),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "y / Enter  confirm    n / Esc  cancel",
            Style::default().fg(DIM),
        )).alignment(Alignment::Center),
        rows[2],
    );
}

/// Overlay: move-destination folder picker.
pub(super) fn draw_move_overlay(frame: &mut Frame, browse: &BrowseState, tree_cursor: usize, count: usize) {
    let area = centered_popup(frame.area(), 55, 16);
    frame.render_widget(Clear, area);

    let noun = if count == 1 { "item" } else { "items" };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            format!(" Move {count} {noun} — select destination "),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let items: Vec<ListItem> = browse.all_folders.iter().map(|path| {
        let depth = BrowseState::folder_depth(path);
        let name = BrowseState::folder_display_name(path);
        let indent = "  ".repeat(depth);
        ListItem::new(Span::raw(format!("{indent}{name}")))
    }).collect();

    let mut list_state = ListState::default();
    list_state.select(Some(tree_cursor));

    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ "),
        rows[0],
        &mut list_state,
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            "↑↓/jk navigate    Enter move here    Esc cancel",
            Style::default().fg(DIM),
        )).alignment(Alignment::Center),
        rows[1],
    );
}

/// Overlay: create a new folder inside the current path.
pub(super) fn draw_new_folder_overlay(frame: &mut Frame, parent: &str, input: &str, error: Option<&str>) {
    let area = centered_popup(frame.area(), 60, 10);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            " New Folder ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // parent path label
            Constraint::Length(1), // blank
            Constraint::Length(3), // name input
            Constraint::Length(1), // error or hint
        ])
        .split(inner);

    let parent_label = if parent.is_empty() { "/".to_string() } else { format!("/{parent}/") };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Inside: ", Style::default().fg(DIM)),
            Span::styled(parent_label, Style::default().fg(Color::White)),
        ])),
        rows[0],
    );

    frame.render_widget(
        Paragraph::new(format!("{input}|")).block(input_block("Folder name", true)),
        rows[2],
    );

    if let Some(err) = error {
        frame.render_widget(
            Paragraph::new(Span::styled(err, Style::default().fg(FAILURE))),
            rows[3],
        );
    } else {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "Enter confirm    Esc cancel",
                Style::default().fg(DIM),
            ))
            .alignment(Alignment::Center),
            rows[3],
        );
    }
}

/// Overlay: background file-decrypt in progress.
pub(super) fn draw_previewing_overlay(frame: &mut Frame, filename: &str) {
    // height = 2 rows content + 2 borders + 2 margin = 6; use 7 for comfort
    let area = centered_popup(frame.area(), 55, 7);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            " Decrypting ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // filename
            Constraint::Length(1), // label
        ])
        .split(inner);

    let w = rows[0].width as usize;
    let display = if filename.len() > w {
        format!("…{}", &filename[filename.len().saturating_sub(w.saturating_sub(1))..])
    } else {
        filename.to_string()
    };
    frame.render_widget(
        Paragraph::new(Span::styled(display, Style::default().fg(Color::White)))
            .alignment(Alignment::Center),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Decrypting… please wait",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        ))
        .alignment(Alignment::Center),
        rows[1],
    );
}

/// Overlay: background file-add (encrypt) operation in progress.
pub(super) fn draw_adding_overlay(frame: &mut Frame, total: usize, done: usize, current_file: &str) {
    draw_progress_overlay(
        frame,
        " Adding Files ",
        current_file,
        done,
        total,
        "Encrypting… please wait",
    );
}

/// Overlay: background file-export (decrypt-to-disk) in progress.
pub(super) fn draw_exporting_overlay(frame: &mut Frame, total: usize, done: usize, current_file: &str) {
    draw_progress_overlay(
        frame,
        " Exporting Files ",
        current_file,
        done,
        total,
        "Decrypting… please wait",
    );
}

/// Overlay: background gallery image-decrypt in progress.
pub(super) fn draw_loading_gallery_overlay(frame: &mut Frame, folder: &str, done: usize, total: usize) {
    let label = if folder.is_empty() { "/" } else { folder };
    draw_progress_overlay(
        frame,
        " Loading Gallery ",
        label,
        done,
        total,
        "Decrypting images… please wait",
    );
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Shared progress-overlay template: title, current item, gauge, status line.
fn draw_progress_overlay(
    frame: &mut Frame,
    title: &str,
    current_item: &str,
    done: usize,
    total: usize,
    status_text: &str,
) {
    let area = centered_popup(frame.area(), 60, 8);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            title,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // current item
            Constraint::Length(1), // blank
            Constraint::Length(1), // progress gauge
            Constraint::Length(1), // status label
        ])
        .split(inner);

    // Current item (truncated to fit)
    let item_w = rows[0].width as usize;
    let item_display = if current_item.len() > item_w {
        format!("…{}", &current_item[current_item.len().saturating_sub(item_w.saturating_sub(1))..])
    } else {
        current_item.to_string()
    };
    frame.render_widget(
        Paragraph::new(Span::styled(item_display, Style::default().fg(Color::White)))
            .alignment(Alignment::Center),
        rows[0],
    );

    let ratio = if total == 0 { 0.0 } else { done as f64 / total as f64 };
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(ACCENT).bg(DIM))
            .ratio(ratio)
            .label(format!("{done} / {total}")),
        rows[2],
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            status_text,
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        ))
        .alignment(Alignment::Center),
        rows[3],
    );
}
