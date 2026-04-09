//! Draw functions for the Vault page.

#[path = "draw/util.rs"]
mod util;

#[path = "draw/forms.rs"]
mod forms;

#[path = "draw/overlays.rs"]
mod overlays;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};

use crate::{ACCENT, DIM};
use super::state::{BrowseState, PanelFocus, Phase, VaultState};
use util::human_size;

pub(crate) fn draw_vault(frame: &mut Frame, state: &VaultState) {
    match &state.phase {
        Phase::VaultMenu { cursor } => forms::draw_vault_menu(frame, *cursor),
        Phase::Locked { vault_path, password, focus, path_edit_mode, error } => {
            forms::draw_locked(frame, vault_path, password, *focus, *path_edit_mode, error.as_deref(), false)
        }
        Phase::Creating { vault_path, blobs_dir, password, focus, path_edit_mode, error } => {
            forms::draw_creating(frame, vault_path, blobs_dir, password, *focus, *path_edit_mode, error.as_deref())
        }
        Phase::ConfirmCreateDir { vault_path, .. } => {
            // Show the creating form dimmed behind the overlay.
            forms::draw_creating(frame, vault_path, "", "", 0, false, None);
            overlays::draw_confirm_create_dir_overlay(frame, vault_path);
        }
        Phase::Opening(pct) => {
            forms::draw_locked(frame, "", "", 1, false, None, true);
            // Re-draw the progress bar on top of the locked form
            forms::draw_opening(frame, *pct);
        }
        Phase::Browse => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
            }
        }
        Phase::Rename { uuid, input } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
                overlays::draw_rename_overlay(frame, input, browse.entry(uuid).map(|e| e.name.as_str()).unwrap_or(""));
            }
        }
        Phase::ConfirmDelete { uuids } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
                overlays::draw_confirm_delete_overlay(frame, uuids.len());
            }
        }
        Phase::Move { uuids, tree_cursor } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
                overlays::draw_move_overlay(frame, browse, *tree_cursor, uuids.len());
            }
        }
        Phase::Adding { total, done, current_file } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
                overlays::draw_adding_overlay(frame, *total, *done, current_file);
            }
        }
        Phase::NewFolder { parent, input, error } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
                overlays::draw_new_folder_overlay(frame, parent, input, error.as_deref());
            }
        }
        Phase::Previewing { filename } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
                overlays::draw_previewing_overlay(frame, filename);
            }
        }
        // PreviewReady is transient: main loop calls render_vault_preview before draw,
        // so by the time draw runs the phase is already back to Browse.
        Phase::PreviewReady { .. } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
            }
        }
        Phase::Exporting { total, done, current_file } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
                overlays::draw_exporting_overlay(frame, *total, *done, current_file);
            }
        }
        Phase::LoadingGallery { folder, done, total } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
                overlays::draw_loading_gallery_overlay(frame, folder, *done, *total);
            }
        }
        // GalleryReady is transient: main loop calls render_vault_gallery before draw,
        // so by the time draw runs the phase is already back to Browse.
        Phase::GalleryReady { .. } => {
            if let Some(browse) = &state.browse {
                draw_browse(frame, browse, None);
            }
        }
    }
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

    let sort_label = browse.sort_key.label();
    let sort_arrow = browse.sort_dir.arrow();
    let path_title = if browse.current_path.is_empty() {
        format!(" / [{sort_label} {sort_arrow}] ")
    } else {
        format!(" /{} [{sort_label} {sort_arrow}] ", browse.current_path)
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
        items.push(ListItem::new(ratatui::text::Line::from(vec![
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
        items.push(ListItem::new(ratatui::text::Line::from(vec![
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
        ("Tab tree    ↑↓/jk navigate    Enter open/preview    Space select    o sort    O dir    g gallery    G cur gallery    i add    e export    n folder    r rename    d del    x cut    p paste    m move    s save    h/Esc up", DIM)
    };

    let line = Span::styled(text, Style::default().fg(color));
    frame.render_widget(
        Paragraph::new(line).alignment(Alignment::Center),
        area,
    );
}
