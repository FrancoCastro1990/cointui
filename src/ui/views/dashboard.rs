use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::App;
use crate::domain::models::{format_cents, TransactionKind};
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
        format_cents(total_income, currency, tsep, dsep),
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
        format_cents(balance, currency, tsep, dsep),
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
        format_cents(total_expense, currency, tsep, dsep),
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
        .dashboard_transactions
        .iter()
        .map(|tx| {
            let tag_name = app.tag_name(tx.tag_id);
            let amount_style = match tx.kind {
                TransactionKind::Income => theme::income_style(),
                TransactionKind::Expense => theme::expense_style(),
            };
            let is_recurring = tx.notes.as_ref().is_some_and(|n| n.starts_with("Auto: recurring"));
            let kind_str = match tx.kind {
                TransactionKind::Income => if is_recurring { "INC↻" } else { "INC" },
                TransactionKind::Expense => if is_recurring { "EXP↻" } else { "EXP" },
            };

            Row::new(vec![
                Cell::from(tx.date.format("%Y-%m-%d").to_string()),
                Cell::from(tx.source.clone()),
                Cell::from(Span::styled(
                    format_cents(tx.amount, currency, tsep, dsep),
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
        Constraint::Length(6),
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
        let tag_name = match budget.tag_id {
            Some(tid) => app.tag_name(tid),
            None => "Global".to_string(),
        };
        let style = if pct >= 100.0 {
            theme::expense_style()
        } else if pct >= 60.0 {
            theme::warning_style()
        } else {
            theme::income_style()
        };
        let msg = format!(
            "{}: {} / {} ({:.0}%) - {}",
            tag_name,
            format_cents(*spent, currency, tsep, dsep),
            format_cents(limit, currency, tsep, dsep),
            pct,
            budget.period,
        );
        alerts.push(Line::from(Span::styled(msg, style)));
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

pub fn draw_dashboard_recurring(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = theme::styled_block(" Recurring ");
    let currency = &app.config.currency;
    let tsep = &app.config.thousands_separator;
    let dsep = &app.config.decimal_separator;

    if app.recurring_entries.is_empty() {
        let para = Paragraph::new(Span::styled(
            "No recurring entries.",
            theme::muted_style(),
        ))
        .block(block);
        frame.render_widget(para, area);
        return;
    }

    let lines: Vec<Line> = app
        .recurring_entries
        .iter()
        .map(|entry| {
            let status = if entry.active { "[ON]" } else { "[OFF]" };
            let status_style = if entry.active {
                theme::income_style()
            } else {
                theme::muted_style()
            };
            let tag_name = app.tag_name(entry.tag_id);
            Line::from(vec![
                Span::styled(status, status_style),
                Span::raw(" "),
                Span::styled(&entry.source, theme::text_style()),
                Span::raw("  "),
                Span::styled(
                    format_cents(entry.amount, currency, tsep, dsep),
                    theme::text_style(),
                ),
                Span::raw("  "),
                Span::styled(entry.interval.to_string(), theme::muted_style()),
                Span::raw("  "),
                Span::styled(tag_name, theme::muted_style()),
            ])
        })
        .collect();

    let para = Paragraph::new(lines).block(block);
    frame.render_widget(para, area);
}

pub fn draw_dashboard_spending(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let [month_area, year_area] =
        Layout::vertical([Constraint::Ratio(1, 2); 2]).areas(area);

    let now = chrono::Local::now();
    let month_title = format!(" Spending ({}) ", now.format("%b %Y"));
    let year_title = format!(" Spending ({}) ", now.format("%Y"));

    draw_spending_block(frame, app, month_area, &month_title, &app.dashboard_spending_month);
    draw_spending_block(frame, app, year_area, &year_title, &app.dashboard_spending_year);
}

fn draw_spending_block(
    frame: &mut Frame,
    app: &App,
    area: ratatui::layout::Rect,
    title: &str,
    data: &[(i64, i64)],
) {
    let block = theme::styled_block(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if data.is_empty() {
        let empty = Paragraph::new(Span::styled("No expenses.", theme::muted_style()));
        frame.render_widget(empty, inner);
        return;
    }

    let currency = &app.config.currency;
    let tsep = &app.config.thousands_separator;
    let dsep = &app.config.decimal_separator;

    let lines: Vec<Line> = data
        .iter()
        .take(5)
        .map(|(tag_id, amount)| {
            let tag_name = app.tag_name(*tag_id);
            let formatted = format_cents(*amount, currency, tsep, dsep);
            Line::from(vec![
                Span::styled(format!("{:<16}", tag_name), theme::text_style()),
                Span::styled(formatted, theme::expense_style()),
            ])
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
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
