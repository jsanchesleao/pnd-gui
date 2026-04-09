//! Drawing utilities: layout helpers, size formatting, shared span builders.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;

use crate::{ACCENT, DIM};
use crate::yazi::yazi_available;

/// Return a centered `Rect` of given percentage-width and fixed height.
pub(super) fn centered_popup(area: Rect, percent_w: u16, height: u16) -> Rect {
    let w = (area.width * percent_w / 100).max(20);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect { x, y, width: w, height: h }
}

/// Format a byte count as a human-readable string ("1.2 GB", "34.5 KB", etc.).
pub(super) fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB      { format!("{:.1} GB", bytes as f64 / GB as f64) }
    else if bytes >= MB { format!("{:.1} MB", bytes as f64 / MB as f64) }
    else if bytes >= KB { format!("{:.1} KB", bytes as f64 / KB as f64) }
    else                { format!("{bytes} B") }
}

/// Build the sub-hint spans shown below the vault-path input field.
///
/// In edit mode shows Enter/Esc/Tab hints; in display mode shows the
/// `t type  b browser  [y yazi]  Enter auto-pick` picker hints.
pub(super) fn path_input_hint_spans(edit_mode: bool) -> Vec<Span<'static>> {
    if edit_mode {
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
    }
}
