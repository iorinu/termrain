// Open-Meteo プロバイダー実装。
// API キー不要、世界対応、JSON で返ってくるシンプルな仕様。
// https://open-meteo.com/en/docs

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::{
    CurrentWeather, DailyPoint, HourlyPoint, RadarGrid, WeatherIcon, WeatherProvider,
};
use super::jma::{
    blend, draw_cross, draw_legend_bar, lonlat_to_tile, rain_to_yahoo, sample_bilinear,
    tile_to_lonlat,
};

type MapTileKey = (&'static str, u8, u32, u32);

pub struct OpenMeteo {
    client: reqwest::Client,
    /// CARTO Voyager 等のタイル PNG をキャッシュ。スタイル別キー。
    map_image_cache: Arc<Mutex<HashMap<MapTileKey, Arc<image::RgbaImage>>>>,
    /// 地図スタイル（外国対応のため CARTO のみ実用）
    map_style: Arc<Mutex<crate::config::MapStyle>>,
}

impl OpenMeteo {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("termrain/0.1 (+https://github.com/iorinu/termrain)")
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("reqwest クライアントの構築に失敗");
        Self {
            client,
            map_image_cache: Arc::new(Mutex::new(HashMap::new())),
            map_style: Arc::new(Mutex::new(crate::config::MapStyle::CartoVoyager)),
        }
    }

    pub fn set_map_style(&self, style: crate::config::MapStyle) {
        // 地理院系は日本限定なので外国では CARTO に fallback
        let effective = match style {
            crate::config::MapStyle::GsiStd | crate::config::MapStyle::GsiPhoto => {
                crate::config::MapStyle::CartoVoyager
            }
            s => s,
        };
        *self.map_style.lock().unwrap() = effective;
    }

    async fn fetch_map_image(&self, z: u8, x: u32, y: u32) -> Result<Arc<image::RgbaImage>> {
        let style = *self.map_style.lock().unwrap();
        let key = (style.cache_key(), z, x, y);
        if let Some(g) = self.map_image_cache.lock().unwrap().get(&key).cloned() {
            return Ok(g);
        }
        let url = style.tile_url(z, x, y);
        let resp = self.client.get(&url).send().await?;
        let img = if resp.status().is_success() {
            let bytes = resp.bytes().await?;
            image::load_from_memory(&bytes)
                .context("地図タイルデコード失敗")?
                .to_rgba8()
        } else {
            image::RgbaImage::from_pixel(256, 256, image::Rgba([240, 240, 240, 255]))
        };
        let arc = Arc::new(img);
        self.map_image_cache.lock().unwrap().insert(key, arc.clone());
        Ok(arc)
    }
}

impl Default for OpenMeteo {
    fn default() -> Self {
        Self::new()
    }
}

// ===== レスポンスの型 =====
//
// JSON のフィールドをそのまま映す構造体。
// serde の rename_all を使わず、API 側のキーがスネークケースなのでそのままで OK。

#[derive(Debug, Deserialize)]
struct ForecastResponse {
    /// timezone=auto を指定すると返ってくる現地タイムゾーンのオフセット (秒)。
    /// 例: パリ夏時間なら 7200。これを使って current/hourly の time を UTC に直す。
    #[serde(default)]
    utc_offset_seconds: Option<i32>,
    current: Option<CurrentBlock>,
    hourly: Option<HourlyBlock>,
    daily: Option<DailyBlock>,
}

#[derive(Debug, Deserialize)]
struct CurrentBlock {
    time: String,
    temperature_2m: f64,
    relative_humidity_2m: Option<f64>,
    weather_code: u32,
    wind_speed_10m: Option<f64>,
    wind_direction_10m: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct HourlyBlock {
    time: Vec<String>,
    temperature_2m: Vec<f64>,
    precipitation: Vec<f64>,
    precipitation_probability: Option<Vec<Option<f64>>>,
}

#[derive(Debug, Deserialize)]
struct DailyBlock {
    time: Vec<String>,
    weather_code: Vec<u32>,
    temperature_2m_max: Vec<Option<f64>>,
    temperature_2m_min: Vec<Option<f64>>,
    precipitation_probability_max: Option<Vec<Option<f64>>>,
}

// ===== WMO 天気コード → アイコン / 文字列 =====
// https://open-meteo.com/en/docs (Weather variable documentation)
fn wmo_to_icon(code: u32) -> WeatherIcon {
    match code {
        0 => WeatherIcon::Sunny,
        1..=2 => WeatherIcon::PartlyCloudy,
        3 => WeatherIcon::Cloudy,
        45 | 48 => WeatherIcon::Cloudy, // 霧
        51..=67 | 80..=82 => WeatherIcon::Rain,
        71..=77 | 85 | 86 => WeatherIcon::Snow,
        95..=99 => WeatherIcon::Thunder,
        _ => WeatherIcon::Unknown,
    }
}

fn wmo_to_text(code: u32) -> &'static str {
    match code {
        0 => "快晴",
        1 => "晴れ",
        2 => "晴れ時々曇り",
        3 => "曇り",
        45 | 48 => "霧",
        51 | 53 | 55 => "霧雨",
        61 | 63 | 65 => "雨",
        66 | 67 => "凍雨",
        71 | 73 | 75 => "雪",
        77 => "霧雪",
        80 | 81 | 82 => "にわか雨",
        85 | 86 => "にわか雪",
        95 => "雷雨",
        96 | 99 => "雷雨（雹あり）",
        _ => "不明",
    }
}

