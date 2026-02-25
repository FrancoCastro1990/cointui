# Stats View Redesign — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Redesign the Stats view with period-filtered overview, grouped bar charts, MoM deltas, averages, and budget pace projections — using only Ratatui 0.29 built-in widgets.

**Architecture:** Add two new TransactionRepo query methods for period-scoped data. Add OverviewPeriod enum and new cached state fields to App. Rewrite all three sub-tabs in `stats.rs`: Overview gets Monthly/Yearly toggle with MoM deltas; Trends gets a grouped BarChart + detail table; Budgets gets pace projection lines.

**Tech Stack:** Rust, Ratatui 0.29 (BarChart, BarGroup, Bar, Sparkline, LineGauge), chrono 0.4, rusqlite 0.32.

---

### Task 1: Add `get_totals_for_period()` to TransactionRepo

**Files:**
- Modify: `src/db/transaction_repo.rs:205` (after `get_totals()`)

**Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` block in `src/db/transaction_repo.rs`:

```rust
#[test]
fn test_get_totals_for_period() {
    let db = Database::in_memory().unwrap();
    let tag_repo = TagRepo::new(&db);
    tag_repo.create("Test").unwrap();
    let repo = TransactionRepo::new(&db);

    // Add transactions across different months
    repo.create(&Transaction {
        id: None, source: "Jan income".into(), amount: 1000, kind: TransactionKind::Income,
        tag_id: 1, date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        notes: None, created_at: None, updated_at: None,
    }).unwrap();
    repo.create(&Transaction {
        id: None, source: "Jan expense".into(), amount: 400, kind: TransactionKind::Expense,
        tag_id: 1, date: NaiveDate::from_ymd_opt(2026, 1, 20).unwrap(),
        notes: None, created_at: None, updated_at: None,
    }).unwrap();
    repo.create(&Transaction {
        id: None, source: "Feb income".into(), amount: 1500, kind: TransactionKind::Income,
        tag_id: 1, date: NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(),
        notes: None, created_at: None, updated_at: None,
    }).unwrap();

    // Query only January
    let (inc, exp) = repo.get_totals_for_period("2026-01-01", "2026-02-01").unwrap();
    assert_eq!(inc, 1000);
    assert_eq!(exp, 400);

    // Query only February
    let (inc, exp) = repo.get_totals_for_period("2026-02-01", "2026-03-01").unwrap();
    assert_eq!(inc, 1500);
    assert_eq!(exp, 0);

    // Empty range
    let (inc, exp) = repo.get_totals_for_period("2025-01-01", "2025-02-01").unwrap();
    assert_eq!(inc, 0);
    assert_eq!(exp, 0);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_get_totals_for_period -- --nocapture`
Expected: FAIL — method `get_totals_for_period` not found

**Step 3: Write minimal implementation**

Add after `get_totals()` (line 205) in `src/db/transaction_repo.rs`:

```rust
/// Return `(total_income, total_expense)` for transactions where
/// `date >= start AND date < end`. Date strings are `YYYY-MM-DD`.
pub fn get_totals_for_period(&self, start: &str, end: &str) -> Result<(i64, i64)> {
    let mut stmt = self.db.conn().prepare(
        "SELECT
             COALESCE(SUM(CASE WHEN kind = 'income'  THEN amount ELSE 0 END), 0),
             COALESCE(SUM(CASE WHEN kind = 'expense' THEN amount ELSE 0 END), 0)
         FROM transactions
         WHERE date >= ?1 AND date < ?2",
    )?;
    let (income, expense) = stmt.query_row(rusqlite::params![start, end], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })?;
    Ok((income, expense))
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_get_totals_for_period -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/db/transaction_repo.rs
git commit -m "feat(repo): add get_totals_for_period to TransactionRepo"
```

---

### Task 2: Add `get_expense_by_tag_for_period()` to TransactionRepo

**Files:**
- Modify: `src/db/transaction_repo.rs` (after the method added in Task 1)

**Step 1: Write the failing test**

```rust
#[test]
fn test_get_expense_by_tag_for_period() {
    let db = Database::in_memory().unwrap();
    let tag_repo = TagRepo::new(&db);
    tag_repo.create("Food").unwrap();     // id=1
    tag_repo.create("Transport").unwrap(); // id=2
    let repo = TransactionRepo::new(&db);

    // January expenses
    repo.create(&Transaction {
        id: None, source: "Lunch".into(), amount: 300, kind: TransactionKind::Expense,
        tag_id: 1, date: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
        notes: None, created_at: None, updated_at: None,
    }).unwrap();
    repo.create(&Transaction {
        id: None, source: "Bus".into(), amount: 100, kind: TransactionKind::Expense,
        tag_id: 2, date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        notes: None, created_at: None, updated_at: None,
    }).unwrap();
    // Income (should be excluded)
    repo.create(&Transaction {
        id: None, source: "Salary".into(), amount: 5000, kind: TransactionKind::Income,
        tag_id: 1, date: NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
        notes: None, created_at: None, updated_at: None,
    }).unwrap();

    let result = repo.get_expense_by_tag_for_period("2026-01-01", "2026-02-01").unwrap();
    // Sorted descending by amount: Food(300) then Transport(100)
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], (1, 300)); // Food
    assert_eq!(result[1], (2, 100)); // Transport

    // Empty range
    let result = repo.get_expense_by_tag_for_period("2025-06-01", "2025-07-01").unwrap();
    assert!(result.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_get_expense_by_tag_for_period -- --nocapture`
Expected: FAIL — method not found

**Step 3: Write minimal implementation**

```rust
/// Return expense totals grouped by tag_id for the given date range,
/// sorted descending by amount. Date strings are `YYYY-MM-DD`.
pub fn get_expense_by_tag_for_period(&self, start: &str, end: &str) -> Result<Vec<(i64, i64)>> {
    let mut stmt = self.db.conn().prepare(
        "SELECT tag_id, COALESCE(SUM(amount), 0)
         FROM transactions
         WHERE kind = 'expense' AND date >= ?1 AND date < ?2
         GROUP BY tag_id
         ORDER BY SUM(amount) DESC",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![start, end], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_get_expense_by_tag_for_period -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/db/transaction_repo.rs
git commit -m "feat(repo): add get_expense_by_tag_for_period to TransactionRepo"
```

---

### Task 3: Add OverviewPeriod enum and new App state fields

**Files:**
- Modify: `src/app.rs:77` (after `PendingAction` enum, before `App` struct)
- Modify: `src/app.rs:130-131` (stats state fields inside `App` struct)
- Modify: `src/app.rs:168-169` (initialization in `App::new()`)
- Modify: `src/app.rs:180-191` (reload_all — add new reload call)
- Modify: `src/app.rs:267-280` (after reload_expense_by_tag — add new reload method)

**Step 1: Add the OverviewPeriod enum**

Add after line 76 (after `PendingAction` enum) in `src/app.rs`:

```rust
/// Period filter for the Stats Overview tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewPeriod {
    Monthly,
    Yearly,
}
```

**Step 2: Add new fields to `App` struct**

Replace lines 129-131 in `src/app.rs`:

```rust
    // Stats sub-tab state.
    pub stats_tab: usize,
    pub stats_months_range: usize,
    pub stats_overview_period: OverviewPeriod,
    /// Period-scoped totals (income, expense) for current period.
    pub overview_totals: (i64, i64),
    /// Period-scoped totals (income, expense) for previous period (for delta).
    pub overview_prev_totals: (i64, i64),
    /// Expense by tag for the selected overview period.
    pub overview_expense_by_tag: Vec<(i64, i64)>,
```

**Step 3: Initialize new fields in `App::new()`**

After line 169 (`stats_months_range: 6,`) add:

```rust
            stats_overview_period: OverviewPeriod::Monthly,
            overview_totals: (0, 0),
            overview_prev_totals: (0, 0),
            overview_expense_by_tag: Vec::new(),
```

**Step 4: Add `reload_overview_data()` method**

Add after `reload_expense_by_tag()` (after line 280):

```rust
    pub fn reload_overview_data(&mut self) -> Result<()> {
        use chrono::{Local, Datelike, NaiveDate};

        let today = Local::now().date_naive();
        let (cur_start, cur_end, prev_start, prev_end) = match self.stats_overview_period {
            OverviewPeriod::Monthly => {
                let cur_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
                let cur_end = if today.month() == 12 {
                    NaiveDate::from_ymd_opt(today.year() + 1, 1, 1).unwrap()
                } else {
                    NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1).unwrap()
                };
                let prev_end = cur_start;
                let prev_start = if today.month() == 1 {
                    NaiveDate::from_ymd_opt(today.year() - 1, 12, 1).unwrap()
                } else {
                    NaiveDate::from_ymd_opt(today.year(), today.month() - 1, 1).unwrap()
                };
                (cur_start, cur_end, prev_start, prev_end)
            }
            OverviewPeriod::Yearly => {
                let cur_start = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
                let cur_end = NaiveDate::from_ymd_opt(today.year() + 1, 1, 1).unwrap();
                let prev_start = NaiveDate::from_ymd_opt(today.year() - 1, 1, 1).unwrap();
                let prev_end = cur_start;
                (cur_start, cur_end, prev_start, prev_end)
            }
        };

        let repo = TransactionRepo::new(&self.db);
        let fmt = |d: NaiveDate| d.format("%Y-%m-%d").to_string();

        self.overview_totals = repo.get_totals_for_period(&fmt(cur_start), &fmt(cur_end))?;
        self.overview_prev_totals = repo.get_totals_for_period(&fmt(prev_start), &fmt(prev_end))?;
        self.overview_expense_by_tag = repo.get_expense_by_tag_for_period(&fmt(cur_start), &fmt(cur_end))?;
        Ok(())
    }
```

**Step 5: Wire into `reload_all()`**

Add `self.reload_overview_data()?;` after `self.reload_expense_by_tag()?;` (line 189).

**Step 6: Run all tests**

Run: `cargo test`
Expected: All tests pass (compilation check — no new tests needed for this wiring task)

**Step 7: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): add OverviewPeriod enum and overview data caching"
```

---

### Task 4: Update key handling for `m` key context-awareness

**Files:**
- Modify: `src/app.rs:691-715` (`handle_stats_key()`)

**Step 1: Rewrite `handle_stats_key()` method**

Replace lines 691-715 in `src/app.rs`:

```rust
    fn handle_stats_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => {
                if self.stats_tab > 0 {
                    self.stats_tab -= 1;
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if self.stats_tab < 2 {
                    self.stats_tab += 1;
                }
            }
            KeyCode::Char('m') => {
                if self.stats_tab == 0 {
                    // Overview: toggle Monthly/Yearly
                    self.stats_overview_period = match self.stats_overview_period {
                        OverviewPeriod::Monthly => OverviewPeriod::Yearly,
                        OverviewPeriod::Yearly => OverviewPeriod::Monthly,
                    };
                    if let Err(e) = self.reload_overview_data() {
                        self.set_status(e.user_message());
                    }
                } else {
                    // Trends/Budgets: cycle months range
                    self.stats_months_range = match self.stats_months_range {
                        6 => 12,
                        12 => 24,
                        _ => 6,
                    };
                    if let Err(e) = self.reload_monthly_totals() {
                        self.set_status(e.user_message());
                    }
                }
            }
            _ => {}
        }
    }
```

**Step 2: Run all tests**

Run: `cargo test`
Expected: All tests pass

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): context-aware 'm' key for stats overview/trends"
```

---

### Task 5: Rewrite Overview tab with period filter and MoM deltas

**Files:**
- Modify: `src/ui/views/stats.rs:1-4` (add imports for chrono)
- Modify: `src/ui/views/stats.rs:46-198` (replace `draw_overview`, `draw_totals_header`, `draw_savings_rate`, `draw_expense_breakdown`)

**Step 1: Update imports**

Replace lines 1-9 in `src/ui/views/stats.rs`:

```rust
use chrono::{Datelike, Local};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Bar, BarChart, BarGroup, Block, LineGauge, Paragraph, Tabs};
use ratatui::Frame;

