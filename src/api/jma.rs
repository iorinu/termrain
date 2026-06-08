// 気象庁プロバイダー（日本国内向け）
//
// 気象庁の Bosai-JMA は公式に公開されている JSON エンドポイント群。
// area code（地域コード）は area.json から引く必要があるが、
// 簡易実装としては「最も近い予報官署」を緯度経度から線形に選ぶ。
//
// レーダー（ナウキャスト）は PNG タイルが配布されているので、
// 表示エリアに対応するタイルを DL → ピクセル色を読んで降水強度に変換する。

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Local, NaiveDate, TimeZone};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::{
    CurrentWeather, DailyPoint, HourlyPoint, RadarGrid, WeatherIcon, WeatherProvider,
};

/// 1 タイル分の降水量グリッド（128x128, mm/h）。
/// 取得後はメモリキャッシュに保持し、隣接タイルへの移動でも DL し直さない。
type TileGrid = Vec<Vec<f64>>;
type TileKey = (u8, u32, u32, String); // (z, x, y, validtime)

const TILE_W: usize = 128;
const TILE_H: usize = 128;
const VIEW_W: usize = 128;
const VIEW_H: usize = 128;

/// 背景地図タイル用キャッシュキー（時刻不要：地図タイルは静的、style ごとに別キー）
type MapTileKey = (&'static str, u8, u32, u32);
/// 地図ドットグリッド（128x128, true=線/文字あり）
type MapDotGrid = Vec<Vec<bool>>;

pub struct Jma {
    client: reqwest::Client,
    /// (z, x, y, validtime) → 128x128 grid のキャッシュ。
    /// validtime をキーに含めることで、ナウキャストが更新されたら自動的に
    /// 別エントリ扱いになり、古いタイルが使い回されない。
    tile_cache: Arc<Mutex<HashMap<TileKey, Arc<TileGrid>>>>,
    /// 地理院淡色ドット用キャッシュ（現状未使用、Brailleフォールバック用）
    map_tile_cache: Arc<Mutex<HashMap<MapTileKey, Arc<MapDotGrid>>>>,
    /// JMA ナウキャスト PNG をそのまま RGBA 画像でキャッシュ（画像合成用）。
    rain_image_cache: Arc<Mutex<HashMap<TileKey, Arc<image::RgbaImage>>>>,
    /// 地図 PNG/JPG をスタイル別にキャッシュ（画像合成用）。
    /// キーには map_style.cache_key() を含めて、スタイルを切り替えても
    /// 過去にDLしたタイルが再利用される。
    map_image_cache: Arc<Mutex<HashMap<MapTileKey, Arc<image::RgbaImage>>>>,
    /// 現在の地図スタイル。Arc<Mutex> で外部から動的に切り替え可能。
    map_style: Arc<Mutex<crate::config::MapStyle>>,
}

impl Jma {
    pub fn set_map_style(&self, style: crate::config::MapStyle) {
        *self.map_style.lock().unwrap() = style;
    }
}

impl Jma {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("termrain/0.1 (+https://github.com/iorinu/termrain)")
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("reqwest クライアントの構築に失敗");
        Self {
            client,
            tile_cache: Arc::new(Mutex::new(HashMap::new())),
            map_tile_cache: Arc::new(Mutex::new(HashMap::new())),
            rain_image_cache: Arc::new(Mutex::new(HashMap::new())),
            map_image_cache: Arc::new(Mutex::new(HashMap::new())),
            map_style: Arc::new(Mutex::new(crate::config::MapStyle::CartoVoyager)),
        }
    }

    /// JMA ナウキャスト PNG をそのまま RgbaImage で取得（画像合成用）。
    async fn fetch_rain_image(
        &self,
        z: u8,
        x: u32,
        y: u32,
        basetime: &str,
        validtime: &str,
    ) -> Result<Arc<image::RgbaImage>> {
        let key = (z, x, y, validtime.to_string());
        if let Some(g) = self.rain_image_cache.lock().unwrap().get(&key).cloned() {
            return Ok(g);
        }
        let url = format!(
            "https://www.jma.go.jp/bosai/jmatile/data/nowc/{}/none/{}/surf/hrpns/{}/{}/{}.png",
            basetime, validtime, z, x, y
        );
        let resp = self.client.get(&url).send().await?;
        let img = if resp.status().is_success() {
            let bytes = resp.bytes().await?;
            image::load_from_memory(&bytes)
                .context("雨雲 PNG デコード失敗")?
                .to_rgba8()
        } else {
            // 雨雲なし領域は完全透明 256x256 にしておく
            image::RgbaImage::from_pixel(256, 256, image::Rgba([0, 0, 0, 0]))
        };
        let arc = Arc::new(img);
        self.rain_image_cache
            .lock()
            .unwrap()
            .insert(key, arc.clone());
        Ok(arc)
    }

    /// 背景地図タイル（現在の map_style に応じて URL 切り替え）を RgbaImage で取得。
    /// キャッシュは style 別にキーを分けるので、スタイル切り替え後の再取得が高速。
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

    /// 地理院淡色ドット（Brailleフォールバック用）。現状は使われていないが保持。
    async fn fetch_map_tile(&self, z: u8, x: u32, y: u32) -> Result<Arc<MapDotGrid>> {
        let key = ("pale_dots", z, x, y);
        if let Some(g) = self.map_tile_cache.lock().unwrap().get(&key).cloned() {
            return Ok(g);
        }
        let url = format!(
            "https://cyberjapandata.gsi.go.jp/xyz/pale/{}/{}/{}.png",
            z, x, y
        );
        let resp = self.client.get(&url).send().await?;
        let dots: MapDotGrid = if resp.status().is_success() {
            let bytes = resp.bytes().await?;
            let img = image::load_from_memory(&bytes)
                .context("地理院タイル PNG デコード失敗")?
                .to_rgba8();
            binarize_map_tile(&img)
        } else {
            vec![vec![false; TILE_W]; TILE_H]
        };
        let arc = Arc::new(dots);
        self.map_tile_cache.lock().unwrap().insert(key, arc.clone());
        Ok(arc)
    }

    /// PNG タイルを取得して 128x128 グリッドにする。キャッシュ優先。
    /// 404（雨雲の無い領域）は「全 0 タイル」として扱い、これもキャッシュする。
    async fn fetch_tile(
        &self,
        z: u8,
        x: u32,
        y: u32,
        basetime: &str,
        validtime: &str,
    ) -> Result<Arc<TileGrid>> {
        let key = (z, x, y, validtime.to_string());
        if let Some(g) = self.tile_cache.lock().unwrap().get(&key).cloned() {
            return Ok(g);
        }

        let url = format!(
            "https://www.jma.go.jp/bosai/jmatile/data/nowc/{}/none/{}/surf/hrpns/{}/{}/{}.png",
            basetime, validtime, z, x, y
        );
        let resp = self.client.get(&url).send().await?;
        let grid: TileGrid = if resp.status().is_success() {
            let bytes = resp.bytes().await?;
            let img = image::load_from_memory(&bytes)
                .context("ナウキャスト PNG デコード失敗")?
                .to_rgba8();
            downsample_tile(&img)
        } else {
            // 雨雲が一切無いタイルは 404 が返るケースがある → 空タイル扱い
            vec![vec![0.0; TILE_W]; TILE_H]
        };
        let arc = Arc::new(grid);
        self.tile_cache.lock().unwrap().insert(key, arc.clone());
        Ok(arc)
    }

    /// 緯度経度から最も近い「府県予報区（クラス10/オフィスエリア）」のコードを返す。
    /// テーブルはコンパイル時に持っておく（気象庁の area.json から主要地点を抽出）。
    fn nearest_area(lat: f64, lon: f64) -> &'static AreaEntry {
        let table = areas();
        table
            .iter()
            .min_by(|a, b| {
                let da = (a.lat - lat).powi(2) + (a.lon - lon).powi(2);
                let db = (b.lat - lat).powi(2) + (b.lon - lon).powi(2);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("エリアテーブルが空")
    }
}

impl Default for Jma {
    fn default() -> Self {
        Self::new()
    }
}

// 主要府県のコードと府県庁所在地の緯度経度
pub struct AreaEntry {
    pub office: &'static str, // 例 "130000" (東京都)
    pub class10: &'static str, // 例 "130010" (東京地方)
    pub name: &'static str,
    pub lat: f64,
    pub lon: f64,
}

fn areas() -> &'static [AreaEntry] {
    // 47 都道府県すべてではなく、主要都市のサブセット。
    // 不足するエリアは将来追加（area.json を取り込んで自動生成しても良い）。
    static TABLE: &[AreaEntry] = &[
        AreaEntry { office: "016000", class10: "016010", name: "札幌", lat: 43.0642, lon: 141.3469 },
        AreaEntry { office: "020000", class10: "020010", name: "青森", lat: 40.8244, lon: 140.7400 },
        AreaEntry { office: "040000", class10: "040010", name: "仙台", lat: 38.2682, lon: 140.8694 },
        AreaEntry { office: "130000", class10: "130010", name: "東京", lat: 35.6812, lon: 139.7671 },
        AreaEntry { office: "140000", class10: "140010", name: "横浜", lat: 35.4478, lon: 139.6425 },
        AreaEntry { office: "150000", class10: "150010", name: "新潟", lat: 37.9026, lon: 139.0232 },
        AreaEntry { office: "230000", class10: "230010", name: "名古屋", lat: 35.1815, lon: 136.9066 },
        AreaEntry { office: "270000", class10: "270000", name: "大阪", lat: 34.6937, lon: 135.5023 },
        AreaEntry { office: "280000", class10: "280010", name: "神戸", lat: 34.6913, lon: 135.1830 },
        AreaEntry { office: "340000", class10: "340010", name: "広島", lat: 34.3853, lon: 132.4553 },
        AreaEntry { office: "390000", class10: "390010", name: "高知", lat: 33.5597, lon: 133.5311 },
        AreaEntry { office: "400000", class10: "400010", name: "福岡", lat: 33.5904, lon: 130.4017 },
        AreaEntry { office: "471000", class10: "471010", name: "那覇", lat: 26.2124, lon: 127.6809 },
    ];
    TABLE
}

