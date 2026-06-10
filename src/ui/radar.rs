// 雨雲レーダーパネル
//
// 表示レイヤ（下から順）:
//   1. 海岸線（Natural Earth ベクタライン）— 灰色の細線
//   2. 中心点 "+" 十字（地理的アンカー、黄色）
//   3. 雨雲降水セル（色付き Braille 点）
//
// ratatui::widgets::canvas::Canvas は内部で Braille 文字（⠿）を使って
// 「1セル=2x4ドット」の点描画ができる。地図線も雨雲点も同じキャンバスに
// 重ねて描画することで、雨雲レーダー風の見た目になる。
//
// 座標変換:
//   レーダー grid は (width, height) のセル空間。bounds = ((lat_s, lon_w), (lat_n, lon_e))。
//   海岸線データは (lon, lat) の地理座標。
//   両者を共通の「キャンバス座標 (x, y) = (0..width, 0..height)」に変換する。
//     x = (lon - lon_w) / (lon_e - lon_w) * width
//     y = (lat - lat_s) / (lat_n - lat_s) * height   // 北が上

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Paragraph,
    canvas::{Canvas, Line as CanvasLine, Points},
};

use super::titled_block;
use crate::app::AppState;
use crate::map;
use crate::render::color::precipitation_color;
use ratatui_image::{Resize, StatefulImage};

/// Kitty/Sixel graphics プロトコルで合成画像を描画する版。
/// StatefulImage は描画時に area サイズに合わせてリサイズするため、
/// パネルが大きいウィンドウでも画像が領域いっぱいに広がる。
fn draw_image_radar(f: &mut Frame, area: Rect, state: &mut AppState) {
    let s = crate::i18n::strings(state.config.ui.language);
    let Some(grid) = state.radar.as_ref() else {
        return;
    };
    let max_mmh = grid
        .data
        .iter()
        .flat_map(|r| r.iter().copied())
        .fold(0.0_f64, f64::max);
    // 現在時刻との差分（分）から「+5分」等の相対表示を作る
    let now = chrono::Local::now();
    let diff_min = (grid.observed_at - now).num_minutes();
    let rel = match state.config.ui.language {
        crate::i18n::Language::Japanese => {
            if diff_min == 0 {
                "現在".to_string()
            } else if diff_min > 0 {
                format!("+{}分", diff_min)
            } else {
                format!("{}分", diff_min)
            }
        }
        crate::i18n::Language::English => {
            if diff_min == 0 {
                "now".to_string()
            } else if diff_min > 0 {
                format!("+{}min", diff_min)
            } else {
                format!("{}min", diff_min)
            }
        }
    };
    let play = if state.radar_playing { " ▶" } else { "" };
    let map_attrib = state.config.radar.map_style.label();
    let loading_mark = if state.radar_loading {
        format!("{} ", state.spinner())
    } else {
        String::new()
    };
    let map_word = match state.config.ui.language {
        crate::i18n::Language::Japanese => "地図",
        crate::i18n::Language::English => "Map",
    };
    let title = format!(
        "{}{}  {} ({}){}  max {:.1}mm/h  [{}: {}]",
        loading_mark,
        s.radar_title,
        grid.observed_at.format("%H:%M"),
        rel,
        play,
        max_mmh,
        map_word,
        map_attrib,
    );
    let block = if state.radar_loading {
        // 取得中はタイトル色を WARN（黄）にして「更新中」を強調
        ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(ratatui::style::Style::default().fg(super::theme::WARN))
            .title(ratatui::text::Span::styled(
                format!(" {} ", title),
                ratatui::style::Style::default()
                    .fg(super::theme::WARN)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ))
            .style(ratatui::style::Style::default().bg(super::theme::BG))
    } else {
        titled_block(&title)
    };
    let inner = block.inner(area);
    f.render_widget(block, area);
    if let Some(protocol) = state.radar_protocol.as_mut() {
        // Resize::Fit は「画像が area より小さければそのまま」になるので、
        // 拡大もしてほしい場合は Scale を使う。これで合成画像がパネル全域に広がる。
        let image_widget = StatefulImage::default().resize(Resize::Scale(None));
        f.render_stateful_widget(image_widget, inner, protocol);
    }
}

