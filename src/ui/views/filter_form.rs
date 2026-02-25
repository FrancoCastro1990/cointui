use chrono::NaiveDate;
use ratatui::layout::{Constraint, Layout, Margin};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::db::transaction_repo::TransactionFilter;
use crate::domain::models::TransactionKind;
use crate::ui::theme;
use crate::ui::views::form::{centered_rect, render_text_field, render_toggle_field};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterField {
    Search,
    DateFrom,
    DateTo,
    Tag,
    Kind,
    MinAmount,
    MaxAmount,
}

const FILTER_FIELD_ORDER: &[FilterField] = &[
    FilterField::Search,
    FilterField::DateFrom,
    FilterField::DateTo,
    FilterField::Tag,
    FilterField::Kind,
    FilterField::MinAmount,
    FilterField::MaxAmount,
];

#[derive(Debug, Clone)]
pub struct FilterForm {
    pub search: String,
    pub date_from: String,
    pub date_to: String,
    pub selected_tag_index: Option<usize>,
    /// 0=All, 1=Income, 2=Expense
    pub kind_option: usize,
    pub min_amount: String,
    pub max_amount: String,
    pub field_index: usize,
    pub tag_names: Vec<String>,
    pub tag_ids: Vec<i64>,
}

impl FilterForm {
    pub fn new(tag_names: Vec<String>, tag_ids: Vec<i64>) -> Self {
        Self {
            search: String::new(),
            date_from: String::new(),
            date_to: String::new(),
            selected_tag_index: None,
            kind_option: 0,
            min_amount: String::new(),
            max_amount: String::new(),
            field_index: 0,
            tag_names,
            tag_ids,
        }
    }

    pub fn from_filter(
        filter: &TransactionFilter,
        tag_names: Vec<String>,
        tag_ids: Vec<i64>,
    ) -> Self {
        let selected_tag_index = filter
            .tag_id
            .and_then(|tid| tag_ids.iter().position(|&id| id == tid));

        let kind_option = match filter.kind {
            None => 0,
            Some(TransactionKind::Income) => 1,
            Some(TransactionKind::Expense) => 2,
        };

        Self {
            search: filter.search.clone().unwrap_or_default(),
            date_from: filter
                .date_from
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default(),
            date_to: filter
                .date_to
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default(),
            selected_tag_index,
            kind_option,
            min_amount: filter
                .min_amount
                .map(|a| a.to_string())
                .unwrap_or_default(),
            max_amount: filter
                .max_amount
                .map(|a| a.to_string())
                .unwrap_or_default(),
            field_index: 0,
            tag_names,
            tag_ids,
        }
    }

    pub fn to_filter(&self) -> TransactionFilter {
        let search = if self.search.trim().is_empty() {
            None
        } else {
            Some(self.search.trim().to_string())
        };

        let date_from = NaiveDate::parse_from_str(self.date_from.trim(), "%Y-%m-%d").ok();
        let date_to = NaiveDate::parse_from_str(self.date_to.trim(), "%Y-%m-%d").ok();

        let kind = match self.kind_option {
            1 => Some(TransactionKind::Income),
            2 => Some(TransactionKind::Expense),
            _ => None,
        };

        let tag_id = self
            .selected_tag_index
            .and_then(|i| self.tag_ids.get(i).copied());

        let min_amount = self.parse_amount(&self.min_amount);
        let max_amount = self.parse_amount(&self.max_amount);

        TransactionFilter {
            search,
            date_from,
            date_to,
            kind,
            tag_id,
            min_amount,
            max_amount,
        }
    }

    pub fn current_field(&self) -> FilterField {
        FILTER_FIELD_ORDER[self.field_index]
    }

    pub fn next_field(&mut self) {
        self.field_index = (self.field_index + 1) % FILTER_FIELD_ORDER.len();
    }

    pub fn prev_field(&mut self) {
        if self.field_index == 0 {
            self.field_index = FILTER_FIELD_ORDER.len() - 1;
        } else {
            self.field_index -= 1;
        }
    }

