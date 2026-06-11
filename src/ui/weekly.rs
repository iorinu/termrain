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
    //   行1: アイコン + 日付 + 降水確率
    //   行2: 最低気温 ─━━━─ 最高気温 （週全体の温度レンジに対するバー）
    // パネルの高さに収まるだけ表示する。
    let max_days = ((inner.height as usize) / 2).min(7);

    // 週全体の最低・最高（バーのスケール共通化のため）
    let gmin = state
        .daily
        .iter()
        .filter_map(|d| d.temp_min_c)
        .fold(f64::INFINITY, f64::min);
    let gmax = state
        .daily
        .iter()
        .filter_map(|d| d.temp_max_c)
        .fold(f64::NEG_INFINITY, f64::max);
    // " 18° " + バー + " 24°" + " 40%" を引いた残りがバー幅
    let bar_w = (inner.width as usize).saturating_sub(16).clamp(6, 16);

    for (i, d) in state.daily.iter().take(max_days).enumerate() {
        // 日付ヘッダ：曜日付き
        let date_label = d.date.format("%m/%d (%a)").to_string();
        let pop = d
            .precipitation_prob_pct
            .map(|p| format!("{:>3.0}%", p))
            .unwrap_or_else(|| "  - ".into());
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
            Span::raw("  "),
            Span::styled(pop, Style::default().fg(theme::RAIN)),
        ]));

        // 温度レンジバー
        let mut spans: Vec<Span> = Vec::new();
        match (d.temp_min_c, d.temp_max_c) {
            (Some(lo), Some(hi)) if gmax > gmin => {
                spans.push(Span::styled(
                    format!(" {:>3.0}°", lo),
                    Style::default().fg(theme::temp_color(lo)),
                ));
                spans.push(Span::raw(" "));
                for c in 0..bar_w {
                    // セル中央の温度を求め、当日のレンジ内なら色付きの太線で描く
                    let cell_t = gmin + (c as f64 + 0.5) / bar_w as f64 * (gmax - gmin);
                    if cell_t >= lo && cell_t <= hi {
                        spans.push(Span::styled(
                            "━",
                            Style::default().fg(theme::temp_color(cell_t)),
                        ));
                    } else {
                        spans.push(Span::styled("─", Style::default().fg(theme::BORDER)));
                    }
                }
                spans.push(Span::styled(
                    format!(" {:>3.0}°", hi),
                    Style::default().fg(theme::temp_color(hi)),
                ));
            }
            _ => {
                let hi = d
                    .temp_max_c
                    .map(|v| format!("{:>3.0}", v))
                    .unwrap_or_else(|| "  -".into());
                let lo = d
                    .temp_min_c
                    .map(|v| format!("{:>3.0}", v))
                    .unwrap_or_else(|| "  -".into());
                spans.push(Span::raw("    "));
                spans.push(Span::styled(
                    format!("{}/{}", hi, lo),
                    Style::default().fg(theme::TEMP),
                ));
            }
        }
        lines.push(Line::from(spans));

        // 日と日の間に小さな区切り
        if i + 1 < max_days {
            lines.push(Line::from(""));
        }
    }

    f.render_widget(Paragraph::new(lines), inner);
}