// ===== JMA forecast JSON の型 =====
// forecast/{office_code}.json のレスポンスは配列で、先頭が「短期予報」、
// 末尾要素が「週間予報」になっている。中身は時系列ごとに areas が入る入れ子。
//
// 完全に型付けすると膨大なので、ここでは必要最小限を抽出する serde_json::Value を使う。

#[derive(Debug, Deserialize)]
struct OverviewResponse {
    #[serde(rename = "reportDatetime")]
    report_datetime: String,
    text: String,
    #[serde(rename = "targetArea")]
    target_area: String,
}

fn text_to_icon(s: &str) -> WeatherIcon {
    // ざっくり判定。先頭に来やすい語を優先的に拾う
    if s.contains("雷") {
        WeatherIcon::Thunder
    } else if s.contains("雪") {
        WeatherIcon::Snow
    } else if s.contains("雨") {
        WeatherIcon::Rain
    } else if s.contains("曇") && s.contains("晴") {
        WeatherIcon::PartlyCloudy
    } else if s.contains("曇") {
        WeatherIcon::Cloudy
    } else if s.contains("晴") {
        WeatherIcon::Sunny
    } else {
        WeatherIcon::Unknown
    }
}

#[async_trait]
impl WeatherProvider for Jma {
    fn name(&self) -> &'static str {
        "気象庁 (JMA)"
    }

    async fn current(&self, lat: f64, lon: f64) -> Result<CurrentWeather> {
        // JMA は「今この瞬間の気温・湿度」を返す軽量 API が無いため、
        // overview_forecast の天気概況 + forecast の本日の最高/最低を使って
        // 「現在に近い」概況を返す。気温は最寄りの地点予報の最高気温を仮置き。
        let area = Self::nearest_area(lat, lon);
        let overview_url = format!(
            "https://www.jma.go.jp/bosai/forecast/data/overview_forecast/{}.json",
            area.office
        );
        let overview: OverviewResponse = self
            .client
            .get(&overview_url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // 天気概況の最初の数十文字を「天気」として使う
        let condition = overview
            .text
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(40)
            .collect::<String>();
        let icon = text_to_icon(&condition);

        // ISO8601 with offset (+09:00 など) を Local にする
        let observed_at = DateTime::parse_from_rfc3339(&overview.report_datetime)
            .context("JMA report_datetime パース失敗")?
            .with_timezone(&Local);

        // forecast から本日の最高気温を取り出す（あれば）
        let temp = fetch_today_temp(&self.client, area).await.unwrap_or(f64::NAN);

        Ok(CurrentWeather {
            observed_at,
            condition: if condition.is_empty() {
                overview.target_area
            } else {
                condition
            },
            icon,
            temperature_c: temp,
            humidity_pct: None,
            wind_speed_ms: None,
            wind_direction_deg: None,
        })
    }

    async fn hourly(&self, lat: f64, lon: f64) -> Result<Vec<HourlyPoint>> {
        // JMA forecast.json は 1時間粒度の降水量は配信していない。
        // 代わりに「3時間刻みの降水確率 (pops)」と「3時間刻みの気温 (temps)」が取れる。
        // 降水量バーは降水確率を代用する（UI 側で表示切替）。
        let area = Self::nearest_area(lat, lon);
        let url = format!(
            "https://www.jma.go.jp/bosai/forecast/data/forecast/{}.json",
            area.office
        );
        let json: serde_json::Value = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let short = json.get(0).context("forecast[0] が無い")?;
        let series = short
            .get("timeSeries")
            .and_then(|v| v.as_array())
            .context("timeSeries が無い")?;

        let area_name = area.name;

        // 「最寄り地点を含む area。無ければ先頭」を選ぶヘルパ
        let pick_area = |ts: &serde_json::Value| -> Option<serde_json::Value> {
            let areas = ts.get("areas").and_then(|v| v.as_array())?;
            areas
                .iter()
                .find(|a| {
                    a.get("area")
                        .and_then(|x| x.get("name"))
                        .and_then(|x| x.as_str())
                        .is_some_and(|n| n.contains(area_name))
                })
                .or_else(|| areas.first())
                .cloned()
        };

        // ---- 1) pops を時刻ペアで集める ----
        let mut pops_pairs: Vec<(DateTime<Local>, f64)> = Vec::new();
        for ts in series {
            let Some(a) = pick_area(ts) else { continue };
            let Some(pops) = a.get("pops").and_then(|v| v.as_array()) else {
                continue;
            };
            let Some(times) = ts.get("timeDefines").and_then(|v| v.as_array()) else {
                continue;
            };
            for (i, pv) in pops.iter().enumerate() {
                let Some(time_s) = times.get(i).and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(p_str) = pv.as_str() else { continue };
                let Ok(p) = p_str.parse::<f64>() else { continue };
                let Ok(dt) = DateTime::parse_from_rfc3339(time_s) else {
                    continue;
                };
                pops_pairs.push((dt.with_timezone(&Local), p));
            }
        }

        // 最寄りの pop を探すユーティリティ。
        // pops は 3-6 時間粒度なので、時刻差が大きくても妥協的に最寄りを採用する。
        let nearest_pop = |t: DateTime<Local>| -> Option<f64> {
            pops_pairs
                .iter()
                .min_by_key(|(pt, _)| (*pt - t).num_minutes().abs())
                .map(|(_, p)| *p)
        };

        // ---- 2) temps から HourlyPoint を作り、最寄り pop を付加 ----
        let mut hourly: Vec<HourlyPoint> = Vec::new();
        for ts in series {
            let times = ts
                .get("timeDefines")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let Some(a) = pick_area(ts) else { continue };
            let Some(temps) = a.get("temps").and_then(|v| v.as_array()) else {
                continue;
            };
            for (i, t) in temps.iter().enumerate() {
                let Some(time_s) = times.get(i).and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(t_str) = t.as_str() else { continue };
                let Ok(temp_c) = t_str.parse::<f64>() else { continue };
                let Ok(dt) = DateTime::parse_from_rfc3339(time_s) else {
                    continue;
                };
                let time = dt.with_timezone(&Local);
                hourly.push(HourlyPoint {
                    time,
                    temperature_c: temp_c,
                    precipitation_mm: 0.0,
                    precipitation_prob_pct: nearest_pop(time),
                });
            }
        }

        // temps が空 (週末等) なら pops 単体で系列を構築
        if hourly.is_empty() {
            for (time, pop) in pops_pairs {
                hourly.push(HourlyPoint {
                    time,
                    temperature_c: f64::NAN,
                    precipitation_mm: 0.0,
                    precipitation_prob_pct: Some(pop),
                });
            }
        }

        Ok(hourly)
    }

    async fn daily(&self, lat: f64, lon: f64) -> Result<Vec<DailyPoint>> {
        let area = Self::nearest_area(lat, lon);
        let url = format!(
            "https://www.jma.go.jp/bosai/forecast/data/forecast/{}.json",
            area.office
        );
        let json: serde_json::Value = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // forecast[1] が週間予報
        let weekly = json.get(1).context("forecast[1] (週間予報) が無い")?;
        let series = weekly
            .get("timeSeries")
            .and_then(|v| v.as_array())
            .context("週間 timeSeries が無い")?;

        // 1つ目の timeSeries に天気コード/降水確率、2つ目に最高/最低気温
        let weather_ts = series.first().context("週間 weather TS")?;
        let temp_ts = series.get(1).context("週間 temp TS")?;

        let dates: Vec<NaiveDate> = weather_ts
            .get("timeDefines")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .filter_map(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Local).date_naive())
                    .collect()
            })
            .unwrap_or_default();

        // 最寄り地点を含む area
        let pick = |ts: &serde_json::Value| -> Option<serde_json::Value> {
            let areas = ts.get("areas").and_then(|v| v.as_array())?;
            areas
                .iter()
                .find(|a| {
                    a.get("area")
                        .and_then(|x| x.get("name"))
                        .and_then(|x| x.as_str())
                        .is_some_and(|n| n.contains(area.name))
                })
                .or_else(|| areas.first())
                .cloned()
        };

        let weather_area = pick(weather_ts).context("週間 weather area")?;
        let temp_area = pick(temp_ts).context("週間 temp area")?;

        let weather_codes: Vec<String> = weather_area
            .get("weatherCodes")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let pops: Vec<Option<f64>> = weather_area
            .get("pops")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().map(|v| v.as_str().and_then(|s| s.parse().ok())).collect())
            .unwrap_or_default();
        let tmax: Vec<Option<f64>> = temp_area
            .get("tempsMax")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().map(|v| v.as_str().and_then(|s| s.parse().ok())).collect())
            .unwrap_or_default();
        let tmin: Vec<Option<f64>> = temp_area
            .get("tempsMin")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().map(|v| v.as_str().and_then(|s| s.parse().ok())).collect())
            .unwrap_or_default();

        let mut out = Vec::new();
        for i in 0..dates.len() {
            let code = weather_codes.get(i).cloned().unwrap_or_default();
            let condition = jma_weather_code_text(&code);
            out.push(DailyPoint {
                date: dates[i],
                condition: condition.to_string(),
                icon: text_to_icon(condition),
                temp_max_c: tmax.get(i).copied().flatten(),
                temp_min_c: tmin.get(i).copied().flatten(),
                precipitation_prob_pct: pops.get(i).copied().flatten(),
            });
        }

        Ok(out)
    }

    fn set_map_style(&self, style: crate::config::MapStyle) {
        Self::set_map_style(self, style);
    }

    async fn radar(&self, lat: f64, lon: f64, zoom: u8, time_offset: i32) -> Result<RadarGrid> {
        // ナウキャストは PNG タイル。targetTimes_N1.json には複数時刻が並ぶ。
        // 配列の先頭が最新（現在）、time_offset で前後にスクラブ。
        #[derive(Deserialize)]
        struct TargetTime {
            basetime: String,
            validtime: String,
        }
        let times: Vec<TargetTime> = self
            .client
            .get("https://www.jma.go.jp/bosai/jmatile/data/nowc/targetTimes_N1.json")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if times.is_empty() {
            anyhow::bail!("ナウキャスト targetTimes が空");
        }
        // 範囲外クランプ。time_offset==0 が「現在」。
        let len = times.len() as i32;
        let idx = (time_offset).clamp(0, len - 1) as usize;
        let t = &times[idx];
        let basetime = t.basetime.clone();
        let validtime = t.validtime.clone();

        // 地図と雨雲でズームを分離する。
        // - 地図 (CARTO/GSI): z=13 まで実データがある → 高ズームで取れば綺麗
        // - 雨雲 (JMA hrpns): z=10 が上限 → それ以上は中心領域をクロップして拡大
        // view 範囲は地図ズームのタイル1枚分に固定 → 自然なズーム表示。
        let map_z: u8 = zoom.min(13);
        let rain_z: u8 = zoom.min(10);
        // 地図中心タイル
        let (mz, cx, cy) = lonlat_to_tile(lon, lat, map_z);
        // 雨雲中心タイル
        let (_, rcx, rcy) = lonlat_to_tile(lon, lat, rain_z);
        // 下流コードと変数名を揃えるためのエイリアス
        let z = mz;

        // ---- 3x3 タイル × 4 種類を並列取得 ----
        // - 雨雲 grid（数値、Brailleフォールバック用 + max計算用）
        // - 地図ドット（Brailleフォールバック用、現状は使わない予定）
        // - 雨雲 PNG画像（Kitty graphics 用）
        // - 地図 PNG画像（Kitty graphics 用）
        let mut rain_fetches = Vec::with_capacity(9);
        let mut map_dot_fetches = Vec::with_capacity(9);
        let mut rain_img_fetches = Vec::with_capacity(9);
        let mut map_img_fetches = Vec::with_capacity(9);
        // 地図は map_z で 3x3、雨雲は rain_z で 3x3。
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                // 地図側 (map_z)
                let mtx = cx as i32 + dx;
                let mty = cy as i32 + dy;
                if mtx >= 0 && mty >= 0 {
                    let mtx = mtx as u32;
                    let mty = mty as u32;
                    map_dot_fetches.push(async move {
                        let g = self.fetch_map_tile(rain_z, mtx, mty).await.ok();
                        ((dx, dy), g)
                    });
                    map_img_fetches.push(async move {
                        let g = self.fetch_map_image(map_z, mtx, mty).await.ok();
                        ((dx, dy), g)
                    });
                }
                // 雨雲側 (rain_z, 中心 rcx/rcy)
                let rtx = rcx as i32 + dx;
                let rty = rcy as i32 + dy;
                if rtx >= 0 && rty >= 0 {
                    let rtx = rtx as u32;
                    let rty = rty as u32;
                    let bt = basetime.clone();
                    let vt = validtime.clone();
                    rain_fetches.push(async move {
                        let g = self.fetch_tile(rain_z, rtx, rty, &bt, &vt).await.ok();
                        ((dx, dy), g)
                    });
                    let bt2 = basetime.clone();
                    let vt2 = validtime.clone();
                    rain_img_fetches.push(async move {
                        let g = self.fetch_rain_image(rain_z, rtx, rty, &bt2, &vt2).await.ok();
                        ((dx, dy), g)
                    });
                }
            }
        }
        let (rain_results, map_results, rain_img_results, map_img_results) = tokio::join!(
            futures::future::join_all(rain_fetches),
            futures::future::join_all(map_dot_fetches),
            futures::future::join_all(rain_img_fetches),
            futures::future::join_all(map_img_fetches),
        );
        let mut tile_map: HashMap<(i32, i32), Arc<TileGrid>> = HashMap::new();
        for ((dx, dy), maybe_grid) in rain_results {
            if let Some(g) = maybe_grid {
                tile_map.insert((dx, dy), g);
            }
        }
        let mut map_dot_map: HashMap<(i32, i32), Arc<MapDotGrid>> = HashMap::new();
        for ((dx, dy), maybe_grid) in map_results {
            if let Some(g) = maybe_grid {
                map_dot_map.insert((dx, dy), g);
            }
        }
        let mut rain_img_map: HashMap<(i32, i32), Arc<image::RgbaImage>> = HashMap::new();
        for ((dx, dy), maybe) in rain_img_results {
            if let Some(g) = maybe {
                rain_img_map.insert((dx, dy), g);
            }
        }
        let mut map_img_map: HashMap<(i32, i32), Arc<image::RgbaImage>> = HashMap::new();
        for ((dx, dy), maybe) in map_img_results {
            if let Some(g) = maybe {
                map_img_map.insert((dx, dy), g);
            }
        }

        // ---- 表示範囲：ユーザー位置を中央に、地図ズーム (map_z) のタイル1枚分 ----
        // map_z を上げれば view 範囲が自動的に狭くなり、画像はその範囲を綺麗に表示。
        // 雨雲は z=10 までしか無いので、map_z>10 の時は雨雲のピクセル解像度が見える。
        let (lat_n_c, lon_w_c) = tile_to_lonlat(map_z, cx, cy);
        let (lat_s_c, lon_e_c) = tile_to_lonlat(map_z, cx + 1, cy + 1);
        let half_lon = (lon_e_c - lon_w_c) / 2.0;
        let half_lat = (lat_n_c - lat_s_c) / 2.0;
        let view_lon_w = lon - half_lon;
        let view_lon_e = lon + half_lon;
        let view_lat_s = lat - half_lat;
        let view_lat_n = lat + half_lat;

        // ---- 9 タイル合成 grid をユーザー中心の view 範囲で再サンプル ----
        // 雨雲データ (data) と背景地図ドット (map_dots) を同じ座標系で同時に組み立てる。
        let mut data = vec![vec![0.0f64; VIEW_W]; VIEW_H];
        let mut map_dots = vec![vec![false; VIEW_W]; VIEW_H];
        for j in 0..VIEW_H {
            for i in 0..VIEW_W {
                let v_lon = view_lon_w
                    + (view_lon_e - view_lon_w) * (i as f64 + 0.5) / VIEW_W as f64;
                let v_lat = view_lat_n
                    - (view_lat_n - view_lat_s) * (j as f64 + 0.5) / VIEW_H as f64;
                let (_zz, tx, ty) = lonlat_to_tile(v_lon, v_lat, z);
                let dx = tx as i32 - cx as i32;
                let dy = ty as i32 - cy as i32;
                if !(-1..=1).contains(&dx) || !(-1..=1).contains(&dy) {
                    continue;
                }
                // タイル内のピクセル位置（左上= (lat_n_t, lon_w_t)）
                let (lat_n_t, lon_w_t) = tile_to_lonlat(z, tx, ty);
                let (lat_s_t, lon_e_t) = tile_to_lonlat(z, tx + 1, ty + 1);
                let fx = (v_lon - lon_w_t) / (lon_e_t - lon_w_t);
                let fy = (lat_n_t - v_lat) / (lat_n_t - lat_s_t);
                let ti = ((fx * TILE_W as f64) as usize).min(TILE_W - 1);
                let tj = ((fy * TILE_H as f64) as usize).min(TILE_H - 1);
                if let Some(tile) = tile_map.get(&(dx, dy)) {
                    data[j][i] = tile[tj][ti];
                }
                if let Some(map_tile) = map_dot_map.get(&(dx, dy)) {
                    map_dots[j][i] = map_tile[tj][ti];
                }
            }
        }

        let observed_at = parse_jma_compact(&validtime).unwrap_or_else(|_| Local::now());

        // ---- Kitty graphics 用合成画像を生成 ----
        let composite_image = build_composite_image(
            map_z,
            cx,
            cy,
            rain_z,
            rcx,
            rcy,
            view_lon_w,
            view_lon_e,
            view_lat_s,
            view_lat_n,
            lon,
            lat,
            &map_img_map,
            &rain_img_map,
        );

        Ok(RadarGrid {
            width: VIEW_W,
            height: VIEW_H,
            data,
            map_dots,
            composite_image,
            bounds: ((view_lat_s, view_lon_w), (view_lat_n, view_lon_e)),
            observed_at,
        })
    }
}

