// 降水強度 (mm/h) → ratatui::Color のマッピング
//
// 気象庁ナウキャストの凡例に近い色階調にしている。
//   0:      透明（描画しない）
//   1未満:  灰
//   1-5:    水色
//   5-10:   青
//   10-20:  黄
//   20-30:  橙
//   30-50:  赤
//   50-80:  紫
//   80+:    濃紫

use ratatui::style::Color;

pub fn precipitation_color(mmh: f64) -> Option<Color> {
    if mmh < 0.1 {
        None
    } else if mmh < 1.0 {
        Some(Color::Rgb(170, 220, 240))
    } else if mmh < 5.0 {
        Some(Color::Rgb(100, 200, 240))
    } else if mmh < 10.0 {
        Some(Color::Rgb(50, 100, 220))
    } else if mmh < 20.0 {
        Some(Color::Rgb(250, 240, 80))
    } else if mmh < 30.0 {
        Some(Color::Rgb(250, 170, 50))
    } else if mmh < 50.0 {
        Some(Color::Rgb(240, 70, 70))
    } else if mmh < 80.0 {
        Some(Color::Rgb(200, 60, 200))
    } else {
        Some(Color::Rgb(120, 30, 130))
    }
}

#[cfg(test)]
mod tests {
    // 親モジュール (color.rs) のアイテムを全部使えるようにする
    use super::*;

    // しきい値「未満」で色が変わる仕様なので、各境界の「直前」と「ちょうど」を
    // 別の帯に振り分けられるかを確認する。
    // 例: `mmh < 1.0` なら 0.999... は灰、1.0 は水色。

    #[test]
    fn zero_and_trace_returns_none() {
        // 0.1 未満は「描画しない」= None
        assert_eq!(precipitation_color(0.0), None);
        assert_eq!(precipitation_color(0.05), None);
        // 0.1 の直前 (浮動小数のため 0.0999... を使う)
        assert_eq!(precipitation_color(0.099), None);
    }

    #[test]
    fn negative_input_returns_none() {
        // 念のため負値もガード。`< 0.1` なので自然と None になる想定。
        assert_eq!(precipitation_color(-1.0), None);
    }

    #[test]
    fn boundary_0_1_switches_to_gray() {
        // 0.1 ちょうど → 灰 (170,220,240)
        assert_eq!(
            precipitation_color(0.1),
            Some(Color::Rgb(170, 220, 240))
        );
    }

    #[test]
    fn boundary_1_switches_to_light_blue() {
        // 1.0 未満 = 灰、1.0 ちょうど = 水色
        assert_eq!(
            precipitation_color(0.99),
            Some(Color::Rgb(170, 220, 240))
        );
        assert_eq!(
            precipitation_color(1.0),
            Some(Color::Rgb(100, 200, 240))
        );
    }

    #[test]
    fn boundary_5_switches_to_blue() {
        assert_eq!(
            precipitation_color(4.99),
            Some(Color::Rgb(100, 200, 240))
        );
        assert_eq!(
            precipitation_color(5.0),
            Some(Color::Rgb(50, 100, 220))
        );
    }

    #[test]
    fn boundary_10_switches_to_yellow() {
        assert_eq!(
            precipitation_color(9.99),
            Some(Color::Rgb(50, 100, 220))
        );
        assert_eq!(
            precipitation_color(10.0),
            Some(Color::Rgb(250, 240, 80))
        );
    }

    #[test]
    fn boundary_20_switches_to_orange() {
        assert_eq!(
            precipitation_color(19.99),
            Some(Color::Rgb(250, 240, 80))
        );
        assert_eq!(
            precipitation_color(20.0),
            Some(Color::Rgb(250, 170, 50))
        );
    }

    #[test]
    fn boundary_30_switches_to_red() {
        assert_eq!(
            precipitation_color(29.99),
            Some(Color::Rgb(250, 170, 50))
        );
        assert_eq!(
            precipitation_color(30.0),
            Some(Color::Rgb(240, 70, 70))
        );
    }

    #[test]
    fn boundary_50_switches_to_purple() {
        assert_eq!(
            precipitation_color(49.99),
            Some(Color::Rgb(240, 70, 70))
        );
        assert_eq!(
            precipitation_color(50.0),
            Some(Color::Rgb(200, 60, 200))
        );
    }

    #[test]
    fn boundary_80_switches_to_dark_purple() {
        assert_eq!(
            precipitation_color(79.99),
            Some(Color::Rgb(200, 60, 200))
        );
        assert_eq!(
            precipitation_color(80.0),
            Some(Color::Rgb(120, 30, 130))
        );
    }

    #[test]
    fn extreme_value_stays_in_top_band() {
        // 上限なし。100mm/h でも 1000mm/h でも最上位の濃紫になる
        assert_eq!(
            precipitation_color(1000.0),
            Some(Color::Rgb(120, 30, 130))
        );
    }
}
