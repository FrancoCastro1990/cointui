use ratatui::style::{Color, Modifier, Style};

// Tokyo Night-inspired dark color palette.
pub const BG: Color = Color::Rgb(26, 27, 38);
pub const FG: Color = Color::Rgb(192, 202, 245);
pub const ACCENT: Color = Color::Rgb(122, 162, 247);
pub const GREEN: Color = Color::Rgb(158, 206, 106);
pub const RED: Color = Color::Rgb(247, 118, 142);
pub const YELLOW: Color = Color::Rgb(224, 175, 104);
pub const MUTED: Color = Color::Rgb(86, 95, 137);
pub const BORDER: Color = Color::Rgb(59, 66, 97);
pub const HIGHLIGHT_BG: Color = Color::Rgb(40, 52, 87);

/// Style for major header/title text.
pub fn header_style() -> Style {
    Style::default()
        .fg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// Style for income amounts.
pub fn income_style() -> Style {
    Style::default().fg(GREEN)
}

/// Style for expense amounts.
pub fn expense_style() -> Style {
    Style::default().fg(RED)
}

/// Style for the currently selected row/item.
pub fn selected_style() -> Style {
    Style::default()
        .bg(HIGHLIGHT_BG)
        .add_modifier(Modifier::BOLD)
}

/// Style for muted/secondary text.
pub fn muted_style() -> Style {
    Style::default().fg(MUTED)
}

/// Style for warning-level information.
pub fn warning_style() -> Style {
    Style::default().fg(YELLOW)
}

/// Style for block borders.
pub fn border_style() -> Style {
    Style::default().fg(BORDER)
}

/// Default text style.
pub fn text_style() -> Style {
    Style::default().fg(FG)
}

/// Style for focused form field labels.
pub fn focused_field_style() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

/// Style for unfocused form field labels.
pub fn unfocused_field_style() -> Style {
    Style::default().fg(FG)
}

/// Base block with consistent border style.
pub fn styled_block(title: &str) -> ratatui::widgets::Block<'_> {
    ratatui::widgets::Block::bordered()
        .title(title)
        .border_style(border_style())
        .title_style(header_style())
}