pub fn draw(f: &mut Frame, area: Rect, state: &mut AppState) {
    // 画像レンダラー (Kitty/Sixel等) が使えて、合成画像が用意できていれば
    // そちらを優先描画する。ratatui-image なら端末ピクセル単位で滑らかな地図が出る。
    if state.radar_protocol.is_some() {
        draw_image_radar(f, area, state);
        return;
    }

    // レーダーは地図表示なので、wezterm の半透明背景が透けると線も雨雲も読めない。
    // ここだけパネル全体を黒で塗ってベタ地図画面にする。
    // titled_block では bg を制御していないので、専用に Block を組み立てる。
    let radar_bg = Style::default().bg(Color::Black);

    let Some(grid) = state.radar.clone() else {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray))
            .title(Span::styled(
                "雨雲レーダー (読み込み中…)",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .style(radar_bg);
        f.render_widget(block, area);
        return;
    };

    let max_mmh = grid
        .data
        .iter()
        .flat_map(|r| r.iter().copied())
        .fold(0.0_f64, f64::max);
    let title = format!(
        "雨雲レーダー  {}  max {:.1}mm/h",
        grid.observed_at.format("%m/%d %H:%M"),
        max_mmh
    );

    // titled_block と同じ見た目だが、bg を黒に上書き
    let block = titled_block(&title).style(radar_bg);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if grid.width == 0 || grid.height == 0 {
        let p = Paragraph::new(Line::from(Span::styled(
            "データなし",
            Style::default().fg(Color::Gray),
        )));
        f.render_widget(p, inner);
        return;
    }

    let w = grid.width as f64;
    let h = grid.height as f64;
    let (lat_s, lon_w) = grid.bounds.0;
    let (lat_n, lon_e) = grid.bounds.1;
    let lon_span = (lon_e - lon_w).max(1e-9);
    let lat_span = (lat_n - lat_s).max(1e-9);

    // (lon, lat) → キャンバス (x, y)
    let project = move |lon: f64, lat: f64| -> (f64, f64) {
        let x = (lon - lon_w) / lon_span * w;
        let y = (lat - lat_s) / lat_span * h; // 北が上：lat大 → y大
        (x, y)
    };

    // 各レイヤ（海岸線・県境・市町村界）を事前にキャンバス座標に投影しておく。
    // 市町村界はズームが浅いとごちゃつくので、表示範囲の経度幅で判定して
    // 「狭い表示範囲のときだけ」描画する。閾値は経験的に 1.5 度。
    let to_canvas = |lon1: f64, lat1: f64, lon2: f64, lat2: f64| {
        let (x1, y1) = project(lon1, lat1);
        let (x2, y2) = project(lon2, lat2);
        (x1, y1, x2, y2)
    };
    let coast_segments: Vec<(f64, f64, f64, f64)> = state
        .map
        .segments_for(crate::map::Layer::Coast, grid.bounds)
        .into_iter()
        .map(|(a, b, c, d)| to_canvas(a, b, c, d))
        .collect();
    let pref_segments: Vec<(f64, f64, f64, f64)> = state
        .map
        .segments_for(crate::map::Layer::Prefecture, grid.bounds)
        .into_iter()
        .map(|(a, b, c, d)| to_canvas(a, b, c, d))
        .collect();
    let show_municipalities = lon_span <= 1.5;
    let muni_segments: Vec<(f64, f64, f64, f64)> = if show_municipalities {
        state
            .map
            .segments_for(crate::map::Layer::Municipality, grid.bounds)
            .into_iter()
            .map(|(a, b, c, d)| to_canvas(a, b, c, d))
            .collect()
    } else {
        Vec::new()
    };

    // 範囲内の主要都市を事前にピックアップして投影
    let city_marks: Vec<(f64, f64, &'static str)> = map::cities()
        .iter()
        .filter(|c| c.lat >= lat_s && c.lat <= lat_n && c.lon >= lon_w && c.lon <= lon_e)
        .map(|c| {
            let (x, y) = project(c.lon, c.lat);
            (x, y, c.name)
        })
        .collect();

    // 現在地（ユーザー指定座標）をキャンバス座標に投影しておく。
    // 注意: タイル中心 ≠ ユーザー位置。"+" マーカーはキャンバス中央ではなく
    // 実際の (lat, lon) を project() で変換した位置に描く必要がある。
    let user_lat = state.config.location.latitude;
    let user_lon = state.config.location.longitude;
    let (user_x, user_y) = project(user_lon, user_lat);

    // 表示範囲の地理サイズ（スケールバー用）
    // 緯度1度 ≒ 111km。表示範囲の経度幅を中央緯度で補正してから km 算出。
    let mid_lat = (lat_s + lat_n) / 2.0;
    let width_km = (lon_e - lon_w) * 111.0 * mid_lat.to_radians().cos();
    let height_km = (lat_n - lat_s) * 111.0;

    let data = grid.data.clone();
    let map_dots = grid.map_dots.clone();

    // 降水量バケツ → 色
    let buckets: [(f64, f64); 8] = [
        (0.1, 1.0),
        (1.0, 5.0),
        (5.0, 10.0),
        (10.0, 20.0),
        (20.0, 30.0),
        (30.0, 50.0),
        (50.0, 80.0),
        (80.0, f64::INFINITY),
    ];

    let canvas = Canvas::default()
        .background_color(Color::Black)
        .marker(symbols::Marker::Braille)
        .x_bounds([0.0, w])
        .y_bounds([0.0, h])
        .paint(move |ctx| {
            // ---- レイヤ0: 背景地図ドット（地理院淡色タイルから抽出）----
            // 道路・行政界・地名文字の輪郭が淡い点群として出る。
            // 雨雲の下に置くため最初に描く。
            if !map_dots.is_empty() {
                let mut pts: Vec<(f64, f64)> = Vec::new();
                for j in 0..map_dots.len() {
                    let row = &map_dots[j];
                    for i in 0..row.len() {
                        if row[i] {
                            let y = (map_dots.len() - 1 - j) as f64 + 0.5;
                            let x = i as f64 + 0.5;
                            pts.push((x, y));
                        }
                    }
                }
                if !pts.is_empty() {
                    ctx.draw(&Points {
                        coords: &pts,
                        color: Color::Rgb(90, 105, 125),
                    });
                }
            }

            // ---- レイヤ1a: 市町村界（最も暗い・最下層）----
            // 市町村界は線が密なので、暗めの色で「あるけど主張しない」程度に。
            // show_municipalities=false のときは muni_segments が空なのでスキップされる。
            for (x1, y1, x2, y2) in &muni_segments {
                ctx.draw(&CanvasLine {
                    x1: *x1,
                    y1: *y1,
                    x2: *x2,
                    y2: *y2,
                    color: Color::Rgb(60, 70, 85),
                });
            }

            // ---- レイヤ1b: 都道府県境（中間の明度）----
            for (x1, y1, x2, y2) in &pref_segments {
                ctx.draw(&CanvasLine {
                    x1: *x1,
                    y1: *y1,
                    x2: *x2,
                    y2: *y2,
                    color: Color::Rgb(100, 110, 130),
                });
            }

            // ---- レイヤ1c: 海岸線（最も明るい・最上層）----
            for (x1, y1, x2, y2) in &coast_segments {
                ctx.draw(&CanvasLine {
                    x1: *x1,
                    y1: *y1,
                    x2: *x2,
                    y2: *y2,
                    color: Color::Rgb(150, 170, 200),
                });
            }

            // ---- レイヤ2: 雨雲（色付き点）----
            for (lo, hi) in buckets.iter() {
                let Some(color) = precipitation_color(*lo) else {
                    continue;
                };
                let mut pts: Vec<(f64, f64)> = Vec::new();
                for j in 0..data.len() {
                    let row = &data[j];
                    for i in 0..row.len() {
                        let v = row[i];
                        if v >= *lo && v < *hi {
                            // grid の j=0 は北端（lat 大）。キャンバスは y 大が上なので反転不要
                            // ……ではなく、grid 内部の y は「上から下」のインデックス順なので、
                            // キャンバス y に変換するときは反転する。
                            let y = (data.len() - 1 - j) as f64 + 0.5;
                            let x = i as f64 + 0.5;
                            pts.push((x, y));
                        }
                    }
                }
                if !pts.is_empty() {
                    ctx.draw(&Points {
                        coords: &pts,
                        color,
                    });
                }
            }

            // ---- レイヤ3: 主要都市マーカー（範囲内のみ）----
            // 都市マーカーはシアン寄りで、雨雲の青系と区別。
            // ● + 都市名 の組み合わせで地理的な参照点を提供する。
            let city_color = Color::Rgb(120, 220, 220);
            for (cx, cy, name) in &city_marks {
                ctx.print(*cx, *cy, Span::styled("●", Style::default().fg(city_color)));
                ctx.print(
                    *cx + 1.0,
                    *cy,
                    Span::styled(format!(" {}", name), Style::default().fg(city_color)),
                );
            }

            // ---- レイヤ4: 現在地 "+" マーカーのみ ----
            // ユーザー指定の緯度経度をキャンバス座標に投影した位置に置く。
            // タイル中央ではない点に注意。
            // 地点名は近接する都市マーカーやヘッダーに既に出ているので、ここでは付けない。
            ctx.print(
                user_x,
                user_y,
                Span::styled(
                    "+",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            );

            if max_mmh < 0.1 {
                // 「降水なし」はキャンバス中央付近に出す（現在地ラベルと被らせない）
                ctx.print(
                    w / 2.0 - 4.0,
                    h / 2.0 + 1.5,
                    Span::styled("降水なし", Style::default().fg(Color::Gray)),
                );
            }

            // ---- レイヤ5: 方位マーカー (N/S/E/W) ----
            // 上下左右の中央に置く。常に「上が北」なので、
            // 移動・ズーム後も方位感覚を保てる。
            let dir_style = Style::default()
                .fg(Color::Rgb(180, 180, 200))
                .add_modifier(Modifier::BOLD);
            ctx.print(w / 2.0, h - 0.5, Span::styled("N", dir_style));
            ctx.print(w / 2.0, 0.5, Span::styled("S", dir_style));
            ctx.print(0.5, h / 2.0, Span::styled("W", dir_style));
            ctx.print(w - 1.5, h / 2.0, Span::styled("E", dir_style));

            // ---- レイヤ6: 降水量凡例（右下、縦並び） ----
            // 8 段の色付きブロックと、対応する mm/h 値。
            // 縦方向に並べ、最下段が弱い雨、上に行くほど強い雨。
            // 雨雲データの色マッピングと正確に一致させる。
            let legend: [(&str, Color); 8] = [
                ("1", Color::Rgb(170, 220, 240)),
                ("5", Color::Rgb(100, 200, 240)),
                ("10", Color::Rgb(50, 100, 220)),
                ("20", Color::Rgb(250, 240, 80)),
                ("30", Color::Rgb(250, 170, 50)),
                ("50", Color::Rgb(240, 70, 70)),
                ("80", Color::Rgb(200, 60, 200)),
                ("80+", Color::Rgb(120, 30, 130)),
            ];
            let legend_x = w - 8.0;
            // 凡例タイトル「mm/h」
            ctx.print(
                legend_x,
                legend.len() as f64 + 0.5,
                Span::styled("mm/h", Style::default().fg(Color::Rgb(150, 160, 180))),
            );
            for (i, (label, color)) in legend.iter().enumerate() {
                let y = (legend.len() - 1 - i) as f64 + 0.5;
                ctx.print(legend_x, y, Span::styled("■", Style::default().fg(*color)));
                ctx.print(
                    legend_x + 1.0,
                    y,
                    Span::styled(
                        format!(" {}", label),
                        Style::default().fg(Color::Rgb(180, 190, 210)),
                    ),
                );
            }

            // ---- レイヤ7: スケール / 座標ラベル（左下） ----
            ctx.print(
                0.5,
                0.7,
                Span::styled(
                    format!("≈ {:.0}km × {:.0}km", width_km, height_km),
                    Style::default().fg(Color::Rgb(120, 130, 150)),
                ),
            );
            ctx.print(
                0.5,
                0.0,
                Span::styled(
                    format!("{:.2}°N {:.2}°E", lat_s, lon_w),
                    Style::default().fg(Color::Rgb(120, 130, 150)),
                ),
            );
            ctx.print(
                w - 14.0,
                h - 0.3,
                Span::styled(
                    format!("{:.2}°N {:.2}°E", lat_n, lon_e),
                    Style::default().fg(Color::Rgb(120, 130, 150)),
                ),
            );

            // 地理院タイル利用規約に基づく出典明記
            if !map_dots.is_empty() {
                ctx.print(
                    0.5,
                    -0.6,
                    Span::styled(
                        "地図: 国土地理院",
                        Style::default().fg(Color::Rgb(90, 100, 120)),
                    ),
                );
            }
        });

    f.render_widget(canvas, inner);
}