/// Open-Meteo の現地時刻文字列 + UTC オフセット → 絶対時刻を保った Local DateTime。
///
/// timezone=auto を付けると Open-Meteo は **その地点の現地時刻** を返してくる
/// （例: パリの "2026-06-08T18:00" = UTC 16:00）。これを単純に Local 解釈すると
/// ユーザーの Local とずれて「観測時刻が古すぎる」ような表示バグになる。
///
/// utc_offset_seconds を使って一旦 FixedOffset に変換 → ユーザー Local に直すことで
/// 絶対時刻を保ったまま表示できる（パリの 18:00 → 日本では 01:00 と表示）。
fn parse_local_with_offset(s: &str, offset_seconds: Option<i32>) -> Result<DateTime<Local>> {
    let ndt = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M")
        .with_context(|| format!("時刻パース失敗: {s}"))?;
    let offset = offset_seconds.unwrap_or(0);
    let fixed = chrono::FixedOffset::east_opt(offset).context("invalid UTC offset")?;
    let dt = fixed
        .from_local_datetime(&ndt)
        .single()
        .context("ローカル時刻の単一解決に失敗")?;
    Ok(dt.with_timezone(&Local))
}

/// 後方互換: offset 不明（多地点クエリのレスポンスなど）の場合に Local 解釈する。
fn parse_local(s: &str) -> Result<DateTime<Local>> {
    parse_local_with_offset(s, None).or_else(|_| {
        let ndt = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M")?;
        Local
            .from_local_datetime(&ndt)
            .single()
            .context("ローカル時刻の単一解決に失敗")
    })
}

