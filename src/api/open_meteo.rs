// Open-Meteo プロバイダー実装。
// API キー不要、世界対応、JSON で返ってくるシンプルな仕様。
// https://open-meteo.com/en/docs

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::jma::{
    blend, draw_cross, draw_legend_bar, lonlat_to_tile, sample_bilinear, tile_to_lonlat,
};
use super::{CurrentWeather, DailyPoint, HourlyPoint, RadarGrid, WeatherIcon, WeatherProvider};

const API_BASE: &str = "https://api.open-meteo.com/v1/forecast";

type MapTileKey = (&'static str, u8, u32, u32);

pub struct OpenMeteo {
    client: reqwest::Client,
    /// CARTO Voyager 等のタイル PNG をキャッシュ。スタイル別キー。
    map_image_cache: Arc<Mutex<HashMap<MapTileKey, Arc<image::RgbaImage>>>>,
    /// 地図スタイル（外国対応のため CARTO のみ実用）
    map_style: Arc<Mutex<crate::config::MapStyle>>,
    /// 天気テキスト等の表示言語
    language: Arc<Mutex<crate::i18n::Language>>,
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
            language: Arc::new(Mutex::new(crate::i18n::Language::default())),
        }
    }

    pub fn set_language(&self, lang: crate::i18n::Language) {
        *self.language.lock().unwrap() = lang;
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
        self.map_image_cache
            .lock()
            .unwrap()
            .insert(key, arc.clone());
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
    weather_code: Option<Vec<u32>>,
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

fn wmo_to_text(code: u32, lang: crate::i18n::Language) -> &'static str {
    match (lang, code) {
        (crate::i18n::Language::Japanese, c) => match c {
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
        },
        (crate::i18n::Language::English, c) => match c {
            0 => "Clear",
            1 => "Mostly clear",
            2 => "Partly cloudy",
            3 => "Cloudy",
            45 | 48 => "Fog",
            51 | 53 | 55 => "Drizzle",
            61 | 63 | 65 => "Rain",
            66 | 67 => "Freezing rain",
            71 | 73 | 75 => "Snow",
            77 => "Snow grains",
            80 | 81 | 82 => "Showers",
            85 | 86 => "Snow showers",
            95 => "Thunderstorm",
            96 | 99 => "Thunderstorm w/ hail",
            _ => "Unknown",
        },
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

#[async_trait]
impl WeatherProvider for OpenMeteo {
    fn name(&self) -> &'static str {
        // 天気予報は Open-Meteo、雨雲レーダーは RainViewer のタイル画像を使う
        "Open-Meteo + RainViewer"
    }

    fn radar_offset_range(&self) -> (i32, i32) {
        // RainViewer 無料枠は past のみ（最大 12 フレーム = 過去 2 時間）。
        // 未来予測の nowcast は有料プランでないと安定して取れないため、
        // 再生範囲を「過去〜現在」だけにする。
        (-12, 0)
    }

    async fn current(&self, lat: f64, lon: f64) -> Result<CurrentWeather> {
        let url = format!(
            "{API_BASE}?latitude={lat}&longitude={lon}\
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
            condition: wmo_to_text(cur.weather_code, *self.language.lock().unwrap()).to_string(),
            icon: wmo_to_icon(cur.weather_code),
            temperature_c: cur.temperature_2m,
            humidity_pct: cur.relative_humidity_2m,
            wind_speed_ms: cur.wind_speed_10m,
            wind_direction_deg: cur.wind_direction_10m,
        })
    }

    async fn hourly(&self, lat: f64, lon: f64) -> Result<Vec<HourlyPoint>> {
        let url = format!(
            "{API_BASE}?latitude={lat}&longitude={lon}\
             &hourly=temperature_2m,precipitation,precipitation_probability,weather_code\
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
            let icon = h
                .weather_code
                .as_ref()
                .and_then(|v| v.get(i).copied())
                .map(wmo_to_icon)
                .unwrap_or(WeatherIcon::Unknown);
            out.push(HourlyPoint {
                time: parse_local_with_offset(&h.time[i], offset)?,
                temperature_c: h.temperature_2m[i],
                precipitation_mm: h.precipitation[i],
                precipitation_prob_pct: h
                    .precipitation_probability
                    .as_ref()
                    .and_then(|v| v.get(i).copied().flatten()),
                icon,
            });
        }
        Ok(out)
    }

    async fn daily(&self, lat: f64, lon: f64) -> Result<Vec<DailyPoint>> {
        let url = format!(
            "{API_BASE}?latitude={lat}&longitude={lon}\
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
                condition: wmo_to_text(code, *self.language.lock().unwrap()).into(),
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

    async fn radar(
        &self,
        lat: f64,
        lon: f64,
        zoom: u8,
        time_offset: i32,
        aspect: f64,
    ) -> Result<RadarGrid> {
        let aspect = aspect.clamp(1.0, 2.4);
        // 雨雲: RainViewer のタイル画像 (世界対応・無料・レート制限ゆるい)
        // 地図: CARTO Voyager タイル (世界対応)
        // Open-Meteo の多地点 precipitation は無料枠のレート制限がきつくて
        // 512地点クエリだとすぐ 429 になるので RainViewer に切り替えた。
        let map_z: u8 = zoom.min(13);
        // RainViewer の Free Weather Maps API は z<=7 までしか配信していない
        // （z=8 以上は世界中どこを取っても "Zoom Level Not Supported" placeholder が返る）。
        // 有料の Maps API なら z=12 まで対応するが、無料枠でやる以上ここは固定。
        // 1タイル ~80km 相当だが、雨雲の広域分布を見る用途には十分。
        let radar_z: u8 = zoom.min(7);
        let (_, mcx, mcy) = lonlat_to_tile(lon, lat, map_z);
        let (_, rcx, rcy) = lonlat_to_tile(lon, lat, radar_z);

        // view 範囲 = map_z タイル1枚分の地理サイズ（ユーザー位置を中央に）
        let (lat_n_c, lon_w_c) = tile_to_lonlat(map_z, mcx, mcy);
        let (lat_s_c, lon_e_c) = tile_to_lonlat(map_z, mcx + 1, mcy + 1);
        // 横方向はアスペクト比のぶんだけ view を広げる（縦はタイル1枚分のまま）
        let half_lon = (lon_e_c - lon_w_c) / 2.0 * aspect;
        let half_lat = (lat_n_c - lat_s_c) / 2.0;
        let view_lon_w = lon - half_lon;
        let view_lon_e = lon + half_lon;
        let view_lat_s = lat - half_lat;
        let view_lat_n = lat + half_lat;

        // ---- RainViewer の利用可能フレーム一覧と地図タイルを並列取得 ----
        // RainViewer のインデックス JSON は ~数KB の軽いリクエスト。
        // past: 過去2時間分（10分刻み、最大12フレーム）
        // nowcast: 未来30分分（10分刻み、3フレーム）
        let index_fut = self
            .client
            .get("https://api.rainviewer.com/public/weather-maps.json")
            .send();

        let mut map_fetches = Vec::with_capacity(15);
        for dy in -1i32..=1 {
            for dx in -2i32..=2 {
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

        let (index_resp, map_results) =
            tokio::join!(index_fut, futures::future::join_all(map_fetches));
        let index: RvIndex = index_resp?.error_for_status()?.json().await?;

        // time_offset でフレーム選択
        //   0  → past の最新（=現在の雨雲）
        //   -N → past の N 個前
        //   +N → nowcast の N-1 番目
        //
        // RainViewer 無料枠は past のみで未来予測がない（nowcast = 0）ことが多いので、
        // アプリの再生ループ範囲(-6〜+12)に対してフレーム数が足りない。
        // 範囲外は端にクランプする（=再生は最新まで進んで、loop 巻き戻しで一度だけ過去に戻る）。
        // modulo 循環は途中で時間が逆走するので避ける。
        let mut frames = index.radar.past.clone();
        frames.extend(index.radar.nowcast.iter().cloned());
        let total = frames.len() as i32;
        if total == 0 {
            anyhow::bail!("RainViewer: no radar frames available");
        }
        let latest_past_idx = index.radar.past.len().saturating_sub(1) as i32;
        let pick_idx = (latest_past_idx + time_offset).clamp(0, total - 1) as usize;
        let frame = &frames[pick_idx];

        // ---- RainViewer の雨雲タイルを並列取得（5x3 範囲、radar_z） ----
        // color=2 (Universal Blue) は JMA に近い青系の配色
        // options="1_1" = smoothed + with snow
        let tile_url_base = format!("{}{}", index.host, frame.path);
        let color_scheme: u8 = 2;
        let mut radar_fetches = Vec::with_capacity(15);
        for dy in -1i32..=1 {
            for dx in -2i32..=2 {
                let tx = rcx as i32 + dx;
                let ty = rcy as i32 + dy;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let tx = tx as u32;
                let ty = ty as u32;
                let url = format!(
                    "{tile_url_base}/256/{radar_z}/{tx}/{ty}/{color_scheme}/1_1.png"
                );
                let client = self.client.clone();
                radar_fetches.push(async move {
                    let g = fetch_radar_tile(&client, &url).await.ok();
                    ((dx, dy), g)
                });
            }
        }
        let radar_results = futures::future::join_all(radar_fetches).await;

        let mut map_imgs: HashMap<(i32, i32), Arc<image::RgbaImage>> = HashMap::new();
        for ((dx, dy), maybe) in map_results {
            if let Some(g) = maybe {
                map_imgs.insert((dx, dy), g);
            }
        }
        let mut radar_imgs: HashMap<(i32, i32), Arc<image::RgbaImage>> = HashMap::new();
        for ((dx, dy), maybe) in radar_results {
            if let Some(g) = maybe {
                radar_imgs.insert((dx, dy), g);
            }
        }

        let observed_at = chrono::DateTime::from_timestamp(frame.time, 0)
            .map(|dt| dt.with_timezone(&Local))
            .unwrap_or_else(Local::now);

        // RainViewer は数値ではなく事前色付け画像なので、降水量グリッドは不要。
        // RadarGrid の互換性のため空グリッドで埋める。
        let width: usize = 32;
        let height: usize = 16;
        let data = vec![vec![0.0f64; width]; height];

        let composite_image = build_composite_image_rv(
            aspect,
            map_z,
            mcx,
            mcy,
            radar_z,
            rcx,
            rcy,
            view_lon_w,
            view_lon_e,
            view_lat_s,
            view_lat_n,
            lon,
            lat,
            &map_imgs,
            &radar_imgs,
        );

        Ok(RadarGrid {
            width,
            height,
            data,
            map_dots: Vec::new(),
            composite_image,
            bounds: ((view_lat_s, view_lon_w), (view_lat_n, view_lon_e)),
            observed_at,
        })
    }

    fn set_map_style(&self, style: crate::config::MapStyle) {
        Self::set_map_style(self, style);
    }
    fn set_language(&self, lang: crate::i18n::Language) {
        Self::set_language(self, lang);
    }
}

// ===== RainViewer のレスポンス型 =====
//
// インデックス JSON 例:
// {
//   "version": "2.0",
//   "generated": 1700000000,
//   "host": "https://tilecache.rainviewer.com",
//   "radar": {
//     "past": [{"time": 1699999000, "path": "/v2/radar/1699999000"}, ...],
//     "nowcast": [{"time": 1700000600, "path": "/v2/radar/nowcast_xxx"}, ...]
//   }
// }
#[derive(Deserialize, Clone)]
struct RvFrame {
    time: i64,
    path: String,
}

#[derive(Deserialize)]
struct RvRadar {
    #[serde(default)]
    past: Vec<RvFrame>,
    #[serde(default)]
    nowcast: Vec<RvFrame>,
}

#[derive(Deserialize)]
struct RvIndex {
    host: String,
    radar: RvRadar,
}

/// RainViewer の雨雲タイル PNG を取得して RgbaImage にする。
/// 失敗時は呼び出し側で None 扱いにして、その位置の雨雲はスキップする。
///
/// RainViewer は地域・ズームによってはレーダーデータがなく、
/// 「Zoom Level Not Supported」と書かれた固定 placeholder PNG を返してくる。
/// この placeholder は 1370 バイトの 4-bit palette PNG で内容が固定。
/// 本物の雨雲タイル（雨雲がほぼ無い透明タイルでも 2 KB 以上ある）と
/// 明確に区別できるので、サイズで弾いて map のみ表示するようにする。
async fn fetch_radar_tile(client: &reqwest::Client, url: &str) -> Result<Arc<image::RgbaImage>> {
    let bytes = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if bytes.len() < 2000 {
        anyhow::bail!("RainViewer placeholder tile (no radar coverage at this zoom)");
    }
    let img = image::load_from_memory(&bytes)
        .context("RainViewer タイルのデコード失敗")?
        .to_rgba8();
    Ok(Arc::new(img))
}

/// CARTO 地図タイルの上に RainViewer 雨雲タイルをアルファ合成する。
/// 各 (dx, dy) は中心タイル (cx, cy) からの相対オフセット（-2..=2 × -1..=1 の 5x3）。
/// 地図と雨雲は別ズーム (map_z >= radar_z) の場合があるので、それぞれ別に lookup する。
#[allow(clippy::too_many_arguments)]
fn build_composite_image_rv(
    aspect: f64,
    map_z: u8,
    map_cx: u32,
    map_cy: u32,
    radar_z: u8,
    radar_cx: u32,
    radar_cy: u32,
    view_lon_w: f64,
    view_lon_e: f64,
    view_lat_s: f64,
    view_lat_n: f64,
    user_lon: f64,
    user_lat: f64,
    map_imgs: &HashMap<(i32, i32), Arc<image::RgbaImage>>,
    radar_imgs: &HashMap<(i32, i32), Arc<image::RgbaImage>>,
) -> Option<image::DynamicImage> {
    use image::Rgba;

    let out_h: u32 = 1024;
    let out_w: u32 = ((out_h as f64 * aspect).round() as u32).clamp(1024, 2560);
    let mut canvas = image::RgbaImage::from_pixel(out_w, out_h, Rgba([255, 255, 255, 255]));

    for j in 0..out_h {
        for i in 0..out_w {
            let v_lon = view_lon_w + (view_lon_e - view_lon_w) * (i as f64 + 0.5) / out_w as f64;
            let v_lat = view_lat_n - (view_lat_n - view_lat_s) * (j as f64 + 0.5) / out_h as f64;

            // 地図サンプル
            let mut base = Rgba([255, 255, 255, 255]);
            let (_, mtx, mty) = lonlat_to_tile(v_lon, v_lat, map_z);
            let mdx = mtx as i32 - map_cx as i32;
            let mdy = mty as i32 - map_cy as i32;
            if (-2..=2).contains(&mdx) && (-1..=1).contains(&mdy) {
                if let Some(map) = map_imgs.get(&(mdx, mdy)) {
                    let (lat_n_t, lon_w_t) = tile_to_lonlat(map_z, mtx, mty);
                    let (lat_s_t, lon_e_t) = tile_to_lonlat(map_z, mtx + 1, mty + 1);
                    let fx = (v_lon - lon_w_t) / (lon_e_t - lon_w_t);
                    let fy = (lat_n_t - v_lat) / (lat_n_t - lat_s_t);
                    base = sample_bilinear(map, fx, fy);
                }
            }
            // 雨雲サンプル（RainViewer は事前色付け済み RGBA タイル）
            let (_, rtx, rty) = lonlat_to_tile(v_lon, v_lat, radar_z);
            let rdx = rtx as i32 - radar_cx as i32;
            let rdy = rty as i32 - radar_cy as i32;
            if (-2..=2).contains(&rdx) && (-1..=1).contains(&rdy) {
                if let Some(radar) = radar_imgs.get(&(rdx, rdy)) {
                    let (lat_n_t, lon_w_t) = tile_to_lonlat(radar_z, rtx, rty);
                    let (lat_s_t, lon_e_t) = tile_to_lonlat(radar_z, rtx + 1, rty + 1);
                    let fx = (v_lon - lon_w_t) / (lon_e_t - lon_w_t);
                    let fy = (lat_n_t - v_lat) / (lat_n_t - lat_s_t);
                    let pix = sample_bilinear(radar, fx, fy);
                    // RainViewer のタイルはフル不透明の領域が多く、そのまま重ねると
                    // 地図を完全に覆ってしまう。0.55 倍してから合成し、雨雲が
                    // 地図の上に半透明で乗っているように見せる。
                    let a = (pix.0[3] as f64 / 255.0) * 0.55;
                    if a > 0.0 {
                        base.0[0] = blend(base.0[0], pix.0[0], a);
                        base.0[1] = blend(base.0[1], pix.0[1], a);
                        base.0[2] = blend(base.0[2], pix.0[2], a);
                    }
                }
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
