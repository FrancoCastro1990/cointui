use chrono::NaiveDate;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::domain::models::{RecurringInterval, Transaction, TransactionKind};
use crate::ui::theme;

/// Which form field is currently focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    Source,
    Amount,
    Date,
    Kind,
    Tag,
    Notes,
    Recurring,
    Interval,
}

const FIELD_ORDER: &[FormField] = &[
    FormField::Source,
    FormField::Amount,
    FormField::Date,
    FormField::Kind,
    FormField::Tag,
    FormField::Notes,
    FormField::Recurring,
    FormField::Interval,
];

/// State for the transaction add/edit form.
#[derive(Debug, Clone)]
pub struct TransactionForm {
    /// `None` for a new transaction; `Some(id)` when editing.
    pub editing_id: Option<i64>,
    pub source: String,
    pub amount: String,
    pub date: String,
    pub kind: TransactionKind,
    pub selected_tag_index: usize,
    pub tag_names: Vec<String>,
    pub tag_ids: Vec<i64>,
    pub notes: String,
    pub recurring: bool,
    pub interval: RecurringInterval,
    pub field_index: usize,
    pub errors: Vec<String>,
}

impl TransactionForm {
    /// Create a blank form for adding a new transaction.
    pub fn new(tag_names: Vec<String>, tag_ids: Vec<i64>, today: &str) -> Self {
        Self {
            editing_id: None,
            source: String::new(),
            amount: String::new(),
            date: today.to_string(),
            kind: TransactionKind::Expense,
            selected_tag_index: 0,
            tag_names,
            tag_ids,
            notes: String::new(),
            recurring: false,
            interval: RecurringInterval::Monthly,
            field_index: 0,
            errors: Vec::new(),
        }
    }

    /// Create a form pre-filled with an existing transaction for editing.
    pub fn from_transaction(
        tx: &Transaction,
        tag_names: Vec<String>,
        tag_ids: Vec<i64>,
    ) -> Self {
        let selected_tag_index = tag_ids
            .iter()
            .position(|&id| id == tx.tag_id)
            .unwrap_or(0);

        Self {
            editing_id: tx.id,
            source: tx.source.clone(),
            amount: tx.amount.to_string(),
            date: tx.date.format("%Y-%m-%d").to_string(),
            kind: tx.kind,
            selected_tag_index,
            tag_names,
            tag_ids,
            notes: tx.notes.clone().unwrap_or_default(),
            recurring: false,
            interval: RecurringInterval::Monthly,
            field_index: 0,
            errors: Vec::new(),
        }
    }

    /// Returns the currently focused field.
    pub fn current_field(&self) -> FormField {
        FIELD_ORDER[self.field_index]
    }

    /// Move focus to the next field.
    pub fn next_field(&mut self) {
        let max = if self.recurring {
            FIELD_ORDER.len()
        } else {
            // Skip the Interval field when recurring is off.
            FIELD_ORDER.len() - 1
        };
        self.field_index = (self.field_index + 1) % max;
    }

    /// Move focus to the previous field.
    pub fn prev_field(&mut self) {
        let max = if self.recurring {
            FIELD_ORDER.len()
        } else {
            FIELD_ORDER.len() - 1
        };
        if self.field_index == 0 {
            self.field_index = max - 1;
        } else {
            self.field_index -= 1;
        }
    }

    /// Handle a character typed into the currently focused text field.
    pub fn type_char(&mut self, c: char) {
        match self.current_field() {
            FormField::Source => self.source.push(c),
            FormField::Amount => {
                // Only allow digits and a single decimal point.
                if c.is_ascii_digit() || (c == '.' && !self.amount.contains('.')) {
                    self.amount.push(c);
                }
            }
            FormField::Date => {
                if c.is_ascii_digit() || c == '-' {
                    self.date.push(c);
                }
            }
            FormField::Notes => self.notes.push(c),
            _ => {}
        }
    }

