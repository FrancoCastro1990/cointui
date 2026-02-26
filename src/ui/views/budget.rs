use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, LineGauge, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::domain::models::{format_cents, Budget, BudgetPeriod};
use crate::ui::theme;
use crate::ui::views::form::{centered_rect, render_text_field, render_toggle_field};
use crate::ui::views::stats::budget_pace_projection;

// ---------------------------------------------------------------------------
// Budget form
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetFormField {
    Tag,
    Amount,
    Period,
}

const BUDGET_FIELD_ORDER: &[BudgetFormField] = &[
    BudgetFormField::Tag,
    BudgetFormField::Amount,
    BudgetFormField::Period,
];

/// State for the budget add/edit form.
#[derive(Debug, Clone)]
pub struct BudgetForm {
    pub editing_id: Option<i64>,
    pub selected_tag_index: usize,
    pub tag_names: Vec<String>,
    pub tag_ids: Vec<Option<i64>>,
    pub amount: String,
    pub period: BudgetPeriod,
    pub field_index: usize,
    pub errors: Vec<String>,
}

impl BudgetForm {
    /// Create a blank form for adding a new budget.
    pub fn new(tag_names: Vec<String>, tag_ids: Vec<i64>) -> Self {
        // Prepend "Global" option (tag_id = None).
        let mut names = vec!["Global".to_string()];
        names.extend(tag_names);
        let mut ids: Vec<Option<i64>> = vec![None];
        ids.extend(tag_ids.into_iter().map(Some));

        Self {
            editing_id: None,
            selected_tag_index: 1.min(names.len().saturating_sub(1)), // default to first real tag
            tag_names: names,
            tag_ids: ids,
            amount: String::new(),
            period: BudgetPeriod::Monthly,
            field_index: 0,
            errors: Vec::new(),
        }
    }

    /// Create a form pre-filled with an existing budget for editing.
    pub fn from_budget(
        budget: &Budget,
        tag_names: Vec<String>,
        tag_ids: Vec<i64>,
    ) -> Self {
        let mut names = vec!["Global".to_string()];
        names.extend(tag_names);
        let mut ids: Vec<Option<i64>> = vec![None];
        ids.extend(tag_ids.into_iter().map(Some));

        let selected_tag_index = ids
            .iter()
            .position(|&id| id == budget.tag_id)
            .unwrap_or(0);

        Self {
            editing_id: budget.id,
            selected_tag_index,
            tag_names: names,
            tag_ids: ids,
            amount: budget.amount.to_string(),
            period: budget.period,
            field_index: 0,
            errors: Vec::new(),
        }
    }

    pub fn current_field(&self) -> BudgetFormField {
        BUDGET_FIELD_ORDER[self.field_index]
    }

    pub fn next_field(&mut self) {
        self.field_index = (self.field_index + 1) % BUDGET_FIELD_ORDER.len();
    }

    pub fn prev_field(&mut self) {
        if self.field_index == 0 {
            self.field_index = BUDGET_FIELD_ORDER.len() - 1;
        } else {
            self.field_index -= 1;
        }
    }

    pub fn type_char(&mut self, c: char) {
        if self.current_field() == BudgetFormField::Amount
            && (c.is_ascii_digit() || (c == '.' && !self.amount.contains('.')))
        {
            self.amount.push(c);
        }
    }

    pub fn backspace(&mut self) {
        if self.current_field() == BudgetFormField::Amount {
            self.amount.pop();
        }
    }

    pub fn cycle_option(&mut self) {
        match self.current_field() {
            BudgetFormField::Tag => {
                if !self.tag_names.is_empty() {
                    self.selected_tag_index =
                        (self.selected_tag_index + 1) % self.tag_names.len();
                }
            }
            BudgetFormField::Period => {
                self.period = match self.period {
                    BudgetPeriod::Weekly => BudgetPeriod::Monthly,
                    BudgetPeriod::Monthly => BudgetPeriod::Yearly,
                    BudgetPeriod::Yearly => BudgetPeriod::Weekly,
                };
            }
            _ => {}
        }
    }

    /// Validate and convert form data to a Budget.
    pub fn to_budget(&mut self) -> Result<Budget, Vec<String>> {
        self.errors.clear();

        let amount = match self.parse_amount() {
            Some(a) if a > 0 => a,
            _ => {
                self.errors.push("Amount must be a positive number.".into());
                0
            }
        };

        if !self.errors.is_empty() {
            return Err(self.errors.clone());
        }

        let tag_id = self.tag_ids[self.selected_tag_index];

        Ok(Budget {
            id: self.editing_id,
            tag_id,
            amount,
            period: self.period,
            active: true,
        })
    }

    fn parse_amount(&self) -> Option<i64> {
        let trimmed = self.amount.trim();
        if trimmed.is_empty() {
            return None;
        }
        let val: f64 = trimmed.parse().ok()?;
        if val < 0.0 {
            return None;
        }
        Some(val.round() as i64)
    }
}

