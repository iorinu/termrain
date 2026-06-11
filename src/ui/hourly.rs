// 時間別予報グラフ。
// ratatui の Chart は数値配列をいい感じに折れ線グラフにしてくれる。

use super::theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, Paragraph};

use super::titled_block;
use crate::app::AppState;

pub fn draw(f: &mut Frame, area: Rect, state: &AppState) {
    let s = crate::i18n::strings(state.config.ui.language);
    let block = titled_block(s.hourly_title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.hourly.is_empty() {
        let p = Paragraph::new(Line::from(Span::styled(
            s.loading,
            Style::default().fg(theme::SUBTLE),
        )));
        f.render_widget(p, inner);
        return;
    }

    // 現在時刻を含む時間帯から表示（過去分は出さない）
    let now = chrono::Local::now();
    let start = state
        .hourly
        .iter()
        .position(|p| p.time + chrono::Duration::hours(1) > now)
        .unwrap_or(0);
    // 表示数は端末幅から逆算（最大 48 時間）
    let take = (inner.width as usize).saturating_sub(4).min(48).max(8);
    let points: Vec<&crate::api::HourlyPoint> =
        state.hourly.iter().skip(start).take(take).collect();

    // 気温折れ線データ
    let temp_data: Vec<(f64, f64)> = points
        .iter()
        .enumerate()
        .map(|(i, p)| (i as f64, p.temperature_c))
        .collect();

    // 降水棒（縦線 0→値）— Bars 用のスタイルが Chart に無いので、ratatui の Sparkline で別レイヤに描く
    // ここでは「上半分: Chart で気温」「下半分: テキストで降水量バー」に分割
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(inner);

    let datasets = vec![
        Dataset::default()
            .name(s.hourly_temp_label)
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme::TEMP))
            .data(&temp_data),
    ];

    let temp_min = temp_data.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let temp_max = temp_data
        .iter()
        .map(|p| p.1)
        .fold(f64::NEG_INFINITY, f64::max);
    // Y 軸の余白は最小限にして折れ線をダイナミックに見せる
    let pad = ((temp_max - temp_min) * 0.1).max(0.5);

    // X 軸の時刻ラベルを 4 〜 6 個生成。先頭・末尾と等間隔の中間点を選ぶ。
    // ratatui の Axis.labels は bounds 範囲を等間隔に区切って配置するので、
    // points から対応 index の時刻を抽出するだけで OK。
    let n = points.len();
    let label_count = ((inner.width as usize) / 12).clamp(3, 6);
    let mut x_labels: Vec<Span> = Vec::with_capacity(label_count);
    let hour_fmt = match state.config.ui.language {
        crate::i18n::Language::Japanese => "%H時",
        crate::i18n::Language::English => "%H:00",
    };
    for k in 0..label_count {
        let idx = if label_count == 1 {
            0
        } else {
            (k * (n - 1)) / (label_count - 1)
        };
        let text = points[idx].time.format(hour_fmt).to_string();
        x_labels.push(Span::styled(text, Style::default().fg(theme::SUBTLE)));
    }

    // Chart は自身の style で領域を塗りつぶすため、bg を明示しないと
    // ブロックで塗った背景がリセットされ、半透明ターミナルでは壁紙が透ける
    let chart = Chart::new(datasets)
        .style(Style::default().bg(theme::BG))
        .x_axis(
            Axis::default()
                .style(Style::default().fg(theme::SUBTLE))
                .bounds([0.0, temp_data.len() as f64 - 1.0])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(theme::SUBTLE))
                .bounds([temp_min - pad, temp_max + pad])
                .labels(vec![
                    Span::styled(
                        format!("{:.0}°", temp_min - pad),
                        Style::default().fg(theme::temp_color(temp_min)),
                    ),
                    Span::styled(
                        format!("{:.0}°", temp_max + pad),
                        Style::default().fg(theme::temp_color(temp_max)),
                    ),
                ]),
        );

    f.render_widget(chart, split[0]);

    // 降水バー
    // - precipitation_mm が一つでも > 0 ならそちらを表示
    // - 全部 0 でも precipitation_prob_pct が取れるなら降水確率(%)で代用 (JMA経路)
    // - どちらも無ければ空白
    let bar_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let has_mm = points.iter().any(|p| p.precipitation_mm > 0.0);
    let has_pop = points
        .iter()
        .any(|p| p.precipitation_prob_pct.is_some_and(|v| v > 0.0));

    // 降水量は時間ごとに強度が違うので、1文字ずつ色を変えた Span にする
    let (bar_spans, label_text) = if has_mm {
        let max_p = points
            .iter()
            .map(|p| p.precipitation_mm)
            .fold(0.0_f64, f64::max)
            .max(1.0);
        let spans: Vec<Span> = points
            .iter()
            .map(|p| {
                if p.precipitation_mm <= 0.0 {
                    Span::raw(" ")
                } else {
                    let ratio = (p.precipitation_mm / max_p).clamp(0.0, 1.0);
                    let idx = ((ratio * (bar_chars.len() as f64 - 1.0)).round() as usize)
                        .min(bar_chars.len() - 1);
                    let color = crate::render::color::precipitation_color(p.precipitation_mm)
                        .unwrap_or(theme::RAIN);
                    Span::styled(bar_chars[idx].to_string(), Style::default().fg(color))
                }
            })
            .collect();
        let label = s
            .precip_amount_label
            .replace("{:.1}", &format!("{:.1}", max_p));
        (spans, label)
    } else if has_pop {
        // 降水確率 (0-100%)
        let spans: Vec<Span> = points
            .iter()
            .map(|p| {
                let v = p.precipitation_prob_pct.unwrap_or(0.0);
                if v <= 0.0 {
                    Span::raw(" ")
                } else {
                    let ratio = (v / 100.0).clamp(0.0, 1.0);
                    let idx = ((ratio * (bar_chars.len() as f64 - 1.0)).round() as usize)
                        .min(bar_chars.len() - 1);
                    Span::styled(bar_chars[idx].to_string(), Style::default().fg(theme::RAIN))
                }
            })
            .collect();
        (spans, s.precip_prob_label.to_string())
    } else {
        (
            vec![Span::raw(" ".repeat(points.len()))],
            s.no_precip_data.to_string(),
        )
    };

    let bar_line = Line::from(bar_spans);
    let label = Line::from(Span::styled(label_text, Style::default().fg(theme::SUBTLE)));
    let p = Paragraph::new(vec![bar_line, label]);
    f.render_widget(p, split[1]);
}
