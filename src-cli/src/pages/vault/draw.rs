//! Draw functions for the Vault page.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph},
};

use crate::{ACCENT, DIM, FAILURE, SUCCESS};
use crate::pages::widgets::{input_block, outer_block, tail_fit};
use super::state::{BrowseState, PanelFocus, Phase, VaultState};

pub(crate) fn draw_vault(frame: &mut Frame, state: &VaultState) {
    match &state.phase {
        Phase::VaultMenu { cursor } => draw_vault_menu(frame, *cursor),
        Phase::Locked { vault_path, password, focus, error } => {
            draw_locked(frame, vault_path, password, *focus, error.as_deref(), false)
        }
        Phase::Creating { vault_path, blobs_dir, password, focus, error } => {
            draw_creating(frame, vault_path, blobs_dir, password, *focus, error.as_deref())
        }
        Phase::ConfirmCreateDir { vault_path, .. } => {
            // Show the creating form dimmed behind the overlay.
            draw_creating(frame, vault_path, "", "", 0, None);
            draw_confirm_create_dir_overlay(frame, vault_path);
        }
        Phase::Opening(pct) => {
            draw_locked(frame, "", "", 1, None, true);
            // Re-draw the progress bar on top of the locked form
            draw_opening(frame, *pct);
        }
        Phase::Browse => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
            }
        }
        Phase::Rename { uuid, input } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
                draw_rename_overlay(frame, input, browse.entry(uuid).map(|e| e.name.as_str()).unwrap_or(""));
            }
        }
        Phase::ConfirmDelete { uuids } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
                draw_confirm_delete_overlay(frame, uuids.len());
            }
        }
        Phase::Move { uuids, tree_cursor } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
                draw_move_overlay(frame, browse, *tree_cursor, uuids.len());
            }
        }
        Phase::Adding { total, done, current_file } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
                draw_adding_overlay(frame, *total, *done, current_file);
            }
        }
    }
}

// ── Vault submenu ──────────────────────────────────────────────────────────

fn draw_vault_menu(frame: &mut Frame, cursor: usize) {
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

// ── Locked / Opening ───────────────────────────────────────────────────────

fn draw_locked(
    frame: &mut Frame,
    vault_path: &str,
    password: &str,
    focus: usize,
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
    let inner_w = c[2].width.saturating_sub(4) as usize;
    let path_display = {
        let s = if focus == 0 { format!("{vault_path}|") } else { vault_path.to_string() };
        tail_fit(&s, inner_w).to_string()
    };
    let path_label = if focus == 0 { "Vault folder  Enter→browse" } else { "Vault folder" };
    frame.render_widget(
        Paragraph::new(path_display.as_str()).block(input_block(path_label, focus == 0)),
        c[2],
    );

    if focus == 0 {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "  type a path, or press Enter to browse for the vault folder",
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            )),
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
    let hint = match focus {
        0 => "Esc back    Tab next field    Enter browse filesystem",
        _ => "Esc back    Tab previous field    Enter open vault",
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM))).alignment(Alignment::Center),
        c[10],
    );
}

fn draw_creating(
    frame: &mut Frame,
    vault_path: &str,
    blobs_dir: &str,
    password: &str,
    focus: usize,
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
    let path_display = {
        let s = if focus == 0 { format!("{vault_path}|") } else { vault_path.to_string() };
        tail_fit(&s, inner_w).to_string()
    };
    let path_label = if focus == 0 { "Vault folder  Enter→browse" } else { "Vault folder" };
    frame.render_widget(
        Paragraph::new(path_display.as_str()).block(input_block(path_label, focus == 0)),
        c[2],
    );
    if focus == 0 {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "  type a path, or press Enter to browse for the vault folder",
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            )),
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
    let hint = match focus {
        0 => "Esc back    Tab next field    Enter browse filesystem",
        1 => "Esc back    Tab next field    Enter skip to password",
        _ => "Esc back    Tab previous field    Enter create vault",
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(DIM))).alignment(Alignment::Center),
        c[12],
    );
}