/// 9 タイルを地理座標系で 768x768 のキャンバスに貼り合わせ、
/// ユーザー位置を中心にした view 範囲をクロップ。
/// 雨雲は地図の上に alpha blend で重ねる。
fn build_composite_image(
    map_z: u8,
    map_cx: u32,
    map_cy: u32,
    rain_z: u8,
    rain_cx: u32,
    rain_cy: u32,
    view_lon_w: f64,
    view_lon_e: f64,
    view_lat_s: f64,
    view_lat_n: f64,
    user_lon: f64,
    user_lat: f64,
    map_imgs: &HashMap<(i32, i32), Arc<image::RgbaImage>>,
    rain_imgs: &HashMap<(i32, i32), Arc<image::RgbaImage>>,
) -> Option<image::DynamicImage> {
    use image::Rgba;

    // 出力解像度。view 範囲は地理座標で「ほぼ正方形」(タイル1枚分=lat/lon 同程度の度差)
    // なので、ピクセル比も正方形にしないと画像が縦/横に潰れて見える。
    // 表示パネルの幅をレイアウト側で「画像のアスペクト比に合わせる」ことで歪み回避。
    let out_w: u32 = 1024;
    let out_h: u32 = 1024;

    // 9 タイル合成キャンバスは「中心タイル + 周辺8タイル」= 3 タイル幅 × 3 タイル高
    // タイル1枚=256ピクセル → 合成は 768x768
    // ただし出力は view 範囲（地理座標）でクロップするので、合成の中間ステップとして
    // 各出力ピクセルに対応するタイルとピクセル位置を逆引きで求める方式にする。

    let mut canvas = image::RgbaImage::from_pixel(out_w, out_h, Rgba([255, 255, 255, 255]));

    for j in 0..out_h {
        for i in 0..out_w {
            // ピクセル位置 → 地理座標（view 範囲を比例分割）
            let v_lon = view_lon_w
                + (view_lon_e - view_lon_w) * (i as f64 + 0.5) / out_w as f64;
            let v_lat = view_lat_n
                - (view_lat_n - view_lat_s) * (j as f64 + 0.5) / out_h as f64;

            // ---- 地図サンプル (map_z タイル空間) ----
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
            // ---- 雨雲サンプル (rain_z タイル空間) ----
            let (_, rtx, rty) = lonlat_to_tile(v_lon, v_lat, rain_z);
            let rdx = rtx as i32 - rain_cx as i32;
            let rdy = rty as i32 - rain_cy as i32;
            if (-1..=1).contains(&rdx) && (-1..=1).contains(&rdy) {
                if let Some(rain) = rain_imgs.get(&(rdx, rdy)) {
                    let (lat_n_t, lon_w_t) = tile_to_lonlat(rain_z, rtx, rty);
                    let (lat_s_t, lon_e_t) = tile_to_lonlat(rain_z, rtx + 1, rty + 1);
                    let fx = (v_lon - lon_w_t) / (lon_e_t - lon_w_t);
                    let fy = (lat_n_t - v_lat) / (lat_n_t - lat_s_t);
                    let mmh = sample_rain_max(rain, fx, fy);
                    if let Some((rc, gc, bc, ac)) = rain_to_yahoo(mmh) {
                        let a = ac as f64 / 255.0;
                        base.0[0] = blend(base.0[0], rc, a);
                        base.0[1] = blend(base.0[1], gc, a);
                        base.0[2] = blend(base.0[2], bc, a);
                    }
                }
            }
            canvas.put_pixel(i, j, base);
        }
    }

    // ユーザー位置に黄色の十字を描き込む
    let user_px = ((user_lon - view_lon_w) / (view_lon_e - view_lon_w) * out_w as f64) as i32;
    let user_py = ((view_lat_n - user_lat) / (view_lat_n - view_lat_s) * out_h as f64) as i32;
    draw_cross(&mut canvas, user_px, user_py, 12, Rgba([255, 220, 0, 255]));

    // 凡例カラーバーを下端に焼き込む
    draw_legend_bar(&mut canvas);

    Some(image::DynamicImage::ImageRgba8(canvas))
}

