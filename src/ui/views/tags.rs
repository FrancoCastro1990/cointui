use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::ui::theme;
use crate::ui::views::form::{centered_rect, render_text_field};

/// State for the tag add/edit form popup.
#[derive(Debug, Clone, Default)]
pub struct TagForm {
    /// `None` for a new tag; `Some(id)` when editing.
    pub editing_id: Option<i64>,
    pub name: String,
    pub error: Option<String>,
}

impl TagForm {

    pub fn from_existing(id: i64, name: &str) -> Self {
        Self {
            editing_id: Some(id),
            name: name.to_string(),
            error: None,
        }
    }
}

/// State for the tag delete modal with reassignment.
#[derive(Debug, Clone)]
pub struct TagDeleteInfo {
    pub tag_id: i64,
    pub tag_name: String,
    pub transaction_count: usize,
    pub recurring_count: usize,
    /// Index into `available_tags` for the reassignment target.
    pub reassign_tag_index: usize,
    /// Tags available for reassignment (excludes the tag being deleted).
    pub available_tags: Vec<(i64, String)>,
}

/// Draw the main tags view: a table of all tags with keybinding footer.
pub fn draw_tags_view(frame: &mut Frame, app: &mut App, area: Rect) {
    let [table_area, footer_area] = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(area);

    let block = theme::styled_block(" Tags ");

    if app.tags.is_empty() {
        let para = Paragraph::new(Span::styled(
            "  No tags found.",
            theme::muted_style(),
        ))
        .block(block);
        frame.render_widget(para, table_area);
    } else {
        let header = Row::new(vec!["ID", "Name"])
            .style(theme::header_style().add_modifier(Modifier::UNDERLINED))
            .bottom_margin(1);

        let rows: Vec<Row> = app
            .tags
            .iter()
            .map(|tag| {
                Row::new(vec![
                    Cell::from(tag.id.unwrap_or(0).to_string()),
                    Cell::from(tag.name.clone()),
                ])
            })
            .collect();

        let widths = [
            Constraint::Length(6),
            Constraint::Fill(1),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(block)
            .style(theme::text_style())
            .column_spacing(1)
            .row_highlight_style(theme::selected_style())
            .highlight_symbol("> ");

        let mut state = TableState::default();
        if !app.tags.is_empty() {
            state.select(Some(app.tag_selected));
        }

        frame.render_stateful_widget(table, table_area, &mut state);
    }

    // Footer
    let help = Line::from(vec![
        Span::styled(" [a]", theme::header_style()),
        Span::styled("dd ", theme::text_style()),
        Span::styled("[e]", theme::header_style()),
        Span::styled("dit ", theme::text_style()),
        Span::styled("[d]", theme::header_style()),
        Span::styled("elete ", theme::text_style()),
        Span::styled("[Up/Down]", theme::header_style()),
        Span::styled("select ", theme::text_style()),
        Span::styled("[?]", theme::header_style()),
        Span::styled("help", theme::text_style()),
    ]);
    frame.render_widget(Paragraph::new(help), footer_area);
}

/// Draw the tag add/edit form popup.
pub fn draw_tag_form(frame: &mut Frame, form: &TagForm) {
    let area = centered_rect(40, 25, frame.area());
    frame.render_widget(Clear, area);

    let title = if form.editing_id.is_some() {
        " Edit Tag "
    } else {
        " New Tag "
    };

    let block = Block::bordered()
        .title(title)
        .title_style(theme::header_style())
        .border_style(Style::default().fg(theme::ACCENT))
        .style(Style::default().bg(theme::BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let inner_margin = inner.inner(Margin::new(2, 1));
    let constraints = if form.error.is_some() {
        vec![
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(0),
        ]
    } else {
        vec![
            Constraint::Length(2),
            Constraint::Min(0),
        ]
    };
    let areas = Layout::vertical(constraints).split(inner_margin);

    render_text_field(frame, areas[0], "Tag name:", &form.name, true);

    let help_idx = if form.error.is_some() {
        // Show error
        if let Some(ref err) = form.error {
            let err_line = Line::from(Span::styled(err.as_str(), theme::expense_style()));
            frame.render_widget(Paragraph::new(err_line), areas[1]);
        }
        2
    } else {
        1
    };

    if help_idx < areas.len() {
        let help = Line::from(Span::styled(
            "Enter: save | Esc: cancel",
            theme::muted_style(),
        ));
        frame.render_widget(Paragraph::new(help), areas[help_idx]);
    }
}

/// Draw the tag delete modal with reassignment option.
pub fn draw_tag_delete_modal(frame: &mut Frame, info: &TagDeleteInfo) {
    let area = centered_rect(55, 45, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::bordered()
        .title(" Delete Tag ")
        .title_style(theme::warning_style())
        .border_style(Style::default().fg(theme::YELLOW))
        .style(Style::default().bg(theme::BG));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let inner_margin = inner.inner(Margin::new(2, 1));

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("Tag: ", theme::text_style()),
        Span::styled(&info.tag_name, theme::warning_style()),
    ]));
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        format!("  {} transaction(s)", info.transaction_count),
        theme::text_style(),
    )));
    lines.push(Line::from(Span::styled(
        format!("  {} recurring entry(ies)", info.recurring_count),
        theme::text_style(),
    )));
    lines.push(Line::from(""));

    if info.available_tags.is_empty() {
        lines.push(Line::from(Span::styled(
            "No other tags available for reassignment!",
            theme::expense_style(),
        )));
    } else {
        let (_, ref target_name) = info.available_tags[info.reassign_tag_index];
        lines.push(Line::from(vec![
            Span::styled("Reassign to: ", theme::text_style()),
            Span::styled(
                format!(
                    "{} ({}/{})",
                    target_name,
                    info.reassign_tag_index + 1,
                    info.available_tags.len()
                ),
                theme::income_style(),
            ),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("[Space]", theme::header_style()),
        Span::styled(" cycle target  ", theme::text_style()),
        Span::styled("[Enter]", theme::header_style()),
        Span::styled(" confirm  ", theme::text_style()),
        Span::styled("[Esc]", theme::header_style()),
        Span::styled(" cancel", theme::text_style()),
    ]));

    let para = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(para, inner_margin);
}
