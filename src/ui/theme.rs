// カラーテーマ
//
// Catppuccin Mocha インスパイアの統一パレット。
// 各パネルから色を一元参照することで、後で別テーマに切り替えやすい。

use ratatui::style::Color;

/// パネル背景。レーダー画像を除き、半透明背景でも視認できる程度の暗色。
pub const BG: Color = Color::Rgb(30, 30, 46);
/// 通常テキスト
pub const FG: Color = Color::Rgb(205, 214, 244);
/// アクセント（タイトル等）。青系。
pub const ACCENT: Color = Color::Rgb(137, 180, 250);
/// 第二アクセント。紫系。ヘッダーやステータス用。
pub const ACCENT_2: Color = Color::Rgb(203, 166, 247);
/// 成功 / 気温 高め
#[allow(dead_code)]
pub const SUCCESS: Color = Color::Rgb(166, 227, 161);
/// 警告 / 気温
pub const WARN: Color = Color::Rgb(249, 226, 175);
/// エラー / 強い雨
pub const ERROR: Color = Color::Rgb(243, 139, 168);
/// 補助情報 (sub label, hint)
pub const SUBTLE: Color = Color::Rgb(127, 132, 156);
/// パネル枠線
pub const BORDER: Color = Color::Rgb(88, 91, 112);
/// 気温（赤系）
pub const TEMP: Color = Color::Rgb(243, 139, 168);
/// 降水（青系）
pub const RAIN: Color = Color::Rgb(137, 180, 250);
/// 行ハイライト背景（現在時刻の行など）
pub const HIGHLIGHT_BG: Color = Color::Rgb(49, 50, 68);

/// 気温 → 色のグラデーション（寒=青系 → 暑=赤系）
///
/// Catppuccin の色をストップにして線形補間する。
/// 週間予報の温度バーや気温表示の色付けに使う。
pub fn temp_color(t: f64) -> Color {
    const STOPS: [(f64, (u8, u8, u8)); 6] = [
        (-5.0, (116, 199, 236)), // sapphire（寒い）
        (5.0, (137, 220, 235)),  // sky
        (12.0, (166, 227, 161)), // green
        (20.0, (249, 226, 175)), // yellow
        (27.0, (250, 179, 135)), // peach
        (33.0, (243, 139, 168)), // red（暑い）
    ];
    if t <= STOPS[0].0 {
        let (r, g, b) = STOPS[0].1;
        return Color::Rgb(r, g, b);
    }
    for w in STOPS.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        if t <= t1 {
            let k = (t - t0) / (t1 - t0);
            let lerp = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * k).round() as u8;
            return Color::Rgb(lerp(c0.0, c1.0), lerp(c0.1, c1.1), lerp(c0.2, c1.2));
        }
    }
    let (r, g, b) = STOPS[STOPS.len() - 1].1;
    Color::Rgb(r, g, b)
}
