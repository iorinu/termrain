// 地図レイヤ：海岸線・都道府県境・市町村界
//
// 3 種類の GeoJSON を「初回 DL → キャッシュ → 並列ロード」する。
// データ源:
//   - 海岸線:     Natural Earth 50m coastline           (~200KB, public domain)
//   - 都道府県境: Natural Earth 50m admin_1             (~700KB, public domain, 世界対応)
//   - 市町村界:   GADM 4.1 日本 admin level 2           (~数MB, 個人利用フリー)
//
// 各データの内部形式は Vec<Polyline>。Polyline は (lon, lat) の頂点列。
// レーダー描画時は範囲内の線分だけ取り出して Canvas に重ね描く。

use anyhow::{Context, Result};
use std::path::PathBuf;

pub type Polyline = Vec<(f64, f64)>; // (lon, lat) の頂点列

#[derive(Debug, Clone, Default)]
pub struct MapData {
    pub coastlines: Vec<Polyline>,
    pub prefectures: Vec<Polyline>,
    pub municipalities: Vec<Polyline>,
}

/// 主要都市マーカー（レーダー上の地理アンカー用）。
/// 静的に持っておけば軽量で、ネットワーク不要。
pub struct City {
    pub name: &'static str,
    pub lat: f64,
    pub lon: f64,
}

pub fn cities() -> &'static [City] {
    &[
        // 日本
        City { name: "札幌",   lat: 43.0642, lon: 141.3469 },
        City { name: "仙台",   lat: 38.2682, lon: 140.8694 },
        City { name: "新潟",   lat: 37.9026, lon: 139.0232 },
        City { name: "東京",   lat: 35.6812, lon: 139.7671 },
        City { name: "横浜",   lat: 35.4437, lon: 139.6380 },
        City { name: "名古屋", lat: 35.1815, lon: 136.9066 },
        City { name: "京都",   lat: 35.0116, lon: 135.7681 },
        City { name: "大阪",   lat: 34.6937, lon: 135.5023 },
        City { name: "神戸",   lat: 34.6913, lon: 135.1830 },
        City { name: "広島",   lat: 34.3853, lon: 132.4553 },
        City { name: "高知",   lat: 33.5597, lon: 133.5311 },
        City { name: "福岡",   lat: 33.5904, lon: 130.4017 },
        City { name: "鹿児島", lat: 31.5969, lon: 130.5571 },
        City { name: "那覇",   lat: 26.2124, lon: 127.6809 },
        City { name: "金沢",   lat: 36.5613, lon: 136.6562 },
        City { name: "静岡",   lat: 34.9756, lon: 138.3828 },
        City { name: "青森",   lat: 40.8244, lon: 140.7400 },
        City { name: "盛岡",   lat: 39.7036, lon: 141.1527 },
        City { name: "松山",   lat: 33.8392, lon: 132.7657 },
        City { name: "高松",   lat: 34.3401, lon: 134.0434 },
        // 世界の主要都市
        City { name: "Seoul",     lat: 37.5665, lon: 126.9780 },
        City { name: "Beijing",   lat: 39.9042, lon: 116.4074 },
        City { name: "Shanghai",  lat: 31.2304, lon: 121.4737 },
        City { name: "Taipei",    lat: 25.0330, lon: 121.5654 },
        City { name: "Manila",    lat: 14.5995, lon: 120.9842 },
        City { name: "Bangkok",   lat: 13.7563, lon: 100.5018 },
        City { name: "Singapore", lat: 1.3521,  lon: 103.8198 },
        City { name: "Sydney",    lat: -33.8688, lon: 151.2093 },
        City { name: "New York",  lat: 40.7128, lon: -74.0060 },
        City { name: "L.A.",      lat: 34.0522, lon: -118.2437 },
        City { name: "London",    lat: 51.5074, lon: -0.1278 },
        City { name: "Paris",     lat: 48.8566, lon: 2.3522 },
        City { name: "Berlin",    lat: 52.5200, lon: 13.4050 },
        City { name: "Rome",      lat: 41.9028, lon: 12.4964 },
        City { name: "Moscow",    lat: 55.7558, lon: 37.6173 },
    ]
}

impl MapData {
    /// 3 種のレイヤを並列に取得。個別の失敗は飲み込んで空 Vec にする。
    pub async fn load(client: &reqwest::Client) -> Result<Self> {
        let coast_fut = fetch_geojson_lines(
            client,
            "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_50m_coastline.geojson",
            "ne_50m_coastline.geojson",
            "海岸線",
        );
        let pref_fut = fetch_geojson_lines(
            client,
            "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_50m_admin_1_states_provinces.geojson",
            "ne_50m_admin_1.geojson",
            "都道府県境",
        );
        let muni_fut = fetch_geojson_lines(
            client,
            "https://geodata.ucdavis.edu/gadm/gadm4.1/json/gadm41_JPN_2.json",
            "gadm41_JPN_2.json",
            "市町村界",
        );

        let (coast_r, pref_r, muni_r) = tokio::join!(coast_fut, pref_fut, muni_fut);

        let coastlines = unwrap_or_warn(coast_r, "海岸線");
        let prefectures = unwrap_or_warn(pref_r, "都道府県境");
        let municipalities = unwrap_or_warn(muni_r, "市町村界");

        tracing::info!(
            "地図データ読込完了: 海岸線 {} / 県境 {} / 市町村 {} polylines",
            coastlines.len(),
            prefectures.len(),
            municipalities.len()
        );
        Ok(Self {
            coastlines,
            prefectures,
            municipalities,
        })
    }

