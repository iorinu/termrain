// 週間予報パネル（縦並び版）
//
// レーダー右側のサイドバーに配置する想定。1日あたり 2 行を使い、
// 7 日分で 14 行ちょっと埋める。横並び版より読みやすく、
// レーダー右の余白を有効活用する。

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
    let block = titled_block(s.weekly_title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    if state.daily.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  {} {}", state.spinner(), s.fetching),
            Style::default().fg(theme::SUBTLE),
        )));
        f.render_widget(Paragraph::new(lines), inner);
        return;
    }

    // 縦に並べる: 1日あたり 2 行
    //   行1: 日付 + アイコン
    //   行2: 最高/最低 + 降水確率
    // パネルの高さに収まるだけ表示する。
    let max_days = ((inner.height as usize) / 2).min(7);

    for (i, d) in state.daily.iter().take(max_days).enumerate() {
        // 日付ヘッダ：曜日付き
        let date_label = d.date.format("%m/%d (%a)").to_string();
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", d.icon.symbol()),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                date_label,
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        // 気温と降水確率
        let hi = d
            .temp_max_c
            .map(|v| format!("{:>3.0}", v))
            .unwrap_or_else(|| "  -".into());
        let lo = d
            .temp_min_c
            .map(|v| format!("{:>3.0}", v))
            .unwrap_or_else(|| "  -".into());
        let pop = d
            .precipitation_prob_pct
            .map(|p| format!("{:>3.0}%", p))
            .unwrap_or_else(|| "  - ".into());

        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(format!("{}/{}", hi, lo), Style::default().fg(theme::TEMP)),
            Span::raw("  "),
            Span::styled(pop, Style::default().fg(theme::RAIN)),
        ]));

        // 日と日の間に小さな区切り
        if i + 1 < max_days {
            lines.push(Line::from(""));
        }
    }

    f.render_widget(Paragraph::new(lines), inner);
}
