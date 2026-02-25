use chrono::{Datelike, Local};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Bar, BarChart, BarGroup, Block, LineGauge, Paragraph, Tabs};
use ratatui::Frame;

use crate::app::{App, OverviewPeriod};
use crate::domain::models::format_cents;
use crate::ui::theme;

/// Sub-tab titles for the Stats view.
const STATS_TABS: [&str; 3] = ["Overview", "Trends", "Budgets"];

/// Main entry point: draws sub-tab bar, routes to the active sub-tab, draws footer.
pub fn draw_stats(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let [tabs_area, content_area, footer_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area);

    // Sub-tab bar.
    let tabs = Tabs::new(STATS_TABS.to_vec())
        .select(app.stats_tab)
        .style(theme::muted_style())
        .highlight_style(theme::header_style().add_modifier(Modifier::UNDERLINED))
        .divider(" | ");
    frame.render_widget(tabs, tabs_area);

    // Route to active sub-tab.
    match app.stats_tab {
        0 => draw_overview(frame, app, content_area),
        1 => draw_trends(frame, app, content_area),
        2 => draw_budgets(frame, app, content_area),
        _ => draw_overview(frame, app, content_area),
    }

    draw_stats_footer(frame, app, footer_area);
}

// ---------------------------------------------------------------------------
// Overview sub-tab
// ---------------------------------------------------------------------------

fn draw_overview(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let [period_area, header_area, savings_area, breakdown_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(5),
        Constraint::Length(2),
        Constraint::Min(3),
    ])
    .areas(area);

    draw_period_indicator(frame, app, period_area);
    draw_totals_header(frame, app, header_area);
    draw_savings_rate(frame, app, savings_area);
    draw_expense_breakdown(frame, app, breakdown_area);
}