    /// 与えた polyline 群から、bounds 範囲内のセグメント端点ペアを返す。
    /// （海岸線でも県境でも市町村界でも共通の使い方ができる）
    pub fn segments_for(
        &self,
        which: Layer,
        bounds: ((f64, f64), (f64, f64)),
    ) -> Vec<(f64, f64, f64, f64)> {
        let lines = match which {
            Layer::Coast => &self.coastlines,
            Layer::Prefecture => &self.prefectures,
            Layer::Municipality => &self.municipalities,
        };
        extract_segments(lines, bounds)
    }
}

/// レイヤ識別子
#[derive(Debug, Clone, Copy)]
pub enum Layer {
    Coast,
    Prefecture,
    Municipality,
}

fn extract_segments(
    lines: &[Polyline],
    bounds: ((f64, f64), (f64, f64)),
) -> Vec<(f64, f64, f64, f64)> {
    let (lat_min, lon_min) = bounds.0;
    let (lat_max, lon_max) = bounds.1;
    let mut out = Vec::new();
    for line in lines {
        // 大雑把なバウンディングボックスチェック（どの頂点も範囲外なら丸ごとスキップ）
        let any_in = line.iter().any(|(lon, lat)| {
            *lat >= lat_min && *lat <= lat_max && *lon >= lon_min && *lon <= lon_max
        });
        if !any_in {
            continue;
        }
        for w in line.windows(2) {
            let (lon1, lat1) = w[0];
            let (lon2, lat2) = w[1];
            out.push((lon1, lat1, lon2, lat2));
        }
    }
    out
}

fn unwrap_or_warn(r: Result<Vec<Polyline>>, label: &str) -> Vec<Polyline> {
    match r {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("{label} 取得失敗: {e:#}");
            Vec::new()
        }
    }
}

fn cache_dir() -> Option<PathBuf> {
    crate::config::cache_dir()
}

/// 指定 URL から GeoJSON を取得（キャッシュ有り）、ポリライン列にパースする。
async fn fetch_geojson_lines(
    client: &reqwest::Client,
    url: &str,
    cache_name: &str,
    label: &str,
) -> Result<Vec<Polyline>> {
    let dir = cache_dir().context("キャッシュディレクトリ取得失敗")?;
    let path = dir.join(cache_name);

    let bytes = if path.exists() {
        tokio::fs::read(&path).await?
    } else {
        tracing::info!("{label} を初回ダウンロード: {url}");
        let resp = client.get(url).send().await?.error_for_status()?;
        let bytes = resp.bytes().await?.to_vec();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, &bytes).await?;
        bytes
    };

    parse_geojson_lines(&bytes).with_context(|| format!("{label} のパース失敗"))
}

/// GeoJSON から LineString / MultiLineString / Polygon / MultiPolygon を
/// すべてポリライン（線分列）として抽出する。
///
/// 行政界（県境・市町村界）は Polygon / MultiPolygon で表現されることが多い。
/// Polygon の外側リング・内側リング（穴）すべてを Polyline として扱えば、
/// 結果として「境界線」を線として描画できる。
fn parse_geojson_lines(bytes: &[u8]) -> Result<Vec<Polyline>> {
    use geojson::{GeoJson, Geometry, GeometryValue, Position};
    let text = std::str::from_utf8(bytes).context("GeoJSONがUTF-8でない")?;
    let gj: GeoJson = text.parse().context("GeoJSONパース失敗")?;

    let mut out: Vec<Polyline> = Vec::new();

    fn push_line(out: &mut Vec<Polyline>, line: &[Position]) {
        let mut poly: Polyline = Vec::with_capacity(line.len());
        for p in line {
            if p.len() >= 2 {
                poly.push((p[0], p[1])); // (lon, lat)
            }
        }
        if poly.len() >= 2 {
            out.push(poly);
        }
    }

    fn handle_geom(out: &mut Vec<Polyline>, geom: &Geometry) {
        match &geom.value {
            GeometryValue::LineString { coordinates } => push_line(out, coordinates),
            GeometryValue::MultiLineString { coordinates } => {
                for l in coordinates {
                    push_line(out, l);
                }
            }
            GeometryValue::Polygon { coordinates } => {
                // 外側リング + 内側リング（穴）すべてを線として描く
                for ring in coordinates {
                    push_line(out, ring);
                }
            }
            GeometryValue::MultiPolygon { coordinates } => {
                for poly in coordinates {
                    for ring in poly {
                        push_line(out, ring);
                    }
                }
            }
            GeometryValue::GeometryCollection { geometries } => {
                for g in geometries {
                    handle_geom(out, g);
                }
            }
            _ => {}
        }
    }

    match gj {
        GeoJson::FeatureCollection(fc) => {
            for f in fc.features {
                if let Some(g) = f.geometry {
                    handle_geom(&mut out, &g);
                }
            }
        }
        GeoJson::Feature(f) => {
            if let Some(g) = f.geometry {
                handle_geom(&mut out, &g);
            }
        }
        GeoJson::Geometry(g) => handle_geom(&mut out, &g),
    }

    Ok(out)
}