/// 0..255 のチャンネルを alpha 合成（fg を a の割合で混ぜる）
pub(crate) fn blend(bg: u8, fg: u8, a: f64) -> u8 {
    let v = (bg as f64) * (1.0 - a) + (fg as f64) * a;
    v.clamp(0.0, 255.0) as u8
}

/// 降水強度 (mm/h) → 14 段階のカラー (R, G, B, A)。
/// バウンダリ: 0,1,2,4,8,12,16,24,32,40,48,56,64,80,80+
/// 水色 → 青 → 緑 → 黄緑 → 黄 → 橙 → 赤 → 紫 のグラデーション。
pub(crate) fn rain_to_yahoo(mmh: f64) -> Option<(u8, u8, u8, u8)> {
    if mmh < 0.1 {
        return None;
    }
    let (r, g, b, a) = if mmh < 1.0 {
        (200, 230, 255, 170) // 0-1: ほぼ無色〜薄水色
    } else if mmh < 2.0 {
        (160, 210, 250, 190) // 1-2: 水色
    } else if mmh < 4.0 {
        (90, 170, 240, 210) // 2-4: 薄青
    } else if mmh < 8.0 {
        (40, 130, 230, 225) // 4-8: 青
    } else if mmh < 12.0 {
        (50, 180, 80, 235) // 8-12: 緑
    } else if mmh < 16.0 {
        (140, 220, 60, 240) // 12-16: 黄緑
    } else if mmh < 24.0 {
        (250, 230, 50, 240) // 16-24: 黄
    } else if mmh < 32.0 {
        (250, 180, 30, 245) // 24-32: 橙
    } else if mmh < 40.0 {
        (250, 130, 30, 245) // 32-40: 濃橙
    } else if mmh < 48.0 {
        (240, 70, 50, 250) // 40-48: 赤
    } else if mmh < 56.0 {
        (220, 40, 70, 250) // 48-56: 濃赤
    } else if mmh < 64.0 {
        (200, 50, 180, 250) // 56-64: 紫
    } else if mmh < 80.0 {
        (170, 30, 180, 250) // 64-80: 濃紫
    } else {
        (120, 30, 130, 250) // 80+: 暗紫
    };
    Some((r, g, b, a))
}