    pub fn type_char(&mut self, c: char) {
        match self.current_field() {
            FilterField::Search => self.search.push(c),
            FilterField::DateFrom => {
                if c.is_ascii_digit() || c == '-' {
                    self.date_from.push(c);
                }
            }
            FilterField::DateTo => {
                if c.is_ascii_digit() || c == '-' {
                    self.date_to.push(c);
                }
            }
            FilterField::MinAmount => {
                if c.is_ascii_digit() || (c == '.' && !self.min_amount.contains('.')) {
                    self.min_amount.push(c);
                }
            }
            FilterField::MaxAmount => {
                if c.is_ascii_digit() || (c == '.' && !self.max_amount.contains('.')) {
                    self.max_amount.push(c);
                }
            }
            _ => {}
        }
    }

    pub fn backspace(&mut self) {
        match self.current_field() {
            FilterField::Search => { self.search.pop(); }
            FilterField::DateFrom => { self.date_from.pop(); }
            FilterField::DateTo => { self.date_to.pop(); }
            FilterField::MinAmount => { self.min_amount.pop(); }
            FilterField::MaxAmount => { self.max_amount.pop(); }
            _ => {}
        }
    }

    pub fn cycle_option(&mut self) {
        match self.current_field() {
            FilterField::Kind => {
                self.kind_option = (self.kind_option + 1) % 3;
            }
            FilterField::Tag => {
                if self.tag_names.is_empty() {
                    return;
                }
                self.selected_tag_index = match self.selected_tag_index {
                    None => Some(0),
                    Some(i) if i + 1 >= self.tag_names.len() => None,
                    Some(i) => Some(i + 1),
                };
            }
            _ => {}
        }
    }

