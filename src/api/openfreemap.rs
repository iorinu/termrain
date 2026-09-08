//! OpenFreeMap Liberty のベクタタイルを PNG 相当の RGBA タイルへ描画する。
//!
//! OpenFreeMap はラスタ PNG を配信していないため、MapLibre の style JSON と
//! MVT を ezu（MapLibre 互換の CPU レンダラー）で描画してから、既存の
//! RainViewer/JMA 画像合成へ渡す。

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::OnceCell;

const STYLE_URL: &str = "https://tiles.openfreemap.org/styles/liberty";
const VECTOR_SOURCE: &str = "openmaptiles";
const TILE_SIZE: u32 = 256;

/// 公式Liberty styleの道路系line-widthだけを倍率変更する。
///
/// 道路は本線と`*_casing`が別レイヤーなので、道路レイヤー全体に同じ倍率を
/// 適用する。行政境界・河川・鉄道などの幅は変更しない。
fn scale_road_widths(style: &mut Value, scale: f64) {
    let scale = if scale.is_finite() {
        scale.clamp(0.1, 2.0)
    } else {
        crate::config::DEFAULT_OPEN_FREE_MAP_ROAD_SCALE
    };
    let Some(layers) = style["layers"].as_array_mut() else {
        return;
    };

    for layer in layers {
        let is_road_layer = layer["id"].as_str().is_some_and(|id| {
            id.starts_with("road_") || id.starts_with("bridge_") || id.starts_with("tunnel_")
        });
        if !is_road_layer {
            continue;
        }
        if let Some(width) = layer["paint"].get_mut("line-width") {
            scale_line_width_expression(width, scale);
        }
    }
}

fn scale_line_width_expression(value: &mut Value, scale: f64) {
    if let Some(width) = value.as_f64() {
        *value = serde_json::json!(width * scale);
        return;
    }

    let Some(expression) = value.as_array_mut() else {
        return;
    };
    let Some(operator) = expression.first().and_then(Value::as_str) else {
        return;
    };

    let (first_output, step) = match operator {
        "interpolate" => (4, 2),
        "step" => (2, 2),
        _ => return,
    };
    for index in (first_output..expression.len()).step_by(step) {
        if let Some(width) = expression[index].as_f64() {
            expression[index] = serde_json::json!(width * scale);
        }
    }
}

#[derive(Debug, Deserialize)]
struct TileJson {
    tiles: Vec<String>,
}

struct RendererState {
    tile_template: String,
    graph: ezu::graph::Graph,
    cache: ezu::graph::Cache,
    assets: ezu::paint::host::BrushBankLoader,
    raster_sources: ezu::paint::host::RasterSourceRegistry,
    pad: u32,
}

/// OpenFreeMap の style/グラフをプロセス内で一度だけ初期化するレンダラー。
#[derive(Clone)]
pub struct OpenFreeMapRenderer {
    client: reqwest::Client,
    state: Arc<OnceCell<Arc<RendererState>>>,
    road_scale: Arc<Mutex<f64>>,
}