use crate::app::{App, OverviewPeriod};
use crate::domain::models::format_cents;
use crate::ui::theme;
```

**Step 2: Rewrite `draw_overview()`**

Replace lines 46-57:

```rust
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
            format!("  [m] Period: Monthly — {}", month_name)
        }
        OverviewPeriod::Yearly => {
            format!("  [m] Period: Yearly — {}", today.year())
        }
    };
    let line = Line::from(vec![
        Span::styled(label, theme::header_style()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}
```

**Step 3: Rewrite `draw_totals_header()` with MoM deltas**

Replace lines 59-112:

```rust
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

    // Helper to format a delta percentage as "▲ +X,X%" or "▼ -X,X%"
    fn delta_span(current: i64, previous: i64) -> Span<'static> {
        if previous == 0 {
            return Span::styled("  —", Style::default().fg(theme::MUTED));
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
```

**Step 4: Update `draw_savings_rate()` to use overview_totals**

Replace lines 114-142:

```rust
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
```

**Step 5: Update `draw_expense_breakdown()` to use overview data**

Replace lines 144-198. Change references from `app.totals.1` to `app.overview_totals.1` and from `app.expense_by_tag` to `app.overview_expense_by_tag`:

```rust
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
```

**Step 6: Run tests and check compilation**

Run: `cargo test && cargo clippy`
Expected: All tests pass, no warnings

**Step 7: Commit**

```bash
git add src/ui/views/stats.rs
git commit -m "feat(ui): rewrite Overview tab with period filter and MoM deltas"
```

---

### Task 6: Rewrite Trends tab with grouped BarChart and detail table

**Files:**
- Modify: `src/ui/views/stats.rs:204-292` (replace `draw_trends`)

**Step 1: Rewrite `draw_trends()`**

Replace the entire `draw_trends` function (lines 204-292) with:

```rust
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

    // Split: chart top half, table bottom half
    let [chart_area, table_area] = Layout::vertical([
        Constraint::Min(8),
        Constraint::Length(app.monthly_totals.len().min(12) as u16 + 4),
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
    let available = area.width.saturating_sub(4); // borders
    let bar_width = if month_count > 0 {
        // Each group = (bar_width * 2) + bar_gap(1) + group_gap
        // Solve: month_count * (bw*2 + 1 + 2) <= available
        let per_group = available / month_count;
        ((per_group.saturating_sub(3)) / 2).max(1)
    } else {
        3
    };

    let chart = BarChart::default()
        .block(
            Block::bordered()
                .title(title.to_string())
                .title_style(theme::header_style())
                .border_style(Style::default().fg(theme::BORDER)),
        )
        .data(groups)
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

    // Collect for MoM delta calculation
    let months: Vec<&(String, i64, i64)> = app.monthly_totals.iter().collect();

    // Data rows (most recent first)
    let mut total_income: i64 = 0;
    let mut total_expense: i64 = 0;
    let count = months.len();

    for (idx, (month, income, expense)) in months.iter().enumerate().rev() {
        let net = income - expense;
        total_income += income;
        total_expense += expense;

        let net_style = if net >= 0 { theme::income_style() } else { theme::expense_style() };

        // MoM delta: compare net to previous month's net
        let delta_span = if idx > 0 {
            let (_, prev_inc, prev_exp) = months[idx - 1];
            let prev_net = prev_inc - prev_exp;
            if prev_net == 0 {
                Span::styled(format!("{:>10}", "—"), Style::default().fg(theme::MUTED))
            } else {
                let pct = ((net - prev_net) as f64 / prev_net.unsigned_abs() as f64) * 100.0;
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
            Span::styled(format!("{:>10}", "—"), Style::default().fg(theme::MUTED))
        };

        lines.push(Line::from(vec![
            Span::styled(format!("  {:<10}", month), theme::text_style()),
            Span::styled(format!("{:>14}", format_cents(**income, currency, tsep, dsep)), theme::income_style()),
            Span::styled(format!("{:>14}", format_cents(**expense, currency, tsep, dsep)), theme::expense_style()),
            Span::styled(format!("{:>14}", format_cents(net, currency, tsep, dsep)), net_style),
            delta_span,
        ]));

        // Limit visible rows to avoid overflow
        if lines.len() >= inner.height as usize {
            break;
        }
    }

    // Averages footer
    if count > 0 {
        let avg_income = total_income / count as i64;
        let avg_expense = total_expense / count as i64;
        let avg_net = avg_income - avg_expense;
        let avg_net_style = if avg_net >= 0 { theme::income_style() } else { theme::expense_style() };

        lines.push(Line::from(vec![
            Span::styled(format!("  {:<10}", "Average"), theme::header_style().add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:>14}", format_cents(avg_income, currency, tsep, dsep)), theme::income_style()),
            Span::styled(format!("{:>14}", format_cents(avg_expense, currency, tsep, dsep)), theme::expense_style()),
            Span::styled(format!("{:>14}", format_cents(avg_net, currency, tsep, dsep)), avg_net_style),
            Span::styled(format!("{:>10}", ""), theme::muted_style()),
        ]));
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, inner);
}
```

**Step 2: Run tests and check compilation**

Run: `cargo test && cargo clippy`
Expected: All tests pass, no warnings

**Step 3: Commit**

```bash
git add src/ui/views/stats.rs
git commit -m "feat(ui): rewrite Trends tab with grouped BarChart and detail table"
```

---

### Task 7: Add pace projection to Budgets tab

**Files:**
- Modify: `src/ui/views/stats.rs` (the `draw_budgets` function, currently lines 298-420)

**Step 1: Write a helper for pace projection calculation**

Add before `draw_budgets()`:

```rust
/// Calculate pace projection for a budget.
/// Returns (projected_total, days_elapsed, total_days) or None if period just started.
fn budget_pace_projection(budget: &crate::domain::models::Budget, spent: i64) -> Option<(i64, u32, u32)> {
    use chrono::{Datelike, Local, NaiveDate};
    use crate::domain::models::BudgetPeriod;

    let today = Local::now().date_naive();

    let (period_start, period_end) = match budget.period {
        BudgetPeriod::Weekly => {
            // Monday of current week
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

    Some((projected, days_elapsed, total_days))
}
```

**Step 2: Rewrite `draw_budgets()` to include pace projection**

Replace the entire `draw_budgets` function. Key changes:
- Each budget now gets 4 lines instead of 3 (label + gauge + pace + spacer)
- Add the pace projection line after each gauge

```rust
fn draw_budgets(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = theme::styled_block(" Budget Status ");
    let currency = &app.config.currency;
    let tsep = &app.config.thousands_separator;
    let dsep = &app.config.decimal_separator;

    if app.budget_spending.is_empty() {
        let para = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "No active budgets.",
                theme::muted_style(),
            )),
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
        constraints.push(Constraint::Length(4)); // label + gauge + pace + spacer
    }
    constraints.push(Constraint::Length(2)); // summary
    constraints.push(Constraint::Min(0));    // fill

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
        let pct = if limit > 0 {
            (*spent as f64 / limit as f64) * 100.0
        } else {
            0.0
        };
        let ratio = if limit > 0 {
            (*spent as f64 / limit as f64).min(1.0)
        } else {
            0.0
        };

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
        if let Some((projected, _days_elapsed, _total_days)) = budget_pace_projection(budget, *spent) {
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
                Span::styled(format!(" — {}", status_label), pace_style),
            ]);
            frame.render_widget(Paragraph::new(pace_line), pace_area);
        }
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
```

**Step 3: Update footer to show context-aware `m` key label**

Replace the `draw_stats_footer` function:

```rust
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
```

**Step 4: Run all tests and clippy**

Run: `cargo test && cargo clippy`
Expected: All pass, zero warnings

**Step 5: Commit**

```bash
git add src/ui/views/stats.rs
git commit -m "feat(ui): add pace projection to Budgets tab and update footer"
```

---

### Task 8: Final integration test — full build and manual smoke test

**Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass

**Step 2: Run clippy**

Run: `cargo clippy`
Expected: Zero warnings

**Step 3: Build release**

Run: `cargo build --release`
Expected: Successful compilation

**Step 4: Manual smoke test checklist**

Run: `cargo run`

Verify:
- [ ] Press `3` to go to Stats view
- [ ] Overview tab shows current month data with MoM deltas (▲/▼)
- [ ] Press `m` to toggle Monthly/Yearly — data updates
- [ ] Press `l` to go to Trends tab
- [ ] Grouped BarChart shows income (green) vs expense (red) bars
- [ ] Monthly detail table below chart has MoM delta column
- [ ] Averages row at bottom
- [ ] Press `m` to cycle 6/12/24 months — chart and table update
- [ ] Press `l` to go to Budgets tab
- [ ] Each budget shows pace projection line
- [ ] Footer shows context-aware `m` label

**Step 5: Final commit (if any fixes needed)**

```bash
git add -A
git commit -m "fix: address issues found during smoke testing"
```