/// 凡例カラーバー用：14 段階のラベルと色。
/// ラベルは「下限値」を表示する（例 "8" は 8〜12mm/h の区間）。
pub(crate) const LEGEND_STOPS: &[(&str, (u8, u8, u8))] = &[
    ("1",   (200, 230, 255)),
    ("2",   (160, 210, 250)),
    ("4",   (90, 170, 240)),
    ("8",   (40, 130, 230)),
    ("12",  (50, 180, 80)),
    ("16",  (140, 220, 60)),
    ("24",  (250, 230, 50)),
    ("32",  (250, 180, 30)),
    ("40",  (250, 130, 30)),
    ("48",  (240, 70, 50)),
    ("56",  (220, 40, 70)),
    ("64",  (200, 50, 180)),
    ("80",  (170, 30, 180)),
    ("80+", (120, 30, 130)),
];

/// 画像下端に凡例カラーバーを焼き込む。
/// 8 色のセグメントを横並びで描き、画像の下 4% の領域を使う。
/// 数値ラベルは TUI 側（フッター or タイトル）で別途表示する想定。
pub(crate) fn draw_legend_bar(img: &mut image::RgbaImage) {
    let w = img.width();
    let h = img.height();
    let bar_h = (h as f32 * 0.04).max(8.0) as u32;
    let bar_y0 = h - bar_h - 4;
    let pad = (w as f32 * 0.02) as u32;
    let bar_w = w - pad * 2;
    let seg = bar_w / LEGEND_STOPS.len() as u32;

    for (idx, (_, (r, g, b))) in LEGEND_STOPS.iter().enumerate() {
        let x0 = pad + (idx as u32) * seg;
        for y in 0..bar_h {
            for x in 0..seg {
                let px = x0 + x;
                let py = bar_y0 + y;
                if px < w && py < h {
                    img.put_pixel(px, py, image::Rgba([*r, *g, *b, 230]));
                }
            }
        }
        // セグメント間の細い区切り
        if idx > 0 {
            for y in 0..bar_h {
                let px = x0;
                let py = bar_y0 + y;
                if px < w && py < h {
                    img.put_pixel(px, py, image::Rgba([255, 255, 255, 255]));
                }
            }
        }
    }
    // バーの上端に細い枠
    for x in pad..pad + bar_w {
        if bar_y0 > 0 {
            img.put_pixel(x, bar_y0 - 1, image::Rgba([255, 255, 255, 200]));
        }
    }
}