    /// Handle backspace in the currently focused text field.
    pub fn backspace(&mut self) {
        match self.current_field() {
            FormField::Source => { self.source.pop(); }
            FormField::Amount => { self.amount.pop(); }
            FormField::Date => { self.date.pop(); }
            FormField::Notes => { self.notes.pop(); }
            _ => {}
        }
    }

    /// Toggle a boolean/enum field, or cycle through options.
    pub fn toggle_field(&mut self) {
        match self.current_field() {
            FormField::Kind => {
                self.kind = match self.kind {
                    TransactionKind::Income => TransactionKind::Expense,
                    TransactionKind::Expense => TransactionKind::Income,
                };
            }
            FormField::Recurring => {
                self.recurring = !self.recurring;
            }
            _ => {}
        }
    }

    /// Cycle through list options (tags, intervals).
    pub fn cycle_option(&mut self) {
        match self.current_field() {
            FormField::Tag => {
                if !self.tag_names.is_empty() {
                    self.selected_tag_index =
                        (self.selected_tag_index + 1) % self.tag_names.len();
                }
            }
            FormField::Interval => {
                self.interval = match self.interval {
                    RecurringInterval::Daily => RecurringInterval::Weekly,
                    RecurringInterval::Weekly => RecurringInterval::Monthly,
                    RecurringInterval::Monthly => RecurringInterval::Yearly,
                    RecurringInterval::Yearly => RecurringInterval::Daily,
                };
            }
            FormField::Kind => {
                self.toggle_field();
            }
            _ => {}
        }
    }

    /// Validate and convert form data to a Transaction. Returns Err with messages
    /// if validation fails.
    pub fn to_transaction(&mut self) -> Result<Transaction, Vec<String>> {
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

        let date = match NaiveDate::parse_from_str(&self.date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                self.errors
                    .push("Date must be YYYY-MM-DD format.".into());
                chrono::Local::now().date_naive()
            }
        };

        if self.tag_ids.is_empty() {
            self.errors.push("No tags available.".into());
        }

        if !self.errors.is_empty() {
            return Err(self.errors.clone());
        }

        let tag_id = self.tag_ids[self.selected_tag_index];
        let notes = if self.notes.trim().is_empty() {
            None
        } else {
            Some(self.notes.trim().to_string())
        };

        Ok(Transaction {
            id: self.editing_id,
            source: self.source.trim().to_string(),
            amount,
            kind: self.kind,
            tag_id,
            date,
            notes,
            created_at: None,
            updated_at: None,
        })
    }

    /// Parse the amount string to cents.
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

    /// Get the interval if recurring is enabled.
    pub fn get_interval(&self) -> Option<RecurringInterval> {
        if self.recurring {
            Some(self.interval)
        } else {
            None
        }
    }
}

/// Calculate a centered popup rectangle.
pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ]);
    let [_, vertical_center, _] = popup_layout.areas(area);

    let popup_layout = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ]);
    let [_, horizontal_center, _] = popup_layout.areas(vertical_center);

    horizontal_center
}

