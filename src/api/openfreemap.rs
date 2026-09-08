//! OpenFreeMap Liberty のベクタタイルを PNG 相当の RGBA タイルへ描画する。
//!
//! OpenFreeMap はラスタ PNG を配信していないため、MapLibre の style JSON と
//! MVT を ezu（MapLibre 互換の CPU レンダラー）で描画してから、既存の
//! RainViewer/JMA 画像合成へ渡す。

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::OnceCell;

const STYLE_URL: &str = "https://tiles.openfreemap.org/styles/liberty";
const VECTOR_SOURCE: &str = "openmaptiles";
const TILE_SIZE: u32 = 256;

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
}

impl OpenFreeMapRenderer {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            state: Arc::new(OnceCell::new()),
        }
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
            "OpenFreeMap Liberty renderer initialized"
        );
        Ok(RendererState {
            tile_template,
            graph,
            cache: ezu::graph::Cache::new(),
            assets: ezu::paint::host::BrushBankLoader::default(),
            raster_sources: ezu::paint::host::build_raster_sources(&document, None),
            pad,
        })
    }
}
