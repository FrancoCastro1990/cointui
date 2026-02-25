pub mod theme;
pub mod views;

use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Tabs};
use ratatui::Frame;

use crate::app::{App, Mode, SortColumn, SortDirection, View};
use crate::ui::views::filter_form::draw_filter_form;
use crate::ui::views::form::draw_form;
use crate::ui::views::help::draw_help;
use crate::ui::views::tags::{draw_tag_delete_modal, draw_tag_form};

/// Main draw function: dispatches to the current view's renderer.
pub fn draw(frame: &mut Frame, app: &mut App) {
    // Global layout: tab bar at top, content in middle, status at bottom.
    let [tabs_area, content_area, status_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    draw_tabs(frame, app, tabs_area);

    match app.current_view {
        View::Dashboard => draw_dashboard(frame, app, content_area),
        View::Transactions => draw_transactions_view(frame, app, content_area),
        View::Stats => draw_stats(frame, app, content_area),
        View::Budgets => draw_budgets(frame, app, content_area),
        View::Recurring => draw_recurring_view(frame, app, content_area),
        View::Tags => views::tags::draw_tags_view(frame, app, content_area),
    }

    // Draw the form overlay if in Adding or Editing mode.
    if matches!(app.mode, Mode::Adding | Mode::Editing)
        && let Some(ref form) = app.form {
            draw_form(frame, form, &app.config.currency);
        }

    // Draw the filter form overlay.
    if matches!(app.mode, Mode::Filtering)
        && let Some(ref filter_form) = app.filter_form {
            draw_filter_form(frame, filter_form, &app.config.currency);
        }

    // Draw tag form overlay.
    if matches!(app.mode, Mode::TagEditing)
        && let Some(ref form) = app.tag_form {
            draw_tag_form(frame, form);
        }

    // Draw tag delete modal overlay.
    if matches!(app.mode, Mode::TagDeleting)
        && let Some(ref info) = app.tag_delete_info {
            draw_tag_delete_modal(frame, info);
        }

    // Draw help overlay.
    if matches!(app.mode, Mode::Help) {
        draw_help(frame, app.current_view);
    }

    // Draw confirmation dialog if confirming.
    if let Mode::Confirming(ref msg) = app.mode {
        draw_confirm(frame, msg);
    }

    // Draw status message.
    draw_status(frame, app, status_area);
}

/// Wrapper to handle the mutable borrow needed for transactions view.
fn draw_transactions_view(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    // Re-layout within the content area for the transactions view.
    let [filter_area, table_area, footer_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(area);

    // Draw filter bar (immutable borrow).
    {
        let filter = &app.filter;
        let mut parts: Vec<Span> = vec![Span::styled(" Filters: ", theme::header_style())];
        let mut has_filter = false;

        if let Some(ref search) = filter.search {
            parts.push(Span::styled(
                format!("search=\"{search}\" "),
                theme::warning_style(),
            ));
            has_filter = true;
        }
        if let Some(kind) = filter.kind {
            parts.push(Span::styled(
                format!("type={kind} "),
                theme::warning_style(),
            ));
            has_filter = true;
        }
        if let Some(tag_id) = filter.tag_id {
            let tag_name = app.tag_name(tag_id);
            parts.push(Span::styled(
                format!("tag={tag_name} "),
                theme::warning_style(),
            ));
            has_filter = true;
        }
        if let Some(d) = filter.date_from {
            parts.push(Span::styled(
                format!("from={d} "),
                theme::warning_style(),
            ));
            has_filter = true;
        }
        if let Some(d) = filter.date_to {
            parts.push(Span::styled(
                format!("to={d} "),
                theme::warning_style(),
            ));
            has_filter = true;
        }
        if !has_filter {
            parts.push(Span::styled("(none)", theme::muted_style()));
        }

        let line = Line::from(parts);
        frame.render_widget(Paragraph::new(line), filter_area);
    }

    // Draw the main table (needs mutable for TableState).
    {
        use ratatui::style::Modifier;
        use ratatui::widgets::{Cell, Row, Table, TableState};
        use crate::domain::models::{format_cents, TransactionKind};

        let currency = app.config.currency.clone();
        let tsep = app.config.thousands_separator.clone();
        let dsep = app.config.decimal_separator.clone();
        let block = theme::styled_block(" Transactions ");

        let sort_indicator = match app.sort_direction {
            SortDirection::Ascending => " \u{25b2}",
            SortDirection::Descending => " \u{25bc}",
        };
        let header_cols: Vec<String> = vec![
            ("Date", SortColumn::Date),
            ("Source", SortColumn::Source),
            ("Amount", SortColumn::Amount),
            ("Type", SortColumn::Kind),
            ("Tag", SortColumn::Tag),
        ]
        .into_iter()
        .map(|(name, col)| {
            if col == app.sort_column {
                format!("{name}{sort_indicator}")
            } else {
                name.to_string()
            }
        })
        .collect();

        let header = Row::new(header_cols)
            .style(theme::header_style().add_modifier(Modifier::UNDERLINED))
            .bottom_margin(1);

        let rows: Vec<Row> = app
            .transactions
            .iter()
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
                        format_cents(tx.amount, &currency, &tsep, &dsep),
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
            .column_spacing(1)
            .row_highlight_style(theme::selected_style())
            .highlight_symbol("> ");

        let mut state = TableState::default();
        if !app.transactions.is_empty() {
            state.select(Some(app.tx_selected));
        }

        frame.render_stateful_widget(table, table_area, &mut state);
    }

    // Draw footer.
    {
        let help = Line::from(vec![
            Span::styled(" [a]", theme::header_style()),
            Span::styled("dd ", theme::text_style()),
            Span::styled("[e]", theme::header_style()),
            Span::styled("dit ", theme::text_style()),
            Span::styled("[d]", theme::header_style()),
            Span::styled("el ", theme::text_style()),
            Span::styled("[/]", theme::header_style()),
            Span::styled("filter ", theme::text_style()),
            Span::styled("[c]", theme::header_style()),
            Span::styled("lear ", theme::text_style()),
            Span::styled("[s]", theme::header_style()),
            Span::styled("ort ", theme::text_style()),
            Span::styled("[S]", theme::header_style()),
            Span::styled("dir ", theme::text_style()),
            Span::styled("[?]", theme::header_style()),
            Span::styled("help", theme::text_style()),
        ]);
        frame.render_widget(Paragraph::new(help), footer_area);
    }
}

fn draw_recurring_view(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let [table_area, footer_area] = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(area);

    // Draw table with stateful widget.
    {
        use ratatui::style::Modifier;
        use ratatui::widgets::{Cell, Row, Table, TableState};
        use crate::domain::models::{format_cents, TransactionKind};

        let currency = app.config.currency.clone();
        let tsep = app.config.thousands_separator.clone();
        let dsep = app.config.decimal_separator.clone();
        let block = theme::styled_block(" Recurring Entries ");

        if app.recurring_entries.is_empty() {
            let para = Paragraph::new(Span::styled(
                "  No recurring entries. Add a transaction with recurring enabled.",
                theme::muted_style(),
            ))
            .block(block);
            frame.render_widget(para, table_area);
        } else {
            let header =
                Row::new(vec!["Status", "Source", "Amount", "Type", "Interval", "Tag"])
                    .style(theme::header_style().add_modifier(Modifier::UNDERLINED))
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
                            format_cents(entry.amount, &currency, &tsep, &dsep),
                            amount_style,
                        )),
                        Cell::from(Span::styled(kind_str, amount_style)),
                        Cell::from(entry.interval.to_string()),
                        Cell::from(tag_name),
                    ])
                })
                .collect();

            let widths = [
                Constraint::Length(6),
                Constraint::Fill(1),
                Constraint::Length(16),
                Constraint::Length(5),
                Constraint::Length(10),
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

            frame.render_stateful_widget(table, table_area, &mut state);
        }
    }

    // Footer
    {
        let help = Line::from(vec![
            Span::styled(" [Space]", theme::header_style()),
            Span::styled("toggle ", theme::text_style()),
            Span::styled("[d]", theme::header_style()),
            Span::styled("elete ", theme::text_style()),
            Span::styled("[Up/Down]", theme::header_style()),
            Span::styled("select ", theme::text_style()),
            Span::styled("[Esc]", theme::header_style()),
            Span::styled("back ", theme::text_style()),
        ]);
        frame.render_widget(Paragraph::new(help), footer_area);
    }
}

