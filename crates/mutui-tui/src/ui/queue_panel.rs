use crate::app::App;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Queue ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.status.queue.is_empty() {
        frame.render_widget(
            Paragraph::new("empty")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let max_rows = inner.height.saturating_sub(1) as usize;
    let center = app.queue_selected;
    let start = center.saturating_sub(max_rows / 2);
    let end = (start + max_rows).min(app.status.queue.len());

    let mut rows: Vec<Line> = Vec::new();
    for (idx, track) in app.status.queue[start..end].iter().enumerate() {
        let real_idx = start + idx;
        let current = real_idx == app.status.queue_index;
        let selected = real_idx == app.queue_selected;

        let prefix = match (current, selected) {
            (true, true) => "◆",
            (true, false) => "▶",
            (false, true) => "▸",
            (false, false) => " ",
        };

        let prefix_style = if current {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else if selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let title_style = if current {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else if selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        rows.push(Line::from(vec![
            Span::styled(format!("{prefix}{:02} ", real_idx + 1), prefix_style),
            Span::styled(
                fit_inline(&track.title, inner.width.saturating_sub(5) as usize),
                title_style,
            ),
        ]));
    }

    frame.render_widget(Paragraph::new(rows), inner);
}

fn fit_inline(input: &str, max: usize) -> String {
    if input.chars().count() <= max {
        return input.to_string();
    }
    if max <= 1 {
        return "…".to_string();
    }
    let mut out = String::new();
    for c in input.chars().take(max - 1) {
        out.push(c);
    }
    out.push('…');
    out
}