fn draw_opening(frame: &mut Frame, pct: u8) {
    let area = frame.area();
    // Paint over the status rows only
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

// ── Browse ─────────────────────────────────────────────────────────────────

fn draw_browse(frame: &mut Frame, browse: &BrowseState, _overlay: Option<()>) {
    let area = frame.area();

    // Vault name from the root directory
    let vault_name = browse
        .handle
        .root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "vault".to_string());

    let dirty_indicator = if browse.dirty { "  [unsaved]" } else { "" };

    // Outer block
    let title = format!(" Vault — {vault_name}{dirty_indicator} ");
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            &title,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);
    frame.render_widget(&outer, area);
    let inner = outer.inner(area);

    // Vertical: hint bar (1) at bottom
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    // Horizontal split: tree 26% | list 74%
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(26), Constraint::Percentage(74)])
        .split(rows[0]);

    draw_tree_panel(frame, browse, cols[0]);
    draw_list_panel(frame, browse, cols[1]);
    draw_browse_hint(frame, browse, rows[1]);
}

fn draw_tree_panel(frame: &mut Frame, browse: &BrowseState, area: Rect) {
    let focused = browse.panel_focus == PanelFocus::Tree;
    let border_color = if focused { ACCENT } else { DIM };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(" Folders ", Style::default().fg(border_color)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = browse.all_folders.iter().enumerate().map(|(i, path)| {
        let depth = BrowseState::folder_depth(path);
        let name = BrowseState::folder_display_name(path);
        let indent = "  ".repeat(depth);
        let is_current = path == &browse.current_path;
        let is_cursor = focused && i == browse.tree_cursor;

        let prefix = if is_cursor { "▶ " } else { "  " };
        let text = format!("{indent}{prefix}{name}");
        let style = if is_current {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else if is_cursor {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(DIM)
        };
        ListItem::new(Span::styled(text, style))
    }).collect();

    // Scroll tree to keep tree_cursor visible
    let mut list_state = ListState::default();
    if focused {
        list_state.select(Some(browse.tree_cursor));
    } else {
        // Show the current_path row highlighted but don't move scroll
        let pos = browse.all_folders.iter().position(|f| f == &browse.current_path);
        list_state.select(pos);
    }

    frame.render_stateful_widget(List::new(items), inner, &mut list_state);
}

fn draw_list_panel(frame: &mut Frame, browse: &BrowseState, area: Rect) {
    let focused = browse.panel_focus == PanelFocus::List;
    let border_color = if focused { ACCENT } else { DIM };

    let path_title = if browse.current_path.is_empty() {
        " / ".to_string()
    } else {
        format!(" /{} ", browse.current_path)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(path_title, Style::default().fg(border_color)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if browse.list_count() == 0 {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "(empty folder)",
                Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
            )),
            inner,
        );
        return;
    }

    // Build items: folders first, then files
    let mut items: Vec<ListItem> = Vec::new();

    for folder in &browse.folders {
        items.push(ListItem::new(Line::from(vec![
            Span::styled("  [/]  ", Style::default().fg(DIM)),
            Span::styled(format!("{folder}/"), Style::default().fg(Color::Cyan)),
        ])));
    }

    for uuid in &browse.file_uuids {
        let Some(entry) = browse.entry(uuid) else { continue };
        let prefix = if browse.clipboard.contains(uuid) {
            Span::styled("  [✂]  ", Style::default().fg(Color::Yellow))
        } else if browse.selected_uuids.contains(uuid.as_str()) {
            Span::styled("  [x]  ", Style::default().fg(ACCENT))
        } else {
            Span::styled("       ", Style::default().fg(DIM))
        };
        let size_str = human_size(entry.size);
        // Right-align size in a fixed width of 9 chars
        let name_and_size = format!("{:<w$}  {:>9}", entry.name, size_str,
            w = (inner.width as usize).saturating_sub(18));
        items.push(ListItem::new(Line::from(vec![
            prefix,
            Span::styled(name_and_size, Style::default().fg(Color::White)),
        ])));
    }

    let highlight = Style::default()
        .fg(Color::Black)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD);

    let mut list_state = ListState::default();
    if focused || browse.list_count() > 0 {
        list_state.select(Some(browse.list_cursor));
    }

    frame.render_stateful_widget(
        List::new(items).highlight_style(highlight).highlight_symbol("▶ "),
        inner,
        &mut list_state,
    );
}