fn draw_tabs(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let titles = vec!["Dashboard", "Transactions", "Stats", "Budgets", "Recurring", "Tags"];
    let selected = match app.current_view {
        View::Dashboard => 0,
        View::Transactions => 1,
        View::Stats => 2,
        View::Budgets => 3,
        View::Recurring => 4,
        View::Tags => 5,
    };

    let tabs = Tabs::new(titles)
        .block(
            ratatui::widgets::Block::bordered()
                .title(" CoinTUI ")
                .title_style(theme::header_style())
                .border_style(theme::border_style()),
        )
        .select(selected)
        .style(theme::text_style())
        .highlight_style(
            theme::header_style()
                .add_modifier(ratatui::style::Modifier::UNDERLINED),
        )
        .divider(" | ");

    frame.render_widget(tabs, area);
}

fn draw_status(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let msg = match &app.status_message {
        Some((msg, _)) => Span::styled(format!(" {msg}"), theme::warning_style()),
        None => Span::styled(
            format!(
                " CoinTUI v{} | {} | DB: {}",
                env!("CARGO_PKG_VERSION"),
                app.config.currency,
                app.db_path_display,
            ),
            theme::muted_style(),
        ),
    };
    frame.render_widget(Paragraph::new(Line::from(msg)), area);
}

fn draw_confirm(frame: &mut Frame, message: &str) {
    use ratatui::layout::Rect;
    use ratatui::widgets::{Block, Clear};

    let area = frame.area();
    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height = 5u16;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::bordered()
        .title(" Confirm ")
        .title_style(theme::warning_style())
        .border_style(ratatui::style::Style::default().fg(theme::YELLOW))
        .style(ratatui::style::Style::default().bg(theme::BG));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let lines = vec![
        Line::from(Span::styled(message, theme::text_style())),
        Line::from(""),
        Line::from(vec![
            Span::styled("[y]", theme::income_style()),
            Span::styled("es  ", theme::text_style()),
            Span::styled("[n]", theme::expense_style()),
            Span::styled("o", theme::text_style()),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

/// Wrapper to draw the dashboard into a specific area (called from the main draw).
fn draw_dashboard(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let [header_area, main_area, alerts_area, footer_area] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(10),
        Constraint::Length(4),
        Constraint::Length(1),
    ])
    .areas(area);

    views::dashboard::draw_dashboard_header(frame, app, header_area);
    views::dashboard::draw_dashboard_recent(frame, app, main_area);
    views::dashboard::draw_dashboard_alerts(frame, app, alerts_area);
    views::dashboard::draw_dashboard_footer(frame, footer_area);
}

fn draw_stats(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    views::stats::draw_stats(frame, app, area);
}

fn draw_budgets(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let [list_area, footer_area] = Layout::vertical([
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(area);

    views::budget::draw_budget_list(frame, app, list_area);
    views::budget::draw_budget_footer(frame, footer_area);
}