/// Render the transaction form as a popup overlay.
pub fn draw_form(frame: &mut Frame, form: &TransactionForm, currency: &str) {
    let area = centered_rect(60, 75, frame.area());

    // Clear the area behind the popup.
    frame.render_widget(Clear, area);

    let title = if form.editing_id.is_some() {
        " Edit Transaction "
    } else {
        " New Transaction "
    };

    let block = Block::bordered()
        .title(title)
        .title_style(theme::header_style())
        .border_style(Style::default().fg(theme::ACCENT))
        .style(Style::default().bg(theme::BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Determine how many fields to show.
    let field_count: u16 = if form.recurring { 10 } else { 9 };
    let mut constraints: Vec<Constraint> = Vec::new();
    // Each field row gets 2 lines (label + input), plus spacing.
    for _ in 0..field_count {
        constraints.push(Constraint::Length(2));
    }
    constraints.push(Constraint::Min(0)); // remaining space for errors/help

    let inner_margin = inner.inner(Margin::new(2, 1));
    let areas = Layout::vertical(constraints).split(inner_margin);

    let mut row = 0usize;

    // Source
    render_text_field(
        frame,
        areas[row],
        "Source:",
        &form.source,
        form.current_field() == FormField::Source,
    );
    row += 1;

    // Amount
    render_text_field(
        frame,
        areas[row],
        &format!("Amount ({currency}):"),
        &form.amount,
        form.current_field() == FormField::Amount,
    );
    row += 1;

    // Date
    render_text_field(
        frame,
        areas[row],
        "Date (YYYY-MM-DD):",
        &form.date,
        form.current_field() == FormField::Date,
    );
    row += 1;

    // Kind (toggle)
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
        form.current_field() == FormField::Kind,
    );
    row += 1;

    // Tag (select)
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
        form.current_field() == FormField::Tag,
    );
    row += 1;

    // Notes
    render_text_field(
        frame,
        areas[row],
        "Notes:",
        &form.notes,
        form.current_field() == FormField::Notes,
    );
    row += 1;

    // Recurring toggle
    let rec_label = if form.recurring { "Yes" } else { "No" };
    render_toggle_field(
        frame,
        areas[row],
        "Recurring:",
        rec_label,
        theme::text_style(),
        form.current_field() == FormField::Recurring,
    );
    row += 1;

    // Interval (only if recurring)
    if form.recurring {
        render_toggle_field(
            frame,
            areas[row],
            "Interval:",
            &form.interval.to_string(),
            theme::text_style(),
            form.current_field() == FormField::Interval,
        );
        row += 1;
    }

    // Help text
    let help_lines = vec![
        Line::from(Span::styled(
            "Tab/Shift+Tab: switch fields | Enter: save | Esc: cancel",
            theme::muted_style(),
        )),
        Line::from(Span::styled(
            "Space: toggle/cycle on Type, Tag, Recurring, Interval",
            theme::muted_style(),
        )),
    ];

    // Errors
    let error_idx = row;
    if error_idx < areas.len() {
        let mut display_lines = Vec::new();
        for err in &form.errors {
            display_lines.push(Line::from(Span::styled(
                err.as_str(),
                theme::expense_style(),
            )));
        }
        display_lines.extend(help_lines);
        let error_para = Paragraph::new(display_lines).wrap(Wrap { trim: true });
        frame.render_widget(error_para, areas[error_idx]);
    }
}

pub(crate) fn render_text_field(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    focused: bool,
) {
    let [label_area, input_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);

    let label_style = if focused {
        theme::focused_field_style()
    } else {
        theme::unfocused_field_style()
    };

    let indicator = if focused { "> " } else { "  " };
    let label_line = Line::from(vec![
        Span::styled(indicator, label_style),
        Span::styled(label, label_style),
    ]);
    frame.render_widget(Paragraph::new(label_line), label_area);

    let display = if focused {
        format!("  {}_", value)
    } else {
        format!("  {}", value)
    };
    let input_style = if focused {
        Style::default().fg(theme::FG).add_modifier(Modifier::UNDERLINED)
    } else {
        theme::text_style()
    };
    frame.render_widget(
        Paragraph::new(Span::styled(display, input_style)),
        input_area,
    );
}

pub(crate) fn render_toggle_field(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    value_style: Style,
    focused: bool,
) {
    let [label_area, input_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);

    let label_style = if focused {
        theme::focused_field_style()
    } else {
        theme::unfocused_field_style()
    };

    let indicator = if focused { "> " } else { "  " };
    let label_line = Line::from(vec![
        Span::styled(indicator, label_style),
        Span::styled(label, label_style),
    ]);
    frame.render_widget(Paragraph::new(label_line), label_area);

    let brackets = if focused { "[ " } else { "  " };
    let brackets_end = if focused { " ]" } else { "" };
    let input_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(brackets, theme::muted_style()),
        Span::styled(value, value_style),
        Span::styled(brackets_end, theme::muted_style()),
        Span::styled(
            if focused { " (Space to change)" } else { "" },
            theme::muted_style(),
        ),
    ]);
    frame.render_widget(Paragraph::new(input_line), input_area);
}
