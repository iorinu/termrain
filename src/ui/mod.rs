// UI レイアウト
//
// 構成:
//   ヘッダー (1)
//   上段 (Min 18): 現在天気 / レーダー / 週間予報
//   時間別予報 (8)
//   フッター (1)
//
// テーマは ui::theme に統一。パネル枠は角丸 (BorderType::Rounded)。

mod current;
mod help;
mod hourly;
mod radar;
mod splash;
pub mod theme;
mod weekly;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::app::AppState;

pub fn draw(f: &mut Frame, state: &mut AppState) {
    let size = f.area();

    // Splash 起動演出（最優先）
    if state.splash_active {
        splash::draw(f, size, state);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(18),
            Constraint::Length(8),
            Constraint::Length(1),
        ])
        .split(size);

    draw_header(f, chunks[0], state);

    // レーダー幅は画像アスペクト比に合わせる
    let radar_aspect = if let Some(img) = state
        .radar
        .as_ref()
        .and_then(|r| r.composite_image.as_ref())
    {
        img.width() as f32 / img.height() as f32
    } else {
        1.0
    };
    let radar_h = chunks[1].height;
    let mut radar_w_cells = ((radar_h as f32) * 2.0 * radar_aspect) as u16;
    const LEFT_W: u16 = 28;
    const SIDE_MIN_W: u16 = 18;
    let total_w = chunks[1].width;
    let reserved = LEFT_W + SIDE_MIN_W;
    if radar_w_cells + reserved > total_w {
        radar_w_cells = total_w.saturating_sub(reserved);
    }
    let side_w = total_w.saturating_sub(LEFT_W + radar_w_cells);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(LEFT_W),
            Constraint::Length(radar_w_cells),
            Constraint::Length(side_w),
        ])
        .split(chunks[1]);
    current::draw(f, top[0], state);
    radar::draw(f, top[1], state);
    weekly::draw(f, top[2], state);

    hourly::draw(f, chunks[2], state);
    draw_footer(f, chunks[3], state);

    // ヘルプモーダル（最後に描画して最前面に）
    if state.show_help {
        help::draw(f, size);
    }
}

fn draw_header(f: &mut Frame, area: Rect, state: &AppState) {
    // ロゴ + 場所 + プロバイダー + 時刻
    let now = chrono::Local::now().format("%H:%M").to_string();
    let line = Line::from(vec![
        Span::styled(
            "  ⛅ termrain ",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(theme::BORDER)),
        Span::styled(
            format!("📍 {}", state.config.location.name),
            Style::default().fg(theme::FG),
        ),
        Span::styled(
            format!(" ({:.3}, {:.3})", state.config.location.latitude, state.config.location.longitude),
            Style::default().fg(theme::SUBTLE),
        ),
        Span::styled("  │  ", Style::default().fg(theme::BORDER)),
        Span::styled(
            &state.provider_name,
            Style::default().fg(theme::ACCENT_2),
        ),
        Span::styled("  │  ", Style::default().fg(theme::BORDER)),
        Span::styled(
            format!("🕐 {}", now),
            Style::default().fg(theme::SUBTLE),
        ),
    ]);
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme::BG)),
        area,
    );
}

fn draw_footer(f: &mut Frame, area: Rect, state: &AppState) {
    // ステータスバー風: 反転背景に主要キーをラベル化
    let play_mark = if state.radar_playing { "▶ " } else { "" };
    let map_label = match state.config.radar.map_style {
        crate::config::MapStyle::GsiStd => "標準",
        crate::config::MapStyle::CartoVoyager => "Carto",
        crate::config::MapStyle::GsiPhoto => "航空",
    };

    let key_style = Style::default()
        .fg(theme::BG)
        .bg(theme::ACCENT)
        .add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(theme::FG).bg(theme::BORDER);
    let sep_style = Style::default().bg(theme::BG);

    let mut spans = vec![
        Span::styled(" ? ", key_style),
        Span::styled(" ヘルプ ", label_style),
        Span::styled(" ", sep_style),
        Span::styled(" q ", key_style),
        Span::styled(" 終了 ", label_style),
        Span::styled(" ", sep_style),
        Span::styled(" r ", key_style),
        Span::styled(" 更新 ", label_style),
        Span::styled(" ", sep_style),
        Span::styled(" hjkl ", key_style),
        Span::styled(" 移動 ", label_style),
        Span::styled(" ", sep_style),
        Span::styled(" , . ", key_style),
        Span::styled(" 時刻 ", label_style),
        Span::styled(" ", sep_style),
        Span::styled(" p ", key_style),
        Span::styled(format!(" {}再生 ", play_mark), label_style),
        Span::styled(" ", sep_style),
        Span::styled(" m ", key_style),
        Span::styled(format!(" {} ", map_label), label_style),
    ];
    if let Some(err) = &state.last_error {
        // フッターは1行なので、長いエラー（URLや stack 含む）は要約して表示。
        // 詳細はログファイルに記録済み。
        let short = summarize_error(err, 64);
        spans.push(Span::styled(
            "  ⚠ ",
            Style::default().fg(theme::ERROR).bg(theme::BG),
        ));
        spans.push(Span::styled(
            short,
            Style::default().fg(theme::ERROR).bg(theme::BG),
        ));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme::BG)),
        area,
    );
}

/// エラーメッセージを1行表示用に短く整形する。
/// - URL や `for url (https://...)` を除去
/// - 改行を空白に
/// - 指定幅で末尾省略
fn summarize_error(err: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    let mut s: String = err.replace('\n', " ");
    // "for url (...)" 以降を削除（reqwest の長い URL を消す）
    if let Some(idx) = s.find(" for url ") {
        s.truncate(idx);
    }
    // "(https://..." も削除
    if let Some(idx) = s.find("(https://") {
        s.truncate(idx);
    }
    // 連続空白を 1 つに
    let s: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    // 幅で切り詰め
    if UnicodeWidthStr::width(s.as_str()) <= max_width {
        return s;
    }
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = UnicodeWidthStr::width(c.to_string().as_str());
        if w + cw + 1 > max_width {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out
}

/// 共通パネル枠（角丸・テーマ色）
pub fn titled_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::BORDER))
        .title(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(theme::BG))
}