    fn parse_amount(&self, s: &str) -> Option<i64> {
        let trimmed = s.trim();
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

pub fn draw_filter_form(frame: &mut Frame, form: &FilterForm, currency: &str) {
    let area = centered_rect(60, 70, frame.area());

    frame.render_widget(Clear, area);

    let block = Block::bordered()
        .title(" Filter Transactions ")
        .title_style(theme::header_style())
        .border_style(ratatui::style::Style::default().fg(theme::ACCENT))
        .style(ratatui::style::Style::default().bg(theme::BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut constraints: Vec<Constraint> = Vec::new();
    for _ in 0..7 {
        constraints.push(Constraint::Length(2));
    }
    constraints.push(Constraint::Min(0));

    let inner_margin = inner.inner(Margin::new(2, 1));
    let areas = Layout::vertical(constraints).split(inner_margin);

    let mut row = 0usize;

    // Search
    render_text_field(
        frame,
        areas[row],
        "Search (source/notes):",
        &form.search,
        form.current_field() == FilterField::Search,
    );
    row += 1;

    // Date From
    render_text_field(
        frame,
        areas[row],
        "Date from (YYYY-MM-DD):",
        &form.date_from,
        form.current_field() == FilterField::DateFrom,
    );
    row += 1;

    // Date To
    render_text_field(
        frame,
        areas[row],
        "Date to (YYYY-MM-DD):",
        &form.date_to,
        form.current_field() == FilterField::DateTo,
    );
    row += 1;

    // Tag (cycle)
    let tag_display = match form.selected_tag_index {
        None => "All".to_string(),
        Some(i) => format!(
            "{} ({}/{})",
            form.tag_names[i],
            i + 1,
            form.tag_names.len()
        ),
    };
    render_toggle_field(
        frame,
        areas[row],
        "Tag:",
        &tag_display,
        theme::text_style(),
        form.current_field() == FilterField::Tag,
    );
    row += 1;

    // Kind (cycle)
    let kind_display = match form.kind_option {
        0 => "All",
        1 => "Income",
        2 => "Expense",
        _ => "All",
    };
    let kind_style = match form.kind_option {
        1 => theme::income_style(),
        2 => theme::expense_style(),
        _ => theme::text_style(),
    };
    render_toggle_field(
        frame,
        areas[row],
        "Type:",
        kind_display,
        kind_style,
        form.current_field() == FilterField::Kind,
    );
    row += 1;

    // Min Amount
    render_text_field(
        frame,
        areas[row],
        &format!("Min amount ({currency}):"),
        &form.min_amount,
        form.current_field() == FilterField::MinAmount,
    );
    row += 1;

    // Max Amount
    render_text_field(
        frame,
        areas[row],
        &format!("Max amount ({currency}):"),
        &form.max_amount,
        form.current_field() == FilterField::MaxAmount,
    );
    row += 1;

    // Help text
    if row < areas.len() {
        let help_lines = vec![
            Line::from(Span::styled(
                "Tab/Shift+Tab: switch fields | Enter: apply | Esc: cancel",
                theme::muted_style(),
            )),
            Line::from(Span::styled(
                "Space: cycle on Tag, Type | Leave blank to skip filter",
                theme::muted_style(),
            )),
        ];
        let para = Paragraph::new(help_lines).wrap(Wrap { trim: true });
        frame.render_widget(para, areas[row]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tag_data() -> (Vec<String>, Vec<i64>) {
        (
            vec!["Comida".into(), "Transporte".into(), "Otros".into()],
            vec![1, 2, 3],
        )
    }

    #[test]
    fn filter_form_from_filter_roundtrip() {
        let (names, ids) = sample_tag_data();

        let original = TransactionFilter {
            search: Some("grocery".into()),
            date_from: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            date_to: Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            kind: Some(TransactionKind::Expense),
            tag_id: Some(2),
            min_amount: Some(1000),
            max_amount: Some(50000),
        };

        let form = FilterForm::from_filter(&original, names, ids);
        let result = form.to_filter();

        assert_eq!(result.search, original.search);
        assert_eq!(result.date_from, original.date_from);
        assert_eq!(result.date_to, original.date_to);
        assert_eq!(result.kind, original.kind);
        assert_eq!(result.tag_id, original.tag_id);
        assert_eq!(result.min_amount, original.min_amount);
        assert_eq!(result.max_amount, original.max_amount);
    }

    #[test]
    fn filter_form_field_navigation() {
        let (names, ids) = sample_tag_data();
        let mut form = FilterForm::new(names, ids);

        assert_eq!(form.current_field(), FilterField::Search);

        form.next_field();
        assert_eq!(form.current_field(), FilterField::DateFrom);

        form.next_field();
        assert_eq!(form.current_field(), FilterField::DateTo);

        form.prev_field();
        assert_eq!(form.current_field(), FilterField::DateFrom);

        // Wrap around forward
        form.field_index = FILTER_FIELD_ORDER.len() - 1;
        assert_eq!(form.current_field(), FilterField::MaxAmount);
        form.next_field();
        assert_eq!(form.current_field(), FilterField::Search);

        // Wrap around backward
        form.prev_field();
        assert_eq!(form.current_field(), FilterField::MaxAmount);
    }

    #[test]
    fn filter_form_empty_produces_default_filter() {
        let (names, ids) = sample_tag_data();
        let form = FilterForm::new(names, ids);
        let filter = form.to_filter();

        assert!(filter.search.is_none());
        assert!(filter.date_from.is_none());
        assert!(filter.date_to.is_none());
        assert!(filter.kind.is_none());
        assert!(filter.tag_id.is_none());
        assert!(filter.min_amount.is_none());
        assert!(filter.max_amount.is_none());
    }

    #[test]
    fn filter_form_tag_cycle() {
        let (names, ids) = sample_tag_data();
        let mut form = FilterForm::new(names, ids);
        form.field_index = 3; // Tag field

        assert_eq!(form.selected_tag_index, None);

        form.cycle_option();
        assert_eq!(form.selected_tag_index, Some(0));

        form.cycle_option();
        assert_eq!(form.selected_tag_index, Some(1));

        form.cycle_option();
        assert_eq!(form.selected_tag_index, Some(2));

        // Wraps back to None (All)
        form.cycle_option();
        assert_eq!(form.selected_tag_index, None);
    }
}
