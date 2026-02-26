use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::domain::models::{
    format_cents, RecurringEntry, RecurringInterval, TransactionKind,
};
use crate::ui::theme;
use crate::ui::views::form::{centered_rect, render_text_field, render_toggle_field};

// ---------------------------------------------------------------------------
// Recurring form
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurringFormField {
    Source,
    Amount,
    Kind,
    Tag,
    Interval,
    Day,
    Month,
}

const ALL_FIELDS: &[RecurringFormField] = &[
    RecurringFormField::Source,
    RecurringFormField::Amount,
    RecurringFormField::Kind,
    RecurringFormField::Tag,
    RecurringFormField::Interval,
    RecurringFormField::Day,
    RecurringFormField::Month,
];

/// State for the recurring entry add/edit form.
#[derive(Debug, Clone)]
pub struct RecurringForm {
    pub editing_id: Option<i64>,
    pub source: String,
    pub amount: String,
    pub kind: TransactionKind,
    pub selected_tag_index: usize,
    pub tag_names: Vec<String>,
    pub tag_ids: Vec<i64>,
    pub interval: RecurringInterval,
    pub day_of_month: String,
    pub month: String,
    pub field_index: usize,
    pub errors: Vec<String>,
}

impl RecurringForm {
    /// Create a blank form for adding a new recurring entry.
    pub fn new(tag_names: Vec<String>, tag_ids: Vec<i64>) -> Self {
        Self {
            editing_id: None,
            source: String::new(),
            amount: String::new(),
            kind: TransactionKind::Expense,
            selected_tag_index: 0,
            tag_names,
            tag_ids,
            interval: RecurringInterval::Monthly,
            day_of_month: "1".to_string(),
            month: "1".to_string(),
            field_index: 0,
            errors: Vec::new(),
        }
    }

    /// Create a form pre-filled with an existing recurring entry for editing.
    pub fn from_recurring(
        entry: &RecurringEntry,
        tag_names: Vec<String>,
        tag_ids: Vec<i64>,
    ) -> Self {
        let selected_tag_index = tag_ids
            .iter()
            .position(|&id| id == entry.tag_id)
            .unwrap_or(0);

        Self {
            editing_id: entry.id,
            source: entry.source.clone(),
            amount: entry.amount.to_string(),
            kind: entry.kind,
            selected_tag_index,
            tag_names,
            tag_ids,
            interval: entry.interval,
            day_of_month: entry.day_of_month.unwrap_or(1).to_string(),
            month: entry.month.unwrap_or(1).to_string(),
            field_index: 0,
            errors: Vec::new(),
        }
    }

    /// Returns the active fields based on the current interval.
    fn active_fields(&self) -> Vec<RecurringFormField> {
        ALL_FIELDS
            .iter()
            .copied()
            .filter(|f| match f {
                RecurringFormField::Day => matches!(
                    self.interval,
                    RecurringInterval::Monthly | RecurringInterval::Yearly
                ),
                RecurringFormField::Month => {
                    matches!(self.interval, RecurringInterval::Yearly)
                }
                _ => true,
            })
            .collect()
    }

    pub fn current_field(&self) -> RecurringFormField {
        let fields = self.active_fields();
        fields[self.field_index.min(fields.len() - 1)]
    }

    pub fn next_field(&mut self) {
        let fields = self.active_fields();
        self.field_index = (self.field_index + 1) % fields.len();
    }

    pub fn prev_field(&mut self) {
        let fields = self.active_fields();
        if self.field_index == 0 {
            self.field_index = fields.len() - 1;
        } else {
            self.field_index -= 1;
        }
    }

    pub fn type_char(&mut self, c: char) {
        match self.current_field() {
            RecurringFormField::Source => self.source.push(c),
            RecurringFormField::Amount => {
                if c.is_ascii_digit() || (c == '.' && !self.amount.contains('.')) {
                    self.amount.push(c);
                }
            }
            RecurringFormField::Day => {
                if c.is_ascii_digit() && self.day_of_month.len() < 2 {
                    self.day_of_month.push(c);
                }
            }
            RecurringFormField::Month => {
                if c.is_ascii_digit() && self.month.len() < 2 {
                    self.month.push(c);
                }
            }
            _ => {}
        }
    }