fn draw_browse_hint(frame: &mut Frame, browse: &BrowseState, area: Rect) {
    // Show status_msg if present, otherwise show context-sensitive keybindings
    let (text, color) = if let Some(msg) = &browse.status_msg {
        (msg.as_str(), Color::White)
    } else if browse.panel_focus == PanelFocus::Tree {
        ("Tab list    ↑↓/jk navigate    Enter select folder    h/Esc up / back", DIM)
    } else {
        let clip_hint = if !browse.clipboard.is_empty() { "  p paste" } else { "" };
        // Build hint string without format!() owning a temporary
        let _ = clip_hint;
        ("Tab tree    ↑↓/jk navigate    Enter open    Space select    i add    r rename    d delete    x cut    p paste    m move    s save    h/Esc up", DIM)
    };

    let line = Span::styled(text, Style::default().fg(color));
    frame.render_widget(
        Paragraph::new(line).alignment(Alignment::Center),
        area,
    );
}

// ── Overlays ───────────────────────────────────────────────────────────────

fn centered_popup(area: Rect, percent_w: u16, height: u16) -> Rect {
    let w = (area.width * percent_w / 100).max(20);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect { x, y, width: w, height: h }
}

fn draw_rename_overlay(frame: &mut Frame, input: &str, original: &str) {
    let area = centered_popup(frame.area(), 60, 7);
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

fn draw_confirm_create_dir_overlay(frame: &mut Frame, vault_path: &str) {
    let area = centered_popup(frame.area(), 62, 7);
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

fn draw_confirm_delete_overlay(frame: &mut Frame, count: usize) {
    let area = centered_popup(frame.area(), 50, 6);
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
            format!("Delete {count} {noun}? This removes them from the index."),
            Style::default().fg(Color::White),
        )).alignment(Alignment::Center),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Blob files are NOT deleted from disk.",
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

fn draw_move_overlay(frame: &mut Frame, browse: &BrowseState, tree_cursor: usize, count: usize) {
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

    // Folder list
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

fn draw_adding_overlay(frame: &mut Frame, total: usize, done: usize, current_file: &str) {
    let area = centered_popup(frame.area(), 60, 7);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            " Adding Files ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // current filename
            Constraint::Length(1), // blank
            Constraint::Length(1), // progress gauge
            Constraint::Length(1), // count label
        ])
        .split(inner);

    // Current filename (truncated to fit)
    let fname_w = rows[0].width as usize;
    let fname_display = if current_file.len() > fname_w {
        format!("…{}", &current_file[current_file.len().saturating_sub(fname_w.saturating_sub(1))..])
    } else {
        current_file.to_string()
    };
    frame.render_widget(
        Paragraph::new(Span::styled(fname_display, Style::default().fg(Color::White)))
            .alignment(Alignment::Center),
        rows[0],
    );

    // Progress gauge
    let ratio = if total == 0 { 0.0 } else { done as f64 / total as f64 };
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(ACCENT).bg(DIM))
            .ratio(ratio)
            .label(format!("{done} / {total}")),
        rows[2],
    );

    // Count label
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Encrypting… please wait",
            Style::default().fg(DIM).add_modifier(Modifier::ITALIC),
        ))
        .alignment(Alignment::Center),
        rows[3],
    );
}

// ── Utilities ──────────────────────────────────────────────────────────────

fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB      { format!("{:.1} GB", bytes as f64 / GB as f64) }
    else if bytes >= MB { format!("{:.1} MB", bytes as f64 / MB as f64) }
    else if bytes >= KB { format!("{:.1} KB", bytes as f64 / KB as f64) }
    else                { format!("{bytes} B") }
}