// ---------------------------------------------------------------------------
// Budget form draw
// ---------------------------------------------------------------------------

pub fn draw_budget_form(frame: &mut Frame, form: &BudgetForm, currency: &str) {
    let area = centered_rect(50, 50, frame.area());

    frame.render_widget(Clear, area);

    let title = if form.editing_id.is_some() {
        " Edit Budget "
    } else {
        " New Budget "
    };

    let block = Block::bordered()
        .title(title)
        .title_style(theme::header_style())
        .border_style(Style::default().fg(theme::ACCENT))
        .style(Style::default().bg(theme::BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let constraints = vec![
        Constraint::Length(2), // Tag
        Constraint::Length(2), // Amount
        Constraint::Length(2), // Period
        Constraint::Min(0),   // errors/help
    ];

    let inner_margin = inner.inner(Margin::new(2, 1));
    let areas = Layout::vertical(constraints).split(inner_margin);

    // Tag
    let tag_display = if form.tag_names.is_empty() {
        "(no tags)".to_string()
    } else {
        format!(
            "{} ({}/{})",
            form.tag_names[form.selected_tag_index],
            form.selected_tag_index + 1,
            form.tag_names.len()
        )
    };
    render_toggle_field(
        frame,
        areas[0],
        "Tag:",
        &tag_display,
        theme::text_style(),
        form.current_field() == BudgetFormField::Tag,
    );

    // Amount
    render_text_field(
        frame,
        areas[1],
        &format!("Amount ({currency}):"),
        &form.amount,
        form.current_field() == BudgetFormField::Amount,
    );

    // Period
    let period_label = match form.period {
        BudgetPeriod::Weekly => "Weekly",
        BudgetPeriod::Monthly => "Monthly",
        BudgetPeriod::Yearly => "Yearly",
    };
    render_toggle_field(
        frame,
        areas[2],
        "Period:",
        period_label,
        theme::text_style(),
        form.current_field() == BudgetFormField::Period,
    );

    // Help + errors
    if areas.len() > 3 {
        let mut display_lines: Vec<Line> = Vec::new();
        for err in &form.errors {
            display_lines.push(Line::from(Span::styled(
                err.as_str(),
                theme::expense_style(),
            )));
        }
        display_lines.push(Line::from(Span::styled(
            "Tab/Shift+Tab: switch fields | Enter: save | Esc: cancel",
            theme::muted_style(),
        )));
        display_lines.push(Line::from(Span::styled(
            "Space: cycle Tag, Period",
            theme::muted_style(),
        )));
        let para = Paragraph::new(display_lines).wrap(Wrap { trim: true });
        frame.render_widget(para, areas[3]);
    }
}

pub fn draw_budget_list(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = theme::styled_block(" Budgets ");
    let currency = &app.config.currency;
    let tsep = &app.config.thousands_separator;
    let dsep = &app.config.decimal_separator;

    if app.budget_spending.is_empty() {
        let para = Paragraph::new(Span::styled(
            "  No budgets configured. Press [a] to add one.",
            theme::muted_style(),
        ))
        .block(block);
        frame.render_widget(para, area);
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Each budget entry gets 4 lines: label, gauge, pace, spacer.
    let budget_count = app.budget_spending.len();
    let mut constraints: Vec<Constraint> = Vec::new();
    for _ in 0..budget_count {
        constraints.push(Constraint::Length(4));
    }
    constraints.push(Constraint::Min(0));

    let rows = Layout::vertical(constraints).split(inner);

    for (i, (budget, spent)) in app.budget_spending.iter().enumerate() {
        if i >= rows.len() - 1 {
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
            theme::expense_style()
        } else if pct >= 80.0 {
            theme::warning_style()
        } else {
            theme::income_style()
        };

        let is_selected = i == app.budget_selected;
        let indicator = if is_selected { "> " } else { "  " };
        let bullet_style = if is_selected { theme::selected_style() } else { theme::text_style() };

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
            Span::styled(indicator, bullet_style),
            Span::styled(
                format!("\u{25cf} {} ({})", tag_name, budget.period),
                bullet_style,
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
}

pub fn draw_budget_footer(frame: &mut Frame, area: ratatui::layout::Rect) {
    let help = Line::from(vec![
        Span::styled(" [a]", theme::header_style()),
        Span::styled("dd ", theme::text_style()),
        Span::styled("[e]", theme::header_style()),
        Span::styled("dit ", theme::text_style()),
        Span::styled("[d]", theme::header_style()),
        Span::styled("elete ", theme::text_style()),
        Span::styled("[Up/Down]", theme::header_style()),
        Span::styled("select ", theme::text_style()),
        Span::styled("[Esc]", theme::header_style()),
        Span::styled("back ", theme::text_style()),
    ]);
    frame.render_widget(Paragraph::new(help), area);
}