    pub fn backspace(&mut self) {
        match self.current_field() {
            RecurringFormField::Source => { self.source.pop(); }
            RecurringFormField::Amount => { self.amount.pop(); }
            RecurringFormField::Day => { self.day_of_month.pop(); }
            RecurringFormField::Month => { self.month.pop(); }
            _ => {}
        }
    }

    pub fn cycle_option(&mut self) {
        match self.current_field() {
            RecurringFormField::Kind => {
                self.kind = match self.kind {
                    TransactionKind::Income => TransactionKind::Expense,
                    TransactionKind::Expense => TransactionKind::Income,
                };
            }
            RecurringFormField::Tag => {
                if !self.tag_names.is_empty() {
                    self.selected_tag_index =
                        (self.selected_tag_index + 1) % self.tag_names.len();
                }
            }
            RecurringFormField::Interval => {
                self.interval = match self.interval {
                    RecurringInterval::Daily => RecurringInterval::Weekly,
                    RecurringInterval::Weekly => RecurringInterval::Monthly,
                    RecurringInterval::Monthly => RecurringInterval::Yearly,
                    RecurringInterval::Yearly => RecurringInterval::Daily,
                };
                // Clamp field_index if active fields changed.
                let fields = self.active_fields();
                if self.field_index >= fields.len() {
                    self.field_index = fields.len() - 1;
                }
            }
            _ => {}
        }
    }