impl OpenFreeMapRenderer {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            state: Arc::new(OnceCell::new()),
            road_scale: Arc::new(Mutex::new(crate::config::DEFAULT_OPEN_FREE_MAP_ROAD_SCALE)),
        }
    }

    /// 描画グラフ初期化前にOpenFreeMapの道路幅倍率を設定する。
    pub fn set_road_scale(&self, scale: f64) {
        *self.road_scale.lock().unwrap() = scale;
    }

    /// 指定した XYZ タイルを Liberty style で RGBA 画像へ変換する。
    pub async fn render_tile(&self, z: u8, x: u32, y: u32) -> Result<Arc<image::RgbaImage>> {
        let state = self
            .state
            .get_or_try_init(|| async { self.initialize().await.map(Arc::new) })
            .await?
            .clone();

        let url = state
            .tile_template
            .replace("{z}", &z.to_string())
            .replace("{x}", &x.to_string())
            .replace("{y}", &y.to_string());
        let bytes = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("OpenFreeMap MVT取得失敗: {url}"))?
            .error_for_status()
            .with_context(|| format!("OpenFreeMap MVT HTTPエラー: {url}"))?
            .bytes()
            .await
            .with_context(|| format!("OpenFreeMap MVT本体取得失敗: {url}"))?;
        let decoded = ezu::features::mvt::decode(&bytes).context("OpenFreeMap MVTデコード失敗")?;
        let tile = ezu::graph::TileId { z, x, y };
        let handle = tokio::runtime::Handle::current();

        // ezu の描画と glyph/sprite の初回取得は同期 API なので、Tokio の
        // ワーカースレッドを塞がないよう spawn_blocking 内で実行する。
        tokio::task::spawn_blocking(move || {
            let canvas = ezu::graph::CanvasInfo::square(TILE_SIZE, state.pad);
            let mut loader = ezu::paint::host::TileLoader::new(&state.assets, tile);
            loader.bind_mvt(VECTOR_SOURCE, decoded);

            // Liberty の Natural Earth レイヤーは低ズームだけで使われる。
            // 通常のtermrain表示（z=11前後）では不要な3x3画像取得を避け、
            // 低ズーム時だけezuのラスタソース取得を有効にする。
            if z <= 7 {
                handle
                    .block_on(ezu::paint::host::bind_raster_sources(
                        &mut loader,
                        &state.raster_sources,
                        tile,
                        canvas,
                    ))
                    .context("OpenFreeMap Natural Earth取得失敗")?;
            } else {
                // 高ズームではNatural Earthレイヤーが非表示だが、source参照を
                // 満たすため透明なパディング済み画像を束縛しておく。
                let (width, height) = canvas.padded_dims();
                loader.bind_raster("ne2_shaded", ezu::graph::RasterBuf::new(width, height));
            }

            let output = ezu::graph::Evaluator::new(&state.graph, &state.cache, &loader)
                .render(tile, canvas, &ezu::graph::ParamValues::new(), 0)
                .context("OpenFreeMap style描画失敗")?;
            let raster = output
                .as_raster()
                .context("OpenFreeMap styleの出力がRasterではありません")?;
            let rgba = ezu::paint::host::raster_to_rgba8(raster, TILE_SIZE, state.pad);
            let image = image::RgbaImage::from_raw(TILE_SIZE, TILE_SIZE, rgba)
                .context("OpenFreeMap RGBA画像生成失敗")?;
            Ok::<_, anyhow::Error>(Arc::new(image))
        })
        .await
        .context("OpenFreeMap描画タスクが停止")?
    }

    async fn initialize(&self) -> Result<RendererState> {
        let style: Value = self
            .client
            .get(STYLE_URL)
            .send()
            .await
            .context("OpenFreeMap style取得失敗")?
            .error_for_status()
            .context("OpenFreeMap style HTTPエラー")?
            .json()
            .await
            .context("OpenFreeMap style JSONデコード失敗")?;

        let source_url = style["sources"][VECTOR_SOURCE]["url"]
            .as_str()
            .context("OpenFreeMap styleにベクタソースURLがありません")?;
        let tile_json: TileJson = self
            .client
            .get(source_url)
            .send()
            .await
            .context("OpenFreeMap TileJSON取得失敗")?
            .error_for_status()
            .context("OpenFreeMap TileJSON HTTPエラー")?
            .json()
            .await
            .context("OpenFreeMap TileJSONデコード失敗")?;
        let tile_template = tile_json
            .tiles
            .into_iter()
            .next()
            .context("OpenFreeMap TileJSONにタイルURLがありません")?;

        let road_scale = *self.road_scale.lock().unwrap();
        let mut style = style;
        scale_road_widths(&mut style, road_scale);

        let options = ezu::translate::maplibre::ConvertOptions {
            tile_size: TILE_SIZE,
            ..Default::default()
        };
        let (recipe, report) = ezu::translate::maplibre::convert(&style, &options)
            .map_err(|e| anyhow::anyhow!("OpenFreeMap style変換失敗: {e}"))?;
        for warning in report.warnings {
            tracing::warn!("OpenFreeMap style変換: {warning}");
        }

        let recipe_json =
            serde_json::to_string(&recipe).context("OpenFreeMap ezu style JSONシリアライズ失敗")?;
        let document = ezu::style::Document::from_json(&recipe_json)
            .map_err(|e| anyhow::anyhow!("OpenFreeMap ezu style解析失敗: {e}"))?;
        let mut assets = ezu::paint::host::BrushBankLoader::default();
        ezu::paint::host::prefetch_doc_assets(&document, std::path::Path::new("."), &mut assets)
            .await
            .map_err(|e| anyhow::anyhow!("OpenFreeMap style asset取得失敗: {e}"))?;
        let registry = ezu::paint::nodes::default_registry();
        let graph = ezu::graph::build_graph(&document, &registry)
            .map_err(|e| anyhow::anyhow!("OpenFreeMap ezu graph構築失敗: {e}"))?;
        let pad = document.pad.max(
            graph
                .required_pad()
                .map_err(|e| anyhow::anyhow!("OpenFreeMap ezu必要パディング計算失敗: {e}"))?,
        );

        tracing::info!(
            tile_template,
            pad,
            road_scale,
            "OpenFreeMap Liberty renderer initialized"
        );
        Ok(RendererState {
            tile_template,
            graph,
            cache: ezu::graph::Cache::new(),
            assets,
            raster_sources: ezu::paint::host::build_raster_sources(&document, None),
            pad,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::scale_road_widths;
    use serde_json::json;

    #[test]
    fn scales_road_width_outputs_without_changing_other_line_layers() {
        let mut style = json!({
            "layers": [
                {
                    "id": "road_minor_casing",
                    "type": "line",
                    "paint": {
                        "line-width": [
                            "interpolate", ["exponential", 1.2], ["zoom"],
                            12, 0.5, 20, 18
                        ]
                    }
                },
                {
                    "id": "boundary_2",
                    "type": "line",
                    "paint": {"line-width": 3}
                }
            ]
        });

        scale_road_widths(&mut style, 0.7);

        let widths = &style["layers"][0]["paint"]["line-width"];
        assert!((widths[4].as_f64().unwrap() - 0.35).abs() < f64::EPSILON);
        assert!((widths[6].as_f64().unwrap() - 12.6).abs() < f64::EPSILON);
        assert_eq!(style["layers"][1]["paint"]["line-width"], json!(3));
    }
}
