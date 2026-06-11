// 時間ごと予報リスト（Yahoo 天気風の縦並び）
//
// 1 行 = 1 時間。時刻・アイコン・気温・降水確率を並べる。
// 現在時刻の行から表示を始め、パネル幅が広いときは複数カラムで埋める。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::theme;
use super::titled_block;
use crate::app::AppState;

/// 1 カラムの幅（行の内容 + 余白）
const COL_W: u16 = 26;

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

    use chrono::Datelike;

    // 現在時刻を含む時間帯からスタート（過去の行は出さない）
    let now = chrono::Local::now();
    let start = state
        .hourly
        .iter()
        .position(|p| p.time + chrono::Duration::hours(1) > now)
        .unwrap_or(0);

    // パネル幅に収まるカラム数（最低 1）。行数はカラム数 × 高さまで。
    let ncols = ((inner.width / COL_W).max(1)) as usize;
    let capacity = ncols * inner.height as usize;

    let mut prev_day: Option<u32> = None;
    for (i, p) in state.hourly.iter().enumerate().skip(start) {
        if lines.len() >= capacity {
            break;
        }

        // 日付が変わったら区切り線を入れる（先頭行は今日なので省略）
        let day = p.time.day();
        if let Some(prev) = prev_day {
            if prev != day && lines.len() + 1 < capacity {
                let label = p.time.format(" %m/%d (%a) ").to_string();
                let pad = (COL_W as usize)
                    .saturating_sub(unicode_width::UnicodeWidthStr::width(label.as_str()) + 2)
                    / 2;
                lines.push(Line::from(Span::styled(
                    format!(
                        "{}{}{}",
                        "─".repeat(pad.max(2)),
                        label,
                        "─".repeat(pad.max(2))
                    ),
                    Style::default().fg(theme::BORDER),
                )));
            }
        }
        prev_day = Some(day);
        if lines.len() >= capacity {
            break;
        }

        let is_now = i == start;
        let row_bg = if is_now {
            Style::default().bg(theme::HIGHLIGHT_BG)
        } else {
            Style::default()
        };

        let time = p.time.format("%H:%M").to_string();
        // 降水: 強度に応じた色のミニバー + 数値
        let bar_chars = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let (rain_bar, rain_text, rain_color) = match (p.precipitation_mm, p.precipitation_prob_pct)
        {
            (mm, _) if mm >= 0.1 => {
                // 16mm/h を最大としてバー高さに変換（それ以上は飽和）
                let ratio = (mm / 16.0).clamp(0.0, 1.0);
                let idx = 1
                    + ((ratio * (bar_chars.len() as f64 - 2.0)).round() as usize)
                        .min(bar_chars.len() - 2);
                let color = crate::render::color::precipitation_color(mm).unwrap_or(theme::RAIN);
                (bar_chars[idx], format!("{:>4.1}mm", mm), color)
            }
            (_, Some(pop)) if pop > 0.0 => {
                let ratio = (pop / 100.0).clamp(0.0, 1.0);
                let idx = 1
                    + ((ratio * (bar_chars.len() as f64 - 2.0)).round() as usize)
                        .min(bar_chars.len() - 2);
                (bar_chars[idx], format!("{:>4.0}% ", pop), theme::RAIN)
            }
            _ => (' ', "   -  ".to_string(), theme::SUBTLE),
        };

        let mut spans = vec![
            Span::styled(if is_now { "▌" } else { " " }, row_bg.fg(theme::ACCENT)),
            Span::styled(
                format!("{} ", time),
                if is_now {
                    row_bg.fg(theme::FG).add_modifier(Modifier::BOLD)
                } else {
                    row_bg.fg(theme::SUBTLE)
                },
            ),
            Span::styled(
                format!("{}  ", p.icon.symbol()),
                row_bg.add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>3.0}°", p.temperature_c),
                row_bg.fg(theme::temp_color(p.temperature_c)),
            ),
            Span::styled("  ", row_bg),
            Span::styled(rain_bar.to_string(), row_bg.fg(rain_color)),
            Span::styled(rain_text, row_bg.fg(rain_color)),
        ];
        if is_now {
            // ハイライト行は背景色をカラム右端まで伸ばす（余分はクリップされる）
            spans.push(Span::styled(" ".repeat(COL_W as usize), row_bg));
        }
        lines.push(Line::from(spans));
    }

    // カラムごとに分割して描画（縦に埋めてから次のカラムへ）
    let rows = inner.height as usize;
    for (col, chunk) in lines.chunks(rows).enumerate().take(ncols) {
        let x = inner.x + col as u16 * COL_W;
        let w = COL_W.min(inner.right().saturating_sub(x));
        if w == 0 {
            break;
        }
        let rect = Rect {
            x,
            y: inner.y,
            width: w,
            height: inner.height,
        };
        f.render_widget(Paragraph::new(chunk.to_vec()), rect);
    }
}
