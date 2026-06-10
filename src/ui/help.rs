// ヘルプモーダル（`?` キーで表示）

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{BorderType, Clear, Paragraph};

use super::theme;
use crate::app::AppState;

pub fn draw(f: &mut Frame, area: Rect, state: &AppState) {
    let s = crate::i18n::strings(state.config.ui.language);
    let modal = centered_rect(70, 24, area);
    f.render_widget(Clear, modal);

    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::ACCENT))
        .title(Span::styled(
            s.help_title.to_string(),
            Style::default()
                .fg(theme::ACCENT_2)
                .add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(theme::BG));
    let inner = block.inner(modal);
    f.render_widget(block, modal);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(section(s.help_keys_section));
    lines.push(kv(s.help_q_esc, s.help_q_esc_desc));
    lines.push(kv("?", s.help_qmark_desc));
    lines.push(kv("r", s.help_r_desc));
    lines.push(kv("+ / -", s.help_zoom_desc));
    lines.push(kv("h j k l", s.help_move_desc));
    lines.push(kv(", / .", s.help_scrub_desc));
    lines.push(kv("p", s.help_play_desc));
    lines.push(kv("m", s.help_map_desc));
    lines.push(Line::from(""));
    lines.push(section(s.help_legend_section));
    lines.push(legend_line());
    lines.push(Line::from(""));
    lines.push(section(s.help_sources_section));
    lines.push(kv(s.help_source_rain_jp, s.help_source_rain_jp_value));
    lines.push(kv(
        s.help_source_rain_global,
        s.help_source_rain_global_value,
    ));
    lines.push(kv(s.help_source_map, s.help_source_map_value));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        s.help_close_hint.to_string(),
        Style::default().fg(theme::SUBTLE),
    )));

    f.render_widget(Paragraph::new(lines), inner);
}

fn section(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!(" ▶ {}", title),
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD),
    ))
}

fn kv(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw("    "),
        Span::styled(
            format!("{:<14}", key),
            Style::default()
                .fg(theme::WARN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc.to_string(), Style::default().fg(theme::FG)),
    ])
}

fn legend_line() -> Line<'static> {
    let stops = [
        ("1", (200u8, 230, 255)),
        ("4", (40, 130, 230)),
        ("8", (50, 180, 80)),
        ("16", (250, 230, 50)),
        ("24", (250, 180, 30)),
        ("40", (240, 70, 50)),
        ("64", (200, 50, 180)),
        ("80+", (120, 30, 130)),
    ];
    let mut spans = vec![Span::raw("    ")];
    for (label, (r, g, b)) in stops.iter() {
        spans.push(Span::styled(
            "  ",
            Style::default().bg(ratatui::style::Color::Rgb(*r, *g, *b)),
        ));
        spans.push(Span::styled(
            format!(" {} ", label),
            Style::default().fg(theme::FG),
        ));
    }
    Line::from(spans)
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(height) / 2),
            Constraint::Length(height.min(area.height)),
            Constraint::Min(0),
        ])
        .split(area);
    let h = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(area.width.saturating_sub(width) / 2),
            Constraint::Length(width.min(area.width)),
            Constraint::Min(0),
        ])
        .split(v[1]);
    h[1]
}