/// Bilinear interpolation: (fx, fy) は 0..1 の正規化座標。
/// 画像の連続的なサンプリングでガビガビ感を消す。
pub(crate) fn sample_bilinear(img: &image::RgbaImage, fx: f64, fy: f64) -> image::Rgba<u8> {
    let w = img.width() as f64;
    let h = img.height() as f64;
    let x = (fx * w - 0.5).max(0.0).min(w - 1.0);
    let y = (fy * h - 0.5).max(0.0).min(h - 1.0);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(img.width() - 1);
    let y1 = (y0 + 1).min(img.height() - 1);
    let dx = x - x0 as f64;
    let dy = y - y0 as f64;

    let p00 = img.get_pixel(x0, y0).0;
    let p10 = img.get_pixel(x1, y0).0;
    let p01 = img.get_pixel(x0, y1).0;
    let p11 = img.get_pixel(x1, y1).0;
    let mut out = [0u8; 4];
    for c in 0..4 {
        let v = (p00[c] as f64) * (1.0 - dx) * (1.0 - dy)
            + (p10[c] as f64) * dx * (1.0 - dy)
            + (p01[c] as f64) * (1.0 - dx) * dy
            + (p11[c] as f64) * dx * dy;
        out[c] = v.clamp(0.0, 255.0) as u8;
    }
    image::Rgba(out)
}