#[async_trait]
impl WeatherProvider for OpenMeteo {
    fn name(&self) -> &'static str {
        "Open-Meteo"
    }

    async fn current(&self, lat: f64, lon: f64) -> Result<CurrentWeather> {
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
             &current=temperature_2m,relative_humidity_2m,weather_code,wind_speed_10m,wind_direction_10m\
             &timezone=auto"
        );
        let resp: ForecastResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let offset = resp.utc_offset_seconds;
        let cur = resp.current.context("Open-Meteo: current が無い")?;
        Ok(CurrentWeather {
            observed_at: parse_local_with_offset(&cur.time, offset)?,
            condition: wmo_to_text(cur.weather_code).to_string(),
            icon: wmo_to_icon(cur.weather_code),
            temperature_c: cur.temperature_2m,
            humidity_pct: cur.relative_humidity_2m,
            wind_speed_ms: cur.wind_speed_10m,
            wind_direction_deg: cur.wind_direction_10m,
        })
    }

    async fn hourly(&self, lat: f64, lon: f64) -> Result<Vec<HourlyPoint>> {
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
             &hourly=temperature_2m,precipitation,precipitation_probability\
             &forecast_days=2&timezone=auto"
        );
        let resp: ForecastResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let offset = resp.utc_offset_seconds;
        let h = resp.hourly.context("Open-Meteo: hourly が無い")?;
        let mut out = Vec::with_capacity(h.time.len());
        for i in 0..h.time.len() {
            out.push(HourlyPoint {
                time: parse_local_with_offset(&h.time[i], offset)?,
                temperature_c: h.temperature_2m[i],
                precipitation_mm: h.precipitation[i],
                precipitation_prob_pct: h
                    .precipitation_probability
                    .as_ref()
                    .and_then(|v| v.get(i).copied().flatten()),
            });
        }
        Ok(out)
    }

    async fn daily(&self, lat: f64, lon: f64) -> Result<Vec<DailyPoint>> {
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
             &daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max\
             &forecast_days=7&timezone=auto"
        );
        let resp: ForecastResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let d = resp.daily.context("Open-Meteo: daily が無い")?;
        let mut out = Vec::with_capacity(d.time.len());
        for i in 0..d.time.len() {
            let date = NaiveDate::parse_from_str(&d.time[i], "%Y-%m-%d")?;
            let code = d.weather_code[i];
            out.push(DailyPoint {
                date,
                condition: wmo_to_text(code).into(),
                icon: wmo_to_icon(code),
                temp_max_c: d.temperature_2m_max[i],
                temp_min_c: d.temperature_2m_min[i],
                precipitation_prob_pct: d
                    .precipitation_probability_max
                    .as_ref()
                    .and_then(|v| v.get(i).copied().flatten()),
            });
        }
        Ok(out)
    }

    async fn radar(&self, lat: f64, lon: f64, zoom: u8, _time_offset: i32) -> Result<RadarGrid> {
        // 外国でもカラー地図 + 雨雲を表示するため、JMA と同様に画像合成を行う。
        // 違いは:
        //   - 雨雲: Open-Meteo の precipitation を多地点で取得 (32x16 grid)
        //   - 地図: CARTO Voyager タイル (世界対応)
        let map_z: u8 = zoom.min(13);
        let (_, mcx, mcy) = lonlat_to_tile(lon, lat, map_z);

        // view 範囲 = map_z タイル1枚分の地理サイズ（ユーザー位置を中央に）
        let (lat_n_c, lon_w_c) = tile_to_lonlat(map_z, mcx, mcy);
        let (lat_s_c, lon_e_c) = tile_to_lonlat(map_z, mcx + 1, mcy + 1);
        let half_lon = (lon_e_c - lon_w_c) / 2.0;
        let half_lat = (lat_n_c - lat_s_c) / 2.0;
        let view_lon_w = lon - half_lon;
        let view_lon_e = lon + half_lon;
        let view_lat_s = lat - half_lat;
        let view_lat_n = lat + half_lat;

        // ---- 雨雲を多地点で取得 (32x16 grid を view 範囲内に分布) ----
        let width: usize = 32;
        let height: usize = 16;
        let mut lats = Vec::with_capacity(width * height);
        let mut lons = Vec::with_capacity(width * height);
        for j in 0..height {
            let lat_j = view_lat_n - (view_lat_n - view_lat_s) * (j as f64) / (height as f64 - 1.0);
            for i in 0..width {
                let lon_i = view_lon_w + (view_lon_e - view_lon_w) * (i as f64) / (width as f64 - 1.0);
                lats.push(format!("{lat_j:.4}"));
                lons.push(format!("{lon_i:.4}"));
            }
        }
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}\
             &current=precipitation&timezone=auto",
            lats.join(","),
            lons.join(",")
        );
        #[derive(Deserialize)]
        struct MultiCurrent {
            current: Option<MultiCurrentInner>,
        }
        #[derive(Deserialize)]
        struct MultiCurrentInner {
            time: String,
            precipitation: f64,
        }
        let arr: Vec<MultiCurrent> = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let mut data = vec![vec![0.0f64; width]; height];
        let mut observed = None;
        for (idx, item) in arr.iter().enumerate() {
            if let Some(c) = &item.current {
                let y = idx / width;
                let x = idx % width;
                if y < height {
                    data[y][x] = c.precipitation.max(0.0);
                }
                if observed.is_none() {
                    observed = Some(parse_local(&c.time)?);
                }
            }
        }

        // ---- 地図タイルを 3x3 並列取得 ----
        let mut map_fetches = Vec::with_capacity(9);
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let tx = mcx as i32 + dx;
                let ty = mcy as i32 + dy;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let tx = tx as u32;
                let ty = ty as u32;
                map_fetches.push(async move {
                    let g = self.fetch_map_image(map_z, tx, ty).await.ok();
                    ((dx, dy), g)
                });
            }
        }
        let map_results = futures::future::join_all(map_fetches).await;
        let mut map_imgs: HashMap<(i32, i32), Arc<image::RgbaImage>> = HashMap::new();
        for ((dx, dy), maybe) in map_results {
            if let Some(g) = maybe {
                map_imgs.insert((dx, dy), g);
            }
        }

        // ---- 合成画像生成 ----
        let composite_image = build_composite_image_om(
            map_z,
            mcx,
            mcy,
            view_lon_w,
            view_lon_e,
            view_lat_s,
            view_lat_n,
            lon,
            lat,
            &map_imgs,
            &data,
            ((view_lat_s, view_lon_w), (view_lat_n, view_lon_e)),
        );

        Ok(RadarGrid {
            width,
            height,
            data,
            map_dots: Vec::new(),
            composite_image,
            bounds: ((view_lat_s, view_lon_w), (view_lat_n, view_lon_e)),
            observed_at: observed.unwrap_or_else(Local::now),
        })
    }

    fn set_map_style(&self, style: crate::config::MapStyle) {
        Self::set_map_style(self, style);
    }
}

