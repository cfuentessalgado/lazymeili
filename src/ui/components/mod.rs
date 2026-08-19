use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Cell, Row, Table, TableState},
};

use super::theme;

pub fn table(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    headers: &[&str],
    rows: Vec<Vec<String>>,
    selected: usize,
    accent: ratatui::style::Color,
) {
    let header = Row::new(headers.iter().map(|header| Cell::from(*header)))
        .style(
            Style::new()
                .fg(accent)
                .bg(theme::SURFACE_ALT)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);
    let rows = rows.into_iter().enumerate().map(|(index, row)| {
        let background = if index % 2 == 0 {
            theme::SURFACE
        } else {
            theme::SURFACE_ALT
        };
        Row::new(
            row.into_iter()
                .map(|value| Cell::from(value.clone()).style(theme::semantic(&value))),
        )
        .style(Style::new().bg(background))
    });
    let widths = headers
        .iter()
        .map(|_| ratatui::layout::Constraint::Ratio(1, headers.len() as u32));
    let widget = Table::new(rows, widths)
        .header(header)
        .style(theme::panel())
        .row_highlight_style(theme::selected(accent))
        .highlight_symbol(" ▸ ")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(accent).bg(theme::SURFACE))
                .title_style(theme::title(accent))
                .title(format!(" {title} "))
                .style(theme::panel()),
        );
    let mut state = TableState::default().with_selected(Some(selected));
    frame.render_stateful_widget(widget, area, &mut state);
}
