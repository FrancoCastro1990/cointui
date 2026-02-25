use ratatui::layout::{Alignment, Constraint};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Bar, BarChart, BarGroup, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::App;
use crate::domain::models::{format_centavos, TransactionKind};
use crate::ui::theme;

pub fn draw_expense_chart(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = theme::styled_block(" Expenses by Tag (Top 5) ");

    // Aggregate expenses by tag.
    let mut tag_totals: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for tx in &app.transactions {
        if tx.kind == TransactionKind::Expense {
            *tag_totals.entry(tx.tag_id).or_insert(0) += tx.amount;
        }
    }

    let mut sorted: Vec<(i64, i64)> = tag_totals.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(5);

    let bars: Vec<Bar> = sorted
        .iter()
        .map(|(tag_id, amount)| {
            let name = app.tag_name(*tag_id);
            // Convert centavos to whole units for the bar chart.
            let value = (*amount / 100) as u64;
            Bar::default()
                .label(Line::from(name))
                .value(value)
                .style(theme::expense_style())
                .value_style(theme::text_style().add_modifier(Modifier::BOLD))
        })
        .collect();

    if bars.is_empty() {
        let para = Paragraph::new(Span::styled(
            "No expense data yet.",
            theme::muted_style(),
        ))
        .block(block)
        .alignment(Alignment::Center);
        frame.render_widget(para, area);
        return;
    }

    let bar_chart = BarChart::default()
        .block(block)
        .data(BarGroup::default().bars(&bars))
        .bar_width(8)
        .bar_gap(2)
        .style(theme::text_style());

    frame.render_widget(bar_chart, area);
}

pub fn draw_summary(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = theme::styled_block(" Summary ");
    let currency = &app.config.currency;
    let (total_income, total_expense) = app.totals;
    let balance = total_income - total_expense;
    let savings_rate = if total_income > 0 {
        ((total_income - total_expense) as f64 / total_income as f64) * 100.0
    } else {
        0.0
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Total Income:   ", theme::text_style()),
            Span::styled(
                format_centavos(total_income, currency),
                theme::income_style().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Total Expenses: ", theme::text_style()),
            Span::styled(
                format_centavos(total_expense, currency),
                theme::expense_style().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Balance:        ", theme::text_style()),
            Span::styled(
                format_centavos(balance, currency),
                if balance >= 0 {
                    theme::income_style()
                } else {
                    theme::expense_style()
                }
                .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Savings Rate:   ", theme::text_style()),
            Span::styled(
                format!("{:.1}%", savings_rate),
                if savings_rate >= 20.0 {
                    theme::income_style()
                } else if savings_rate >= 0.0 {
                    theme::warning_style()
                } else {
                    theme::expense_style()
                },
            ),
        ]),
    ];

    let para = Paragraph::new(lines).block(block);
    frame.render_widget(para, area);
}

pub fn draw_monthly_table(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = theme::styled_block(" Monthly Totals (last 6 months) ");
    let currency = &app.config.currency;

    let header = Row::new(vec!["Month", "Income", "Expenses", "Net"])
        .style(theme::header_style().add_modifier(Modifier::UNDERLINED))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .monthly_totals
        .iter()
        .map(|(month, income, expense)| {
            let net = income - expense;
            let net_style = if net >= 0 {
                theme::income_style()
            } else {
                theme::expense_style()
            };
            Row::new(vec![
                Cell::from(month.clone()),
                Cell::from(Span::styled(
                    format_centavos(*income, currency),
                    theme::income_style(),
                )),
                Cell::from(Span::styled(
                    format_centavos(*expense, currency),
                    theme::expense_style(),
                )),
                Cell::from(Span::styled(format_centavos(net, currency), net_style)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(10),
        Constraint::Length(16),
        Constraint::Length(16),
        Constraint::Length(16),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .style(theme::text_style())
        .column_spacing(2);

    frame.render_widget(table, area);
}

pub fn draw_stats_footer(frame: &mut Frame, area: ratatui::layout::Rect) {
    let help = Line::from(vec![
        Span::styled(" [Esc]", theme::header_style()),
        Span::styled("back ", theme::text_style()),
        Span::styled("[1-5]", theme::header_style()),
        Span::styled("switch view ", theme::text_style()),
    ]);
    frame.render_widget(Paragraph::new(help), area);
}