    /// Validate and convert form data to a RecurringEntry.
    pub fn to_recurring(&mut self) -> Result<RecurringEntry, Vec<String>> {
        self.errors.clear();

        if self.source.trim().is_empty() {
            self.errors.push("Source is required.".into());
        }

        let amount = match self.parse_amount() {
            Some(a) if a > 0 => a,
            _ => {
                self.errors.push("Amount must be a positive number.".into());
                0
            }
        };

        if self.tag_ids.is_empty() {
            self.errors.push("No tags available.".into());
        }

        let (day_of_month, month) = match self.interval {
            RecurringInterval::Daily | RecurringInterval::Weekly => (None, None),
            RecurringInterval::Monthly => {
                let day = self.parse_day();
                if day.is_none() {
                    self.errors.push("Day must be 1-31.".into());
                }
                (day, None)
            }
            RecurringInterval::Yearly => {
                let day = self.parse_day();
                let m = self.parse_month();
                if day.is_none() {
                    self.errors.push("Day must be 1-31.".into());
                }
                if m.is_none() {
                    self.errors.push("Month must be 1-12.".into());
                }
                (day, m)
            }
        };

        if !self.errors.is_empty() {
            return Err(self.errors.clone());
        }

        let tag_id = self.tag_ids[self.selected_tag_index];

        Ok(RecurringEntry {
            id: self.editing_id,
            source: self.source.trim().to_string(),
            amount,
            kind: self.kind,
            tag_id,
            interval: self.interval,
            day_of_month,
            month,
            last_inserted_date: None,
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

    fn parse_day(&self) -> Option<u32> {
        let d: u32 = self.day_of_month.trim().parse().ok()?;
        if (1..=31).contains(&d) { Some(d) } else { None }
    }

    fn parse_month(&self) -> Option<u32> {
        let m: u32 = self.month.trim().parse().ok()?;
        if (1..=12).contains(&m) { Some(m) } else { None }
    }
}

// ---------------------------------------------------------------------------
// Recurring form draw
// ---------------------------------------------------------------------------

pub fn draw_recurring_form(frame: &mut Frame, form: &RecurringForm, currency: &str) {
    let area = centered_rect(60, 65, frame.area());

    frame.render_widget(Clear, area);

    let title = if form.editing_id.is_some() {
        " Edit Recurring Entry "
    } else {
        " New Recurring Entry "
    };

    let block = Block::bordered()
        .title(title)
        .title_style(theme::header_style())
        .border_style(Style::default().fg(theme::ACCENT))
        .style(Style::default().bg(theme::BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build dynamic constraints based on active fields.
    let show_day = matches!(
        form.interval,
        RecurringInterval::Monthly | RecurringInterval::Yearly
    );
    let show_month = matches!(form.interval, RecurringInterval::Yearly);

    let mut constraints: Vec<Constraint> = vec![
        Constraint::Length(2), // Source
        Constraint::Length(2), // Amount
        Constraint::Length(2), // Kind
        Constraint::Length(2), // Tag
        Constraint::Length(2), // Interval
    ];
    if show_day {
        constraints.push(Constraint::Length(2)); // Day
    }
    if show_month {
        constraints.push(Constraint::Length(2)); // Month
    }
    constraints.push(Constraint::Min(0)); // errors/help

    let inner_margin = inner.inner(Margin::new(2, 1));
    let areas = Layout::vertical(constraints).split(inner_margin);

    let mut row = 0usize;

    // Source
    render_text_field(
        frame,
        areas[row],
        "Source:",
        &form.source,
        form.current_field() == RecurringFormField::Source,
    );
    row += 1;

    // Amount
    render_text_field(
        frame,
        areas[row],
        &format!("Amount ({currency}):"),
        &form.amount,
        form.current_field() == RecurringFormField::Amount,
    );
    row += 1;

    // Kind
    let kind_label = match form.kind {
        TransactionKind::Income => "Income",
        TransactionKind::Expense => "Expense",
    };
    let kind_style = match form.kind {
        TransactionKind::Income => theme::income_style(),
        TransactionKind::Expense => theme::expense_style(),
    };
    render_toggle_field(
        frame,
        areas[row],
        "Type:",
        kind_label,
        kind_style,
        form.current_field() == RecurringFormField::Kind,
    );
    row += 1;

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
        areas[row],
        "Tag:",
        &tag_display,
        theme::text_style(),
        form.current_field() == RecurringFormField::Tag,
    );
    row += 1;

    // Interval
    let interval_label = match form.interval {
        RecurringInterval::Daily => "Daily",
        RecurringInterval::Weekly => "Weekly",
        RecurringInterval::Monthly => "Monthly",
        RecurringInterval::Yearly => "Yearly",
    };
    render_toggle_field(
        frame,
        areas[row],
        "Interval:",
        interval_label,
        theme::text_style(),
        form.current_field() == RecurringFormField::Interval,
    );
    row += 1;

    // Day (conditional)
    if show_day {
        render_text_field(
            frame,
            areas[row],
            "Day of month (1-31):",
            &form.day_of_month,
            form.current_field() == RecurringFormField::Day,
        );
        row += 1;
    }

    // Month (conditional)
    if show_month {
        render_text_field(
            frame,
            areas[row],
            "Month (1-12):",
            &form.month,
            form.current_field() == RecurringFormField::Month,
        );
        row += 1;
    }

    // Help + errors
    if row < areas.len() {
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
            "Space: cycle Type, Tag, Interval",
            theme::muted_style(),
        )));
        let para = Paragraph::new(display_lines).wrap(Wrap { trim: true });
        frame.render_widget(para, areas[row]);
    }
}

// ---------------------------------------------------------------------------
// Table view (used from ui/mod.rs draw_recurring_view)
// ---------------------------------------------------------------------------

/// Format the interval display with schedule details.
fn interval_display(entry: &RecurringEntry) -> String {
    const MONTH_ABBRS: &[&str] = &[
        "", "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    match entry.interval {
        RecurringInterval::Daily => "Daily".to_string(),
        RecurringInterval::Weekly => "Weekly".to_string(),
        RecurringInterval::Monthly => {
            if let Some(d) = entry.day_of_month {
                format!("Monthly ({})", d)
            } else {
                "Monthly".to_string()
            }
        }
        RecurringInterval::Yearly => {
            match (entry.month, entry.day_of_month) {
                (Some(m), Some(d)) => {
                    let abbr = MONTH_ABBRS.get(m as usize).unwrap_or(&"?");
                    format!("Yearly ({} {})", abbr, d)
                }
                _ => "Yearly".to_string(),
            }
        }
    }
}

pub fn draw_recurring(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let [table_area, footer_area] = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(area);

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
            "  No recurring entries. Press 'a' to add one.",
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
                    format_cents(entry.amount, currency, tsep, dsep),
                    amount_style,
                )),
                Cell::from(Span::styled(kind_str, amount_style)),
                Cell::from(interval_display(entry)),
                Cell::from(tag_name),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(6),
        Constraint::Fill(1),
        Constraint::Length(16),
        Constraint::Length(5),
        Constraint::Length(16),
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
        Span::styled(" [a]", theme::header_style()),
        Span::styled("dd ", theme::text_style()),
        Span::styled("[e]", theme::header_style()),
        Span::styled("dit ", theme::text_style()),
        Span::styled("[Space]", theme::header_style()),
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
