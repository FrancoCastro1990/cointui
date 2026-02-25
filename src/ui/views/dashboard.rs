use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::App;
use crate::domain::models::{format_centavos, TransactionKind};
use crate::ui::theme;

pub fn draw_dashboard_header(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let [income_area, balance_area, expense_area] =
        Layout::horizontal([Constraint::Ratio(1, 3); 3]).areas(area);

    let currency = &app.config.currency;
    let tsep = &app.config.thousands_separator;
    let dsep = &app.config.decimal_separator;
    let (total_income, total_expense) = app.totals;
    let balance = total_income - total_expense;

    // Income panel
    let income_block = Block::bordered()
        .title(" INCOME ")
        .title_style(theme::income_style().add_modifier(Modifier::BOLD))
        .border_style(Style::default().fg(theme::GREEN));
    let income_text = Paragraph::new(Line::from(Span::styled(
        format_centavos(total_income, currency, tsep, dsep),
        theme::income_style().add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(income_block);
    frame.render_widget(income_text, income_area);

    // Balance panel
    let balance_style = if balance >= 0 {
        theme::income_style()
    } else {
        theme::expense_style()
    };
    let balance_block = Block::bordered()
        .title(" BALANCE ")
        .title_style(theme::header_style())
        .border_style(Style::default().fg(theme::ACCENT));
    let balance_text = Paragraph::new(Line::from(Span::styled(
        format_centavos(balance, currency, tsep, dsep),
        balance_style.add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(balance_block);
    frame.render_widget(balance_text, balance_area);

    // Expense panel
    let expense_block = Block::bordered()
        .title(" EXPENSES ")
        .title_style(theme::expense_style().add_modifier(Modifier::BOLD))
        .border_style(Style::default().fg(theme::RED));
    let expense_text = Paragraph::new(Line::from(Span::styled(
        format_centavos(total_expense, currency, tsep, dsep),
        theme::expense_style().add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(expense_block);
    frame.render_widget(expense_text, expense_area);
}

pub fn draw_dashboard_recent(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let currency = &app.config.currency;
    let tsep = &app.config.thousands_separator;
    let dsep = &app.config.decimal_separator;
    let block = theme::styled_block(" Recent Transactions ");

    let header = Row::new(vec!["Date", "Source", "Amount", "Type", "Tag"])
        .style(
            theme::header_style()
                .add_modifier(Modifier::UNDERLINED),
        )
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .transactions
        .iter()
        .take(10)
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
        .column_spacing(1);

    frame.render_widget(table, area);
}

pub fn draw_dashboard_alerts(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = theme::styled_block(" Budget Alerts ");
    let currency = &app.config.currency;
    let tsep = &app.config.thousands_separator;
    let dsep = &app.config.decimal_separator;

    let mut alerts: Vec<Line> = Vec::new();

    for (budget, spent) in &app.budget_spending {
        let limit = budget.amount;
        if limit == 0 {
            continue;
        }
        let pct = (*spent as f64 / limit as f64) * 100.0;
        if pct >= 80.0 {
            let tag_name = match budget.tag_id {
                Some(tid) => app.tag_name(tid),
                None => "Global".to_string(),
            };
            let style = if pct >= 100.0 {
                theme::expense_style()
            } else {
                theme::warning_style()
            };
            let msg = format!(
                "{}: {} / {} ({:.0}%) - {}",
                tag_name,
                format_centavos(*spent, currency, tsep, dsep),
                format_centavos(limit, currency, tsep, dsep),
                pct,
                budget.period,
            );
            alerts.push(Line::from(Span::styled(msg, style)));
        }
    }

    if alerts.is_empty() {
        alerts.push(Line::from(Span::styled(
            "No budget alerts.",
            theme::muted_style(),
        )));
    }

    let para = Paragraph::new(alerts).block(block);
    frame.render_widget(para, area);
}

pub fn draw_dashboard_footer(frame: &mut Frame, area: ratatui::layout::Rect) {
    let help = Line::from(vec![
        Span::styled(" [1]", theme::header_style()),
        Span::styled("Dashboard ", theme::text_style()),
        Span::styled("[2]", theme::header_style()),
        Span::styled("Transactions ", theme::text_style()),
        Span::styled("[3]", theme::header_style()),
        Span::styled("Stats ", theme::text_style()),
        Span::styled("[4]", theme::header_style()),
        Span::styled("Budgets ", theme::text_style()),
        Span::styled("[5]", theme::header_style()),
        Span::styled("Recurring ", theme::text_style()),
        Span::styled("[q]", theme::expense_style()),
        Span::styled("Quit", theme::text_style()),
    ]);
    frame.render_widget(Paragraph::new(help), area);
}
