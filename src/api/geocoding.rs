// 都市名 → 緯度経度 (+ 国コード) の解決。
// Open-Meteo Geocoding API を利用（無料・キー不要）。

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct GeoHit {
    pub name: String,
    /// 都道府県 / 州 (例: "Tokyo", "Mie")
    pub admin1: Option<String>,
    /// 国名 (例: "Japan")
    pub country_name: Option<String>,
    /// ISO2 国コード (例: "JP", "FR")
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Deserialize)]
struct Resp {
    results: Option<Vec<RespHit>>,
}
#[derive(Debug, Deserialize)]
struct RespHit {
    name: String,
    latitude: f64,
    longitude: f64,
    country_code: Option<String>,
    country: Option<String>,
    admin1: Option<String>,
}

fn to_hit(h: RespHit) -> GeoHit {
    GeoHit {
        name: h.name,
        admin1: h.admin1,
        country_name: h.country,
        country: h.country_code.unwrap_or_default(),
        latitude: h.latitude,
        longitude: h.longitude,
    }
}

/// 先頭1件を返す（既存呼び出し互換、現状は app.rs で search_many を直接呼ぶので未使用）
#[allow(dead_code)]
pub async fn search(
    client: &reqwest::Client,
    query: &str,
    lang: crate::i18n::Language,
) -> Result<GeoHit> {
    let mut hits = search_many(client, query, lang, 1).await?;
    hits.pop().context("該当する地点が見つかりません")
}

/// 候補を最大 `count` 件取得して新しい順 (Open-Meteo の優先度順) に返す。
pub async fn search_many(
    client: &reqwest::Client,
    query: &str,
    lang: crate::i18n::Language,
    count: usize,
) -> Result<Vec<GeoHit>> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count={}&language={}",
        urlencoding::encode(query),
        count.max(1).min(20),
        lang.api_code(),
    );
    let r: Resp = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let hits: Vec<GeoHit> = r
        .results
        .unwrap_or_default()
        .into_iter()
        .map(to_hit)
        .collect();
    Ok(hits)
}