/// 雨雲タイルから (fx, fy) 周辺の降水強度を **bilinear 補間** で取得。
/// 4 ピクセルの mm/h を線形に混ぜることで、雨雲ドットの境界が滑らかになり
/// ピクセル感（カクカクしたドット）が消える。
/// （以前は最大値採用で雨雲が膨張して見えていた）
fn sample_rain_max(img: &image::RgbaImage, fx: f64, fy: f64) -> f64 {
    let w = img.width() as f64;
    let h = img.height() as f64;
    let x = (fx * w - 0.5).max(0.0).min(w - 1.0);
    let y = (fy * h - 0.5).max(0.0).min(h - 1.0);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(img.width() - 1);
    let y1 = (y0 + 1).min(img.height() - 1);
    let dx = x - x0 as f64;
    let dy = y - y0 as f64;
    let m = |px: u32, py: u32| -> f64 {
        let p = img.get_pixel(px, py).0;
        nowcast_color_to_mmh(p[0], p[1], p[2], p[3])
    };
    let v = m(x0, y0) * (1.0 - dx) * (1.0 - dy)
        + m(x1, y0) * dx * (1.0 - dy)
        + m(x0, y1) * (1.0 - dx) * dy
        + m(x1, y1) * dx * dy;
    v
}

/// 画像中心に "+" 形状の十字を描き込む
pub(crate) fn draw_cross(img: &mut image::RgbaImage, cx: i32, cy: i32, radius: i32, color: image::Rgba<u8>) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    for d in -radius..=radius {
        let px = cx + d;
        if (0..w).contains(&px) && (0..h).contains(&cy) {
            img.put_pixel(px as u32, cy as u32, color);
        }
        let py = cy + d;
        if (0..w).contains(&cx) && (0..h).contains(&py) {
            img.put_pixel(cx as u32, py as u32, color);
        }
    }
}

/// 地理院淡色タイル PNG を 128x128 のドットグリッドに変換。
/// グレースケール明度がしきい値以下のピクセル = 線/文字あり。
/// 256x256 → 128x128 ダウンサンプル時は「2x2 のうち1つでも暗ければドットON」（線の連続性を保つため）。
fn binarize_map_tile(img: &image::RgbaImage) -> MapDotGrid {
    let (iw, ih) = (img.width() as usize, img.height() as usize);
    let mut dots = vec![vec![false; TILE_W]; TILE_H];
    // しきい値: 255 が真っ白、淡色タイルの薄いグレー線は 200 前後なので 220 で拾える
    let threshold: u32 = 220;
    let sample = 2usize;
    for j in 0..TILE_H {
        for i in 0..TILE_W {
            let mut hit = false;
            for dy in 0..sample {
                for dx in 0..sample {
                    let px = (i * iw / TILE_W + dx).min(iw - 1);
                    let py = (j * ih / TILE_H + dy).min(ih - 1);
                    let p = img.get_pixel(px as u32, py as u32);
                    // ITU-R 輝度: 0.299R + 0.587G + 0.114B
                    let lum = (299 * p.0[0] as u32 + 587 * p.0[1] as u32 + 114 * p.0[2] as u32) / 1000;
                    if lum < threshold {
                        hit = true;
                        break;
                    }
                }
                if hit {
                    break;
                }
            }
            dots[j][i] = hit;
        }
    }
    dots
}

/// 256x256 の PNG を 128x128 の降水強度グリッドに集約（2x2 ピクセル / セル、最大値採用）
fn downsample_tile(img: &image::RgbaImage) -> TileGrid {
    let (iw, ih) = (img.width() as usize, img.height() as usize);
    let mut data = vec![vec![0.0f64; TILE_W]; TILE_H];
    let sample = 2usize;
    for j in 0..TILE_H {
        for i in 0..TILE_W {
            let mut mx = 0.0f64;
            for dy in 0..sample {
                for dx in 0..sample {
                    let px = (i * iw / TILE_W + dx).min(iw - 1);
                    let py = (j * ih / TILE_H + dy).min(ih - 1);
                    let p = img.get_pixel(px as u32, py as u32);
                    let rate = nowcast_color_to_mmh(p.0[0], p.0[1], p.0[2], p.0[3]);
                    if rate > mx {
                        mx = rate;
                    }
                }
            }
            data[j][i] = mx;
        }
    }
    data
}

// === 補助関数 ===

