use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
};

pub fn table(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    headers: &[&str],
    rows: Vec<Vec<String>>,
    selected: usize,
) {
    let header = Row::new(headers.iter().map(|header| Cell::from(*header)))
        .style(Style::default().add_modifier(Modifier::BOLD));
    let rows = rows
        .into_iter()
        .map(|row| Row::new(row.into_iter().map(Cell::from)));
    let widths = headers
        .iter()
        .map(|_| ratatui::layout::Constraint::Ratio(1, headers.len() as u32));
    let widget = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .highlight_symbol("› ")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} ")),
        );
    let mut state = TableState::default().with_selected(Some(selected));
    frame.render_stateful_widget(widget, area, &mut state);
}
