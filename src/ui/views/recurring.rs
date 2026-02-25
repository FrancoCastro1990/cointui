use ratatui::layout::{Constraint, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use crate::app::App;
use crate::domain::models::{format_centavos, TransactionKind};
use crate::ui::theme;

pub fn draw_recurring(frame: &mut Frame, app: &mut App) {
    let [table_area, footer_area] = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    draw_table(frame, app, table_area);
    draw_footer(frame, footer_area);
}

fn draw_table(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let currency = &app.config.currency;
    let tsep = &app.config.thousands_separator;
    let dsep = &app.config.decimal_separator;
    let block = theme::styled_block(" Recurring Entries ");

    if app.recurring_entries.is_empty() {
        let para = Paragraph::new(Span::styled(
            "  No recurring entries. Add a transaction with recurring enabled.",
            theme::muted_style(),
        ))
        .block(block);
        frame.render_widget(para, area);
        return;
    }

    let header = Row::new(vec!["Status", "Source", "Amount", "Type", "Interval", "Tag"])
        .style(
            theme::header_style()
                .add_modifier(Modifier::UNDERLINED),
        )
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .recurring_entries
        .iter()
        .map(|entry| {
            let status = if entry.active { "[ON] " } else { "[OFF]" };
            let status_style = if entry.active {
                theme::income_style()
            } else {
                theme::muted_style()
            };
            let amount_style = match entry.kind {
                TransactionKind::Income => theme::income_style(),
                TransactionKind::Expense => theme::expense_style(),
            };
            let kind_str = match entry.kind {
                TransactionKind::Income => "INC",
                TransactionKind::Expense => "EXP",
            };
            let tag_name = app.tag_name(entry.tag_id);

            Row::new(vec![
                Cell::from(Span::styled(status, status_style)),
                Cell::from(entry.source.clone()),
                Cell::from(Span::styled(
                    format_centavos(entry.amount, currency, tsep, dsep),
                    amount_style,
                )),
                Cell::from(Span::styled(kind_str, amount_style)),
                Cell::from(entry.interval.to_string()),
                Cell::from(tag_name),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(6),
        Constraint::Fill(1),
        Constraint::Length(16),
        Constraint::Length(5),
        Constraint::Length(10),
        Constraint::Length(16),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .style(theme::text_style())
        .column_spacing(1)
        .row_highlight_style(theme::selected_style())
        .highlight_symbol("> ");

    let mut state = TableState::default();
    if !app.recurring_entries.is_empty() {
        state.select(Some(app.recurring_selected));
    }

    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_footer(frame: &mut Frame, area: ratatui::layout::Rect) {
    let help = Line::from(vec![
        Span::styled(" [Space]", theme::header_style()),
        Span::styled("toggle ", theme::text_style()),
        Span::styled("[d]", theme::header_style()),
        Span::styled("elete ", theme::text_style()),
        Span::styled("[Up/Down]", theme::header_style()),
        Span::styled("select ", theme::text_style()),
        Span::styled("[Esc]", theme::header_style()),
        Span::styled("back ", theme::text_style()),
    ]);
    frame.render_widget(Paragraph::new(help), area);
}