fn draw_period_indicator(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let today = Local::now().date_naive();
    let label = match app.stats_overview_period {
        OverviewPeriod::Monthly => {
            let month_name = today.format("%B %Y").to_string();
            format!("  [m] Period: Monthly \u{2014} {}", month_name)
        }
        OverviewPeriod::Yearly => {
            format!("  [m] Period: Yearly \u{2014} {}", today.year())
        }
    };
    let line = Line::from(vec![
        Span::styled(label, theme::header_style()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_totals_header(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let [income_area, balance_area, expense_area] =
        Layout::horizontal([Constraint::Ratio(1, 3); 3]).areas(area);

    let currency = &app.config.currency;
    let tsep = &app.config.thousands_separator;
    let dsep = &app.config.decimal_separator;
    let (cur_income, cur_expense) = app.overview_totals;
    let (prev_income, prev_expense) = app.overview_prev_totals;
    let cur_balance = cur_income - cur_expense;
    let prev_balance = prev_income - prev_expense;

    fn delta_span(current: i64, previous: i64) -> Span<'static> {
        if previous == 0 {
            return Span::styled("  \u{2014}", Style::default().fg(theme::MUTED));
        }
        let pct = ((current - previous) as f64 / previous.unsigned_abs() as f64) * 100.0;
        if pct >= 0.0 {
            Span::styled(
                format!("  \u{25b2} +{:.1}%", pct),
                Style::default().fg(theme::GREEN),
            )
        } else {
            Span::styled(
                format!("  \u{25bc} {:.1}%", pct),
                Style::default().fg(theme::RED),
            )
        }
    }

    // Income panel
    let income_block = Block::bordered()
        .title(" INCOME ")
        .title_style(theme::income_style().add_modifier(Modifier::BOLD))
        .border_style(Style::default().fg(theme::GREEN));
    let income_text = Paragraph::new(vec![
        Line::from(Span::styled(
            format_cents(cur_income, currency, tsep, dsep),
            theme::income_style().add_modifier(Modifier::BOLD),
        )),
        Line::from(delta_span(cur_income, prev_income)),
    ])
    .alignment(Alignment::Center)
    .block(income_block);
    frame.render_widget(income_text, income_area);

    // Balance panel
    let balance_style = if cur_balance >= 0 {
        theme::income_style()
    } else {
        theme::expense_style()
    };
    let balance_block = Block::bordered()
        .title(" BALANCE ")
        .title_style(theme::header_style())
        .border_style(Style::default().fg(theme::ACCENT));
    let balance_text = Paragraph::new(vec![
        Line::from(Span::styled(
            format_cents(cur_balance, currency, tsep, dsep),
            balance_style.add_modifier(Modifier::BOLD),
        )),
        Line::from(delta_span(cur_balance, prev_balance)),
    ])
    .alignment(Alignment::Center)
    .block(balance_block);
    frame.render_widget(balance_text, balance_area);

    // Expense panel
    let expense_block = Block::bordered()
        .title(" EXPENSES ")
        .title_style(theme::expense_style().add_modifier(Modifier::BOLD))
        .border_style(Style::default().fg(theme::RED));
    let expense_text = Paragraph::new(vec![
        Line::from(Span::styled(
            format_cents(cur_expense, currency, tsep, dsep),
            theme::expense_style().add_modifier(Modifier::BOLD),
        )),
        Line::from(delta_span(cur_expense, prev_expense)),
    ])
    .alignment(Alignment::Center)
    .block(expense_block);
    frame.render_widget(expense_text, expense_area);
}

fn draw_savings_rate(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let (total_income, total_expense) = app.overview_totals;
    let savings_rate = if total_income > 0 {
        (total_income - total_expense) as f64 / total_income as f64
    } else {
        0.0
    };
    let pct = savings_rate * 100.0;

    let style = if pct >= 20.0 {
        theme::income_style()
    } else if pct >= 0.0 {
        theme::warning_style()
    } else {
        theme::expense_style()
    };

    let ratio = savings_rate.clamp(0.0, 1.0);

    let gauge = LineGauge::default()
        .block(Block::default().title(Span::styled(
            format!("  Savings Rate: {:.1}%", pct),
            style.add_modifier(Modifier::BOLD),
        )))
        .filled_style(style)
        .unfilled_style(theme::muted_style())
        .ratio(ratio);
    frame.render_widget(gauge, area);
}

fn draw_expense_breakdown(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = theme::styled_block(" Expenses by Tag ");
    let currency = &app.config.currency;
    let tsep = &app.config.thousands_separator;
    let dsep = &app.config.decimal_separator;
    let total_expense = app.overview_totals.1;

    if app.overview_expense_by_tag.is_empty() {
        let para = Paragraph::new(Span::styled(
            "No expense data for this period.",
            theme::muted_style(),
        ))
        .block(block)
        .alignment(Alignment::Center);
        frame.render_widget(para, area);
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max_amount = app.overview_expense_by_tag.first().map(|(_, a)| *a).unwrap_or(1).max(1);

    let mut lines: Vec<Line> = Vec::new();
    for (tag_id, amount) in &app.overview_expense_by_tag {
        let name = app.tag_name(*tag_id);
        let pct = if total_expense > 0 {
            (*amount as f64 / total_expense as f64) * 100.0
        } else {
            0.0
        };

        let bar_max_width = inner.width.saturating_sub(36) as usize;
        let bar_len = if max_amount > 0 {
            ((*amount as f64 / max_amount as f64) * bar_max_width as f64) as usize
        } else {
            0
        };

        let bar = "\u{2588}".repeat(bar_len);
        let formatted = format_cents(*amount, currency, tsep, dsep);

        lines.push(Line::from(vec![
            Span::styled(format!("  {:<12}", name), theme::text_style()),
            Span::styled(bar, theme::expense_style()),
            Span::styled(
                format!("  {:>12} {:>5.1}%", formatted, pct),
                theme::text_style(),
            ),
        ]));
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, inner);
}

// ---------------------------------------------------------------------------
// Trends sub-tab
// ---------------------------------------------------------------------------

fn draw_trends(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let title = format!(
        " Income vs Expenses (last {} months) ",
        app.stats_months_range
    );

    if app.monthly_totals.is_empty() {
        let block = theme::styled_block(&title);
        let para = Paragraph::new(Span::styled(
            "No transaction data yet.",
            theme::muted_style(),
        ))
        .block(block)
        .alignment(Alignment::Center);
        frame.render_widget(para, area);
        return;
    }

    // Split: chart top, table bottom
    let table_rows = app.monthly_totals.len().min(12) as u16 + 4;
    let [chart_area, table_area] = Layout::vertical([
        Constraint::Min(8),
        Constraint::Length(table_rows),
    ])
    .areas(area);

    draw_trends_chart(frame, app, chart_area, &title);
    draw_trends_table(frame, app, table_area);
}

fn draw_trends_chart(frame: &mut Frame, app: &App, area: ratatui::layout::Rect, title: &str) {
    let groups: Vec<BarGroup> = app
        .monthly_totals
        .iter()
        .map(|(month, income, expense)| {
            // Abbreviate: "2026-02" -> "Feb"
            let label = chrono::NaiveDate::parse_from_str(&format!("{}-01", month), "%Y-%m-%d")
                .map(|d| d.format("%b").to_string())
                .unwrap_or_else(|_| month.clone());

            BarGroup::default()
                .label(Line::from(label))
                .bars(&[
                    Bar::default()
                        .value(*income as u64)
                        .style(Style::default().fg(theme::GREEN)),
                    Bar::default()
                        .value(*expense as u64)
                        .style(Style::default().fg(theme::RED)),
                ])
        })
        .collect();

    // Adapt bar width to available space
    let month_count = groups.len() as u16;
    let available = area.width.saturating_sub(4);
    let bar_width = if month_count > 0 {
        let per_group = available / month_count;
        ((per_group.saturating_sub(3)) / 2).max(1)
    } else {
        3
    };

    let chart = groups
        .into_iter()
        .fold(BarChart::default(), |chart, group| chart.data(group))
        .block(
            Block::bordered()
                .title(title.to_string())
                .title_style(theme::header_style())
                .border_style(Style::default().fg(theme::BORDER)),
        )
        .bar_width(bar_width)
        .bar_gap(1)
        .group_gap(2)
        .bar_style(Style::default().fg(theme::GREEN))
        .value_style(Style::default().fg(theme::FG).add_modifier(Modifier::BOLD));

    frame.render_widget(chart, area);
}

fn draw_trends_table(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = theme::styled_block(" Monthly Detail ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let currency = &app.config.currency;
    let tsep = &app.config.thousands_separator;
    let dsep = &app.config.decimal_separator;

    let mut lines: Vec<Line> = Vec::new();

    // Header row
    lines.push(Line::from(vec![
        Span::styled(format!("  {:<10}", "Month"), theme::header_style()),
        Span::styled(format!("{:>14}", "Income"), theme::header_style()),
        Span::styled(format!("{:>14}", "Expense"), theme::header_style()),
        Span::styled(format!("{:>14}", "Net"), theme::header_style()),
        Span::styled(format!("{:>10}", "MoM \u{0394}"), theme::header_style()),
    ]));

    let months: Vec<&(String, i64, i64)> = app.monthly_totals.iter().collect();

    let mut total_income: i64 = 0;
    let mut total_expense: i64 = 0;
    let count = months.len();

    // Data rows (most recent first)
    for (idx, (month, income, expense)) in months.iter().enumerate().rev() {
        let net = income - expense;
        total_income += income;
        total_expense += expense;

        let net_style = if net >= 0 {
            theme::income_style()
        } else {
            theme::expense_style()
        };

        let delta_span = if idx > 0 {
            let (_, prev_inc, prev_exp) = months[idx - 1];
            let prev_net = prev_inc - prev_exp;
            if prev_net == 0 {
                Span::styled(
                    format!("{:>10}", "\u{2014}"),
                    Style::default().fg(theme::MUTED),
                )
            } else {
                let pct =
                    ((net - prev_net) as f64 / prev_net.unsigned_abs() as f64) * 100.0;
                if pct >= 0.0 {
                    Span::styled(
                        format!("{:>8.1}%\u{25b2}", pct),
                        Style::default().fg(theme::GREEN),
                    )
                } else {
                    Span::styled(
                        format!("{:>8.1}%\u{25bc}", pct),
                        Style::default().fg(theme::RED),
                    )
                }
            }
        } else {
            Span::styled(
                format!("{:>10}", "\u{2014}"),
                Style::default().fg(theme::MUTED),
            )
        };

        lines.push(Line::from(vec![
            Span::styled(format!("  {:<10}", month), theme::text_style()),
            Span::styled(
                format!("{:>14}", format_cents(*income, currency, tsep, dsep)),
                theme::income_style(),
            ),
            Span::styled(
                format!("{:>14}", format_cents(*expense, currency, tsep, dsep)),
                theme::expense_style(),
            ),
            Span::styled(
                format!("{:>14}", format_cents(net, currency, tsep, dsep)),
                net_style,
            ),
            delta_span,
        ]));

        if lines.len() >= inner.height as usize {
            break;
        }
    }

    // Averages footer
    if count > 0 {
        let avg_income = total_income / count as i64;
        let avg_expense = total_expense / count as i64;
        let avg_net = avg_income - avg_expense;
        let avg_net_style = if avg_net >= 0 {
            theme::income_style()
        } else {
            theme::expense_style()
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:<10}", "Average"),
                theme::header_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>14}", format_cents(avg_income, currency, tsep, dsep)),
                theme::income_style(),
            ),
            Span::styled(
                format!("{:>14}", format_cents(avg_expense, currency, tsep, dsep)),
                theme::expense_style(),
            ),
            Span::styled(
                format!("{:>14}", format_cents(avg_net, currency, tsep, dsep)),
                avg_net_style,
            ),
            Span::styled(format!("{:>10}", ""), theme::muted_style()),
        ]));
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, inner);
}

