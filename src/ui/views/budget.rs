use ratatui::layout::{Constraint, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Gauge, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::domain::models::format_centavos;
use crate::ui::theme;

pub fn draw_budget_list(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let block = theme::styled_block(" Budgets ");
    let currency = &app.config.currency;

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

    // Each budget entry gets 3 lines: label, gauge, spacer.
    let budget_count = app.budget_spending.len();
    let mut constraints: Vec<Constraint> = Vec::new();
    for _ in 0..budget_count {
        constraints.push(Constraint::Length(3));
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
        let pct = if limit > 0 {
            (*spent as f64 / limit as f64).min(1.0)
        } else {
            0.0
        };
        let pct_display = if limit > 0 {
            (*spent as f64 / limit as f64) * 100.0
        } else {
            0.0
        };

        let gauge_style = if pct_display >= 100.0 {
            theme::expense_style()
        } else if pct_display >= 60.0 {
            theme::warning_style()
        } else {
            theme::income_style()
        };

        let is_selected = i == app.budget_selected;
        let indicator = if is_selected { "> " } else { "  " };

        let [label_area, gauge_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(2)]).areas(rows[i]);

        let label_style = if is_selected {
            theme::selected_style()
        } else {
            theme::text_style()
        };

        let label = Line::from(vec![
            Span::styled(indicator, label_style),
            Span::styled(
                format!(
                    "{} ({}) - {} / {}  ({:.0}%)",
                    tag_name,
                    budget.period,
                    format_centavos(*spent, currency),
                    format_centavos(limit, currency),
                    pct_display,
                ),
                label_style,
            ),
        ]);
        frame.render_widget(Paragraph::new(label), label_area);

        let gauge = Gauge::default()
            .block(Block::default())
            .gauge_style(gauge_style.add_modifier(Modifier::BOLD))
            .ratio(pct)
            .label(Span::styled(
                format!("{:.0}%", pct_display),
                theme::text_style().add_modifier(Modifier::BOLD),
            ));
        frame.render_widget(gauge, gauge_area);
    }
}

pub fn draw_budget_footer(frame: &mut Frame, area: ratatui::layout::Rect) {
    let help = Line::from(vec![
        Span::styled(" [a]", theme::header_style()),
        Span::styled("dd ", theme::text_style()),
        Span::styled("[d]", theme::header_style()),
        Span::styled("elete ", theme::text_style()),
        Span::styled("[Up/Down]", theme::header_style()),
        Span::styled("select ", theme::text_style()),
        Span::styled("[Esc]", theme::header_style()),
        Span::styled("back ", theme::text_style()),
    ]);
    frame.render_widget(Paragraph::new(help), area);
}