/// Open-Meteo 用合成画像生成（雨雲は数値グリッドから bilinear 補間）。
/// 地図サンプルは JMA と同じヘルパー (sample_bilinear) を使う。
#[allow(clippy::too_many_arguments)]
fn build_composite_image_om(
    map_z: u8,
    map_cx: u32,
    map_cy: u32,
    view_lon_w: f64,
    view_lon_e: f64,
    view_lat_s: f64,
    view_lat_n: f64,
    user_lon: f64,
    user_lat: f64,
    map_imgs: &HashMap<(i32, i32), Arc<image::RgbaImage>>,
    rain_grid: &[Vec<f64>],
    rain_bounds: ((f64, f64), (f64, f64)),
) -> Option<image::DynamicImage> {
    use image::Rgba;

    let out_w: u32 = 1024;
    let out_h: u32 = 1024;
    let mut canvas = image::RgbaImage::from_pixel(out_w, out_h, Rgba([255, 255, 255, 255]));

    let grid_h = rain_grid.len();
    let grid_w = if grid_h > 0 { rain_grid[0].len() } else { 0 };
    let (rlat_s, rlon_w) = rain_bounds.0;
    let (rlat_n, rlon_e) = rain_bounds.1;
    let rlon_span = (rlon_e - rlon_w).max(1e-9);
    let rlat_span = (rlat_n - rlat_s).max(1e-9);

    // 雨雲 grid (lat-lon) からの bilinear 補間
    let sample_rain = |lon: f64, lat: f64| -> f64 {
        if grid_w == 0 || grid_h == 0 {
            return 0.0;
        }
        let fx = (lon - rlon_w) / rlon_span;
        let fy = (rlat_n - lat) / rlat_span; // 北が上、grid は上から下
        if !(0.0..=1.0).contains(&fx) || !(0.0..=1.0).contains(&fy) {
            return 0.0;
        }
        let x = (fx * (grid_w as f64 - 1.0)).max(0.0);
        let y = (fy * (grid_h as f64 - 1.0)).max(0.0);
        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(grid_w - 1);
        let y1 = (y0 + 1).min(grid_h - 1);
        let dx = x - x0 as f64;
        let dy = y - y0 as f64;
        rain_grid[y0][x0] * (1.0 - dx) * (1.0 - dy)
            + rain_grid[y0][x1] * dx * (1.0 - dy)
            + rain_grid[y1][x0] * (1.0 - dx) * dy
            + rain_grid[y1][x1] * dx * dy
    };

    for j in 0..out_h {
        for i in 0..out_w {
            let v_lon = view_lon_w + (view_lon_e - view_lon_w) * (i as f64 + 0.5) / out_w as f64;
            let v_lat = view_lat_n - (view_lat_n - view_lat_s) * (j as f64 + 0.5) / out_h as f64;

            // 地図サンプル
            let mut base = Rgba([255, 255, 255, 255]);
            let (_, mtx, mty) = lonlat_to_tile(v_lon, v_lat, map_z);
            let mdx = mtx as i32 - map_cx as i32;
            let mdy = mty as i32 - map_cy as i32;
            if (-1..=1).contains(&mdx) && (-1..=1).contains(&mdy) {
                if let Some(map) = map_imgs.get(&(mdx, mdy)) {
                    let (lat_n_t, lon_w_t) = tile_to_lonlat(map_z, mtx, mty);
                    let (lat_s_t, lon_e_t) = tile_to_lonlat(map_z, mtx + 1, mty + 1);
                    let fx = (v_lon - lon_w_t) / (lon_e_t - lon_w_t);
                    let fy = (lat_n_t - v_lat) / (lat_n_t - lat_s_t);
                    base = sample_bilinear(map, fx, fy);
                }
            }
            // 雨雲サンプル (grid bilinear)
            let mmh = sample_rain(v_lon, v_lat);
            if let Some((rc, gc, bc, ac)) = rain_to_yahoo(mmh) {
                let a = ac as f64 / 255.0;
                base.0[0] = blend(base.0[0], rc, a);
                base.0[1] = blend(base.0[1], gc, a);
                base.0[2] = blend(base.0[2], bc, a);
            }
            canvas.put_pixel(i, j, base);
        }
    }

    // ユーザー位置に黄色十字
    let user_px = ((user_lon - view_lon_w) / (view_lon_e - view_lon_w) * out_w as f64) as i32;
    let user_py = ((view_lat_n - user_lat) / (view_lat_n - view_lat_s) * out_h as f64) as i32;
    draw_cross(&mut canvas, user_px, user_py, 12, Rgba([255, 220, 0, 255]));

    draw_legend_bar(&mut canvas);

    Some(image::DynamicImage::ImageRgba8(canvas))
}