// ---------------------------------------------------------------------------
// Budgets sub-tab
// ---------------------------------------------------------------------------

/// Calculate pace projection for a budget.
/// Returns (projected_total, days_elapsed, total_days).
fn budget_pace_projection(budget: &crate::domain::models::Budget, spent: i64) -> (i64, u32, u32) {
    use chrono::{Datelike, Local, NaiveDate};
    use crate::domain::models::BudgetPeriod;

    let today = Local::now().date_naive();

    let (period_start, period_end) = match budget.period {
        BudgetPeriod::Weekly => {
            let weekday = today.weekday().num_days_from_monday();
            let start = today - chrono::Duration::days(weekday as i64);
            let end = start + chrono::Duration::days(7);
            (start, end)
        }
        BudgetPeriod::Monthly => {
            let start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
            let end = if today.month() == 12 {
                NaiveDate::from_ymd_opt(today.year() + 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1).unwrap()
            };
            (start, end)
        }
        BudgetPeriod::Yearly => {
            let start = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
            let end = NaiveDate::from_ymd_opt(today.year() + 1, 1, 1).unwrap();
            (start, end)
        }
    };

    let total_days = (period_end - period_start).num_days() as u32;
    let days_elapsed = (today - period_start).num_days().max(1) as u32;

    let daily_rate = spent as f64 / days_elapsed as f64;
    let projected = (daily_rate * total_days as f64).round() as i64;

    (projected, days_elapsed, total_days)
}

