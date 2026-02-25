use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::View;
use crate::ui::theme;
use crate::ui::views::form::centered_rect;

pub fn draw_help(frame: &mut Frame, view: View) {
    let area = centered_rect(55, 70, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::bordered()
        .title(" Keybindings ")
        .title_style(theme::header_style())
        .border_style(ratatui::style::Style::default().fg(theme::ACCENT))
        .style(ratatui::style::Style::default().bg(theme::BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    // Global keys
    lines.push(Line::from(Span::styled(
        "Global",
        theme::header_style(),
    )));
    for (key, desc) in global_keys() {
        lines.push(key_line(key, desc));
    }
    lines.push(Line::from(""));

    // View-specific keys
    let (title, keys) = view_keys(view);
    lines.push(Line::from(Span::styled(title, theme::header_style())));
    for (key, desc) in keys {
        lines.push(key_line(key, desc));
    }
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "Press any key to close",
        theme::muted_style(),
    )));

    let para = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(para, inner);
}

fn key_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {key:<14}"), theme::warning_style()),
        Span::styled(desc, theme::text_style()),
    ])
}

fn global_keys() -> Vec<(&'static str, &'static str)> {
    vec![
        ("1-6", "Switch view"),
        ("Tab", "Next view"),
        ("Shift+Tab", "Previous view"),
        ("?", "Show this help"),
        ("q", "Quit"),
        ("Ctrl+C", "Force quit"),
        ("Esc", "Back to Dashboard"),
    ]
}

fn view_keys(view: View) -> (&'static str, Vec<(&'static str, &'static str)>) {
    match view {
        View::Dashboard => ("Dashboard", vec![
            ("(none)", "Navigate with global keys"),
        ]),
        View::Transactions => ("Transactions", vec![
            ("j/Down", "Move down"),
            ("k/Up", "Move up"),
            ("a", "Add transaction"),
            ("e", "Edit selected"),
            ("d", "Delete selected"),
            ("/", "Open filter form"),
            ("c", "Clear all filters"),
            ("s", "Cycle sort column"),
            ("S", "Toggle sort direction"),
        ]),
        View::Stats => ("Stats", vec![
            ("(none)", "View-only statistics"),
        ]),
        View::Budgets => ("Budgets", vec![
            ("j/Down", "Move down"),
            ("k/Up", "Move up"),
            ("a", "Add budget"),
            ("d", "Delete selected"),
        ]),
        View::Recurring => ("Recurring", vec![
            ("j/Down", "Move down"),
            ("k/Up", "Move up"),
            ("Space", "Toggle active/inactive"),
            ("d", "Delete selected"),
        ]),
        View::Tags => ("Tags", vec![
            ("j/Down", "Move down"),
            ("k/Up", "Move up"),
            ("a", "Add tag"),
            ("e", "Edit selected"),
            ("d", "Delete selected"),
        ]),
    }
}
