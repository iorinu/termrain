// 時間ごと予報リスト（Yahoo 天気風の縦並び）
//
// 1 行 = 1 時間。時刻・アイコン・気温・降水確率を並べる。
// パネル高さに収まる行数だけ表示する。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::theme;
use super::titled_block;
use crate::app::AppState;

pub fn draw(f: &mut Frame, area: Rect, state: &AppState) {
    let s = crate::i18n::strings(state.config.ui.language);
    let title = match state.config.ui.language {
        crate::i18n::Language::Japanese => "1時間ごと",
        crate::i18n::Language::English => "Hourly",
    };
    let block = titled_block(title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    if state.hourly.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  {} {}", state.spinner(), s.fetching),
            Style::default().fg(theme::SUBTLE),
        )));
        f.render_widget(Paragraph::new(lines), inner);
        return;
    }

    let take = (inner.height as usize).min(state.hourly.len()).min(48);

    for p in state.hourly.iter().take(take) {
        let time = p.time.format("%H:%M").to_string();
        let temp = format!("{:>3.0}°C", p.temperature_c);
        let rain = match (p.precipitation_mm, p.precipitation_prob_pct) {
            (mm, _) if mm >= 0.1 => format!("{:>4.1}mm", mm),
            (_, Some(pop)) if pop > 0.0 => format!("{:>3.0}%  ", pop),
            _ => "  -    ".to_string(),
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", time), Style::default().fg(theme::SUBTLE)),
            Span::styled(
                format!("{}  ", p.icon.symbol()),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("{:>6}", temp), Style::default().fg(theme::TEMP)),
            Span::raw("  "),
            Span::styled(rain, Style::default().fg(theme::RAIN)),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}