async fn fetch_today_temp(client: &reqwest::Client, area: &AreaEntry) -> Result<f64> {
    let url = format!(
        "https://www.jma.go.jp/bosai/forecast/data/forecast/{}.json",
        area.office
    );
    let json: serde_json::Value = client.get(&url).send().await?.json().await?;
    // forecast[0] の最後の timeSeries が地点別気温
    let short = json.get(0).context("forecast[0]")?;
    let series = short
        .get("timeSeries")
        .and_then(|v| v.as_array())
        .context("timeSeries")?;
    let last = series.last().context("最後の TS")?;
    let areas = last.get("areas").and_then(|v| v.as_array()).context("areas")?;
    let a = areas.first().context("先頭 area")?;
    let temps = a.get("temps").and_then(|v| v.as_array()).context("temps")?;
    let v = temps.iter().find_map(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()));
    v.context("温度値が無い")
}

/// JMA の気象庁天気コード（数値文字列）→ 簡易文字列
fn jma_weather_code_text(code: &str) -> &'static str {
    // 詳細なテーブルは https://www.jma.go.jp/bosai/forecast/const/weather.json
    // ここでは粗い分類だけ。
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    let m = MAP.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert("100", "晴れ");
        m.insert("101", "晴れ時々曇り");
        m.insert("102", "晴れ一時雨");
        m.insert("110", "晴れのち曇り");
        m.insert("111", "晴れのち曇り");
        m.insert("112", "晴れのち雨");
        m.insert("113", "晴れのち雨");
        m.insert("200", "曇り");
        m.insert("201", "曇り時々晴れ");
        m.insert("202", "曇り一時雨");
        m.insert("203", "曇り時々雨");
        m.insert("210", "曇りのち晴れ");
        m.insert("212", "曇りのち雨");
        m.insert("300", "雨");
        m.insert("301", "雨時々晴れ");
        m.insert("302", "雨時々止む");
        m.insert("303", "雨時々雪");
        m.insert("308", "大雨");
        m.insert("311", "雨のち晴れ");
        m.insert("313", "雨のち曇り");
        m.insert("400", "雪");
        m.insert("401", "雪時々晴れ");
        m.insert("402", "雪時々止む");
        m.insert("403", "雪時々雨");
        m.insert("411", "雪のち晴れ");
        m
    });
    m.get(code).copied().unwrap_or("不明")
}

/// 緯度経度 → Web メルカトル系のタイル番号 (z, x, y)
pub(crate) fn lonlat_to_tile(lon: f64, lat: f64, zoom: u8) -> (u8, u32, u32) {
    let lat_rad = lat.to_radians();
    let n = 2f64.powi(zoom as i32);
    let xtile = ((lon + 180.0) / 360.0 * n).floor() as u32;
    let ytile = ((1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0
        * n)
        .floor() as u32;
    (zoom, xtile, ytile)
}

/// タイル番号 → 左上の (lat, lon)
pub(crate) fn tile_to_lonlat(z: u8, x: u32, y: u32) -> (f64, f64) {
    let n = 2f64.powi(z as i32);
    let lon = x as f64 / n * 360.0 - 180.0;
    let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * y as f64 / n)).sinh().atan();
    (lat_rad.to_degrees(), lon)
}

/// ナウキャスト PNG のピクセル色 → 降水強度 (mm/h)。
///
/// 気象庁ナウキャスト hrpns の公式配色:
///   1mm: 水色 (200, 242, 255)
///   5mm: 青  (33, 140, 254)
///   10mm: 緑 (0, 250, 245) 系または黄緑
///   20mm: 黄 (250, 245, 0)
///   30mm: 橙 (255, 153, 0)
///   50mm: 赤 (255, 40, 0)
///   80mm: 紫 (180, 0, 104)
///   80+ : 濃紫 (130, 0, 70)
///
/// 代表色との **ユークリッド距離** で最も近いカテゴリに分類する。
/// 閾値ベースだとアンチエイリアス境界や RGB のわずかなブレで取りこぼしが出るので、
/// 距離方式の方が堅牢で「緑のリングが消える」ような問題を防げる。
fn nowcast_color_to_mmh(r: u8, g: u8, b: u8, a: u8) -> f64 {
    if a < 30 {
        return 0.0;
    }
    // (R, G, B, mm/h) の代表色テーブル。
    // 観測した JMA タイルから実色を抽出した値ベース（推定含む）。
    const PALETTE: &[(u8, u8, u8, f64)] = &[
        (242, 242, 255, 0.5),  // ごく弱: ほぼ白っぽい水色
        (160, 210, 255, 1.0),  // 弱: 薄水色
        (33,  140, 254, 5.0),  // 普通: 青
        (0,   65,  255, 10.0), // やや強: 濃青 (or 黄緑系)
        (250, 245, 0,   20.0), // 強: 黄
        (255, 153, 0,   30.0), // 激しい: 橙
        (255, 40,  0,   50.0), // 猛烈: 赤
        (180, 0,   104, 80.0), // 極端: 紫
        (130, 0,   70,  100.0),// 80mm+: 濃紫
        // 緑系（JMA配色に含まれる場合に対応）
        (55,  188, 83,  10.0), // 緑: 10mm相当
        (140, 220, 60,  10.0), // 黄緑
    ];
    let mut best_d = f64::INFINITY;
    let mut best_mmh = 0.0_f64;
    let (rf, gf, bf) = (r as f64, g as f64, b as f64);
    for &(pr, pg, pb, mmh) in PALETTE {
        let d = (rf - pr as f64).powi(2)
            + (gf - pg as f64).powi(2)
            + (bf - pb as f64).powi(2);
        if d < best_d {
            best_d = d;
            best_mmh = mmh;
        }
    }
    // 最近傍の代表色から十分遠い場合は「凡例マーカー等のノイズ」とみなし 0 に。
    // 閾値は距離の二乗で 12000 ≒ sqrt(12000) ≈ 110 の許容範囲。
    if best_d > 12000.0 {
        return 0.0;
    }
    best_mmh
}

fn parse_jma_compact(s: &str) -> Result<DateTime<Local>> {
    // JMA ナウキャストの basetime/validtime は UTC 表記 (例: 20260608070000 = UTC 07:00)
    // なので、UTC として解釈してから Local に変換する。
    // 以前 Local として解釈してしまっていたバグで、JST だと 9 時間ずれて見えていた。
    let ndt = chrono::NaiveDateTime::parse_from_str(s, "%Y%m%d%H%M%S")
        .context("validtime parse")?;
    Ok(chrono::Utc.from_utc_datetime(&ndt).with_timezone(&Local))
}
