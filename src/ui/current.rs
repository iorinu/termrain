use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use super::theme;
use super::titled_block;
use crate::app::AppState;

/// 表示幅 (East Asian Wide 考慮) で切り詰める
fn truncate_w(s: &str, width: usize) -> String {
    let mut acc = String::new();
    let mut used = 0usize;
    for c in s.chars() {
        let cw = UnicodeWidthStr::width(c.to_string().as_str());
        if used + cw > width {
            break;
        }
        acc.push(c);
        used += cw;
    }
    acc
}

pub fn draw(f: &mut Frame, area: Rect, state: &AppState) {
    let s = crate::i18n::strings(state.config.ui.language);
    let block = titled_block(s.current_title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    if let Some(cw) = &state.current {
        // パネルの内側幅から、アイコン分（4桁ぶん）を引いた残りまでで切り詰め
        let cond_max = (inner.width as usize).saturating_sub(6);
        let cond = truncate_w(&cw.condition, cond_max);
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {}  ", cw.icon.symbol()),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                cond,
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {}  ", s.temp),
                Style::default().fg(theme::SUBTLE),
            ),
            Span::styled(
                format!("{:>5.1} ℃", cw.temperature_c),
                Style::default()
                    .fg(theme::TEMP)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        if let Some(h) = cw.humidity_pct {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {}  ", s.humidity),
                    Style::default().fg(theme::SUBTLE),
                ),
                Span::styled(format!("{:>5.0} %", h), Style::default().fg(theme::RAIN)),
            ]));
        }
        if let Some(w) = cw.wind_speed_ms {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {}  ", s.wind),
                    Style::default().fg(theme::SUBTLE),
                ),
                Span::styled(format!("{:>5.1} m/s", w), Style::default().fg(theme::FG)),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            cw.observed_at
                .format(&format!("  {} %m/%d %H:%M", s.observed))
                .to_string(),
            Style::default().fg(theme::SUBTLE),
        )));
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  {} {}", state.spinner(), s.fetching),
            Style::default().fg(theme::SUBTLE),
        )));
    }
    f.render_widget(Paragraph::new(lines), inner);
}