fn draw_budgets(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = theme::styled_block(" Budget Status ");
    let currency = &app.config.currency;
    let tsep = &app.config.thousands_separator;
    let dsep = &app.config.decimal_separator;

    if app.budget_spending.is_empty() {
        let para = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("No active budgets.", theme::muted_style())),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Create budgets in the ", theme::muted_style()),
                Span::styled("Budgets", theme::header_style()),
                Span::styled(" view ", theme::muted_style()),
                Span::styled("[4]", theme::header_style()),
            ]),
        ])
        .block(block)
        .alignment(Alignment::Center);
        frame.render_widget(para, area);
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut on_track = 0u32;
    let mut warning = 0u32;
    let mut over = 0u32;

    let budget_count = app.budget_spending.len();
    let mut constraints: Vec<Constraint> = Vec::new();
    for _ in 0..budget_count {
        constraints.push(Constraint::Length(4));
    }
    constraints.push(Constraint::Length(2));
    constraints.push(Constraint::Min(0));

    let rows = Layout::vertical(constraints).split(inner);

    for (i, (budget, spent)) in app.budget_spending.iter().enumerate() {
        if i >= rows.len().saturating_sub(2) {
            break;
        }

        let tag_name = match budget.tag_id {
            Some(tid) => app.tag_name(tid),
            None => "Global".to_string(),
        };

        let limit = budget.amount;
        let pct = if limit > 0 { (*spent as f64 / limit as f64) * 100.0 } else { 0.0 };
        let ratio = if limit > 0 { (*spent as f64 / limit as f64).min(1.0) } else { 0.0 };

        let style = if pct >= 100.0 {
            over += 1;
            theme::expense_style()
        } else if pct >= 80.0 {
            warning += 1;
            theme::warning_style()
        } else {
            on_track += 1;
            theme::income_style()
        };

        let [label_area, gauge_area, pace_area, _spacer] =
            Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .areas(rows[i]);

        let formatted_spent = format_cents(*spent, currency, tsep, dsep);
        let formatted_limit = format_cents(limit, currency, tsep, dsep);

        let label = Line::from(vec![
            Span::styled(
                format!("  \u{25cf} {} ({})", tag_name, budget.period),
                theme::text_style(),
            ),
            Span::styled(
                format!("  {} / {}  ({:.0}%)", formatted_spent, formatted_limit, pct),
                style,
            ),
        ]);
        frame.render_widget(Paragraph::new(label), label_area);

        let gauge = LineGauge::default()
            .filled_style(style.add_modifier(Modifier::BOLD))
            .unfilled_style(theme::muted_style())
            .ratio(ratio);
        frame.render_widget(gauge, gauge_area);

        // Pace projection
        let (projected, _days_elapsed, _total_days) = budget_pace_projection(budget, *spent);
        let formatted_projected = format_cents(projected, currency, tsep, dsep);
        let pace_style = if projected >= limit {
            theme::expense_style()
        } else if projected as f64 >= limit as f64 * 0.8 {
            theme::warning_style()
        } else {
            theme::income_style()
        };
        let status_label = if projected >= limit {
            "\u{26a0} OVER BUDGET"
        } else {
            "\u{2713} On track"
        };
        let pace_line = Line::from(vec![
            Span::styled("    \u{23f1} Pace: ", theme::muted_style()),
            Span::styled(format!("{} projected", formatted_projected), pace_style),
            Span::styled(format!(" \u{2014} {}", status_label), pace_style),
        ]);
        frame.render_widget(Paragraph::new(pace_line), pace_area);
    }

    // Summary line
    let summary_idx = budget_count;
    if summary_idx < rows.len().saturating_sub(1) {
        let summary = Line::from(vec![
            Span::styled("  ", theme::text_style()),
            Span::styled(format!("{on_track} on track"), theme::income_style()),
            Span::styled("    ", theme::text_style()),
            Span::styled(format!("{warning} warning"), theme::warning_style()),
            Span::styled("    ", theme::text_style()),
            Span::styled(format!("{over} over budget"), theme::expense_style()),
        ]);
        frame.render_widget(Paragraph::new(vec![Line::from(""), summary]), rows[summary_idx]);
    }
}

// ---------------------------------------------------------------------------
// Footer
// ---------------------------------------------------------------------------

fn draw_stats_footer(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let m_label = if app.stats_tab == 0 {
        let period = match app.stats_overview_period {
            OverviewPeriod::Monthly => "Monthly",
            OverviewPeriod::Yearly => "Yearly",
        };
        format!("period:{} ", period)
    } else {
        format!("range:{}mo ", app.stats_months_range)
    };
    let help = Line::from(vec![
        Span::styled(" [h/l]", theme::header_style()),
        Span::styled("tab ", theme::text_style()),
        Span::styled("[m]", theme::header_style()),
        Span::styled(m_label, theme::text_style()),
        Span::styled("[1-6]", theme::header_style()),
        Span::styled("view ", theme::text_style()),
        Span::styled("[?]", theme::header_style()),
        Span::styled("help", theme::text_style()),
    ]);
    frame.render_widget(Paragraph::new(help), area);
}
