//! Shared drawing primitives reused across all pages.

use ratatui::{
    layout::Alignment,
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders},
};

use crate::{ACCENT, DIM};

/// Standard page-border block: accent-coloured rounded border, bold centred title.
pub(crate) fn outer_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
}

/// Text-input block whose border and title colour shift to accent when focused.
pub(crate) fn input_block(label: &str, focused: bool) -> Block<'_> {
    let color = if focused { ACCENT } else { DIM };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .title(Span::styled(format!(" {label} "), Style::default().fg(color)))
}

/// Return the rightmost `cols` bytes of `s`, keeping the cursor end visible
/// as the user types. Byte-level — safe for ASCII paths; clips gracefully for Unicode.
pub(crate) fn tail_fit(s: &str, cols: usize) -> &str {
    if s.len() <= cols { s } else { &s[s.len() - cols..] }
}
