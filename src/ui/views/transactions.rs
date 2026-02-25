use ratatui::layout::{Constraint, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use crate::app::App;
use crate::domain::models::{format_centavos, TransactionKind};
use crate::ui::theme;

pub fn draw_transactions(frame: &mut Frame, app: &mut App) {
    let [filter_area, table_area, footer_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    draw_filter_bar(frame, app, filter_area);
    draw_table(frame, app, table_area);
    draw_footer(frame, footer_area);
}

fn draw_filter_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let filter = &app.filter;
    let mut parts: Vec<Span> = vec![Span::styled(" Filters: ", theme::header_style())];

    let mut has_filter = false;

    if let Some(ref search) = filter.search {
        parts.push(Span::styled(
            format!("search=\"{search}\" "),
            theme::warning_style(),
        ));
        has_filter = true;
    }
    if let Some(kind) = filter.kind {
        parts.push(Span::styled(
            format!("type={kind} "),
            theme::warning_style(),
        ));
        has_filter = true;
    }
    if let Some(tag_id) = filter.tag_id {
        let tag_name = app.tag_name(tag_id);
        parts.push(Span::styled(
            format!("tag={tag_name} "),
            theme::warning_style(),
        ));
        has_filter = true;
    }
    if let Some(d) = filter.date_from {
        parts.push(Span::styled(
            format!("from={d} "),
            theme::warning_style(),
        ));
        has_filter = true;
    }
    if let Some(d) = filter.date_to {
        parts.push(Span::styled(
            format!("to={d} "),
            theme::warning_style(),
        ));
        has_filter = true;
    }

    if !has_filter {
        parts.push(Span::styled("(none)", theme::muted_style()));
    }

    let line = Line::from(parts);
    let para = Paragraph::new(line);
    frame.render_widget(para, area);
}

fn draw_table(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let currency = &app.config.currency;
    let tsep = &app.config.thousands_separator;
    let dsep = &app.config.decimal_separator;
    let block = theme::styled_block(" Transactions ");

    let header = Row::new(vec!["Date", "Source", "Amount", "Type", "Tag"])
        .style(
            theme::header_style()
                .add_modifier(Modifier::UNDERLINED),
        )
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .transactions
        .iter()
        .map(|tx| {
            let tag_name = app.tag_name(tx.tag_id);
            let amount_style = match tx.kind {
                TransactionKind::Income => theme::income_style(),
                TransactionKind::Expense => theme::expense_style(),
            };
            let kind_str = match tx.kind {
                TransactionKind::Income => "INC",
                TransactionKind::Expense => "EXP",
            };

            Row::new(vec![
                Cell::from(tx.date.format("%Y-%m-%d").to_string()),
                Cell::from(tx.source.clone()),
                Cell::from(Span::styled(
                    format_centavos(tx.amount, currency, tsep, dsep),
                    amount_style,
                )),
                Cell::from(Span::styled(kind_str, amount_style)),
                Cell::from(tag_name),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(12),
        Constraint::Fill(1),
        Constraint::Length(16),
        Constraint::Length(5),
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
    if !app.transactions.is_empty() {
        state.select(Some(app.tx_selected));
    }

    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_footer(frame: &mut Frame, area: ratatui::layout::Rect) {
    let help = Line::from(vec![
        Span::styled(" [a]", theme::header_style()),
        Span::styled("dd ", theme::text_style()),
        Span::styled("[e]", theme::header_style()),
        Span::styled("dit ", theme::text_style()),
        Span::styled("[d]", theme::header_style()),
        Span::styled("elete ", theme::text_style()),
        Span::styled("[/]", theme::header_style()),
        Span::styled("filter ", theme::text_style()),
        Span::styled("[c]", theme::header_style()),
        Span::styled("lear filter ", theme::text_style()),
        Span::styled("[Esc]", theme::header_style()),
        Span::styled("back ", theme::text_style()),
    ]);
    frame.render_widget(Paragraph::new(help), area);
}
