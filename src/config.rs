// 設定ファイル（~/.config/termrain/config.toml）の読み書き
//
// TOML を使う理由:
//   - 人間が手で編集しやすい
//   - serde で構造体に直接デシリアライズできる
//   - Rust 周辺のツール（Cargo.toml もそう）でデファクト

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub location: Location,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub radar: RadarConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    /// "JP" なら気象庁、それ以外は Open-Meteo を使う判定に利用
    pub country: String,
}

impl Default for Location {
    fn default() -> Self {
        // デフォルトは東京駅
        Self {
            name: "Tokyo".into(),
            latitude: 35.6812,
            longitude: 139.7671,
            country: "JP".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// "metric" or "imperial"
    pub unit: String,
    /// 自動更新間隔（秒）。0 で無効
    pub refresh_interval: u64,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            unit: "metric".into(),
            refresh_interval: 600,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapStyle {
    /// 国土地理院 標準地図（線画、文字くっきり）
    GsiStd,
    /// CARTO Voyager（モダン、Yahoo/Google 風）
    CartoVoyager,
    /// 国土地理院 シームレス航空写真（衛星画像）
    GsiPhoto,
}

impl MapStyle {
    pub fn next(self) -> Self {
        match self {
            Self::GsiStd => Self::CartoVoyager,
            Self::CartoVoyager => Self::GsiPhoto,
            Self::GsiPhoto => Self::GsiStd,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::GsiStd => "国土地理院 標準",
            Self::CartoVoyager => "CARTO Voyager (© OSM, © CARTO)",
            Self::GsiPhoto => "国土地理院 航空写真",
        }
    }
    pub fn tile_url(self, z: u8, x: u32, y: u32) -> String {
        match self {
            Self::GsiStd => format!(
                "https://cyberjapandata.gsi.go.jp/xyz/std/{}/{}/{}.png",
                z, x, y
            ),
            Self::CartoVoyager => format!(
                "https://basemaps.cartocdn.com/rastertiles/voyager/{}/{}/{}.png",
                z, x, y
            ),
            Self::GsiPhoto => format!(
                "https://cyberjapandata.gsi.go.jp/xyz/seamlessphoto/{}/{}/{}.jpg",
                z, x, y
            ),
        }
    }
    pub fn cache_key(self) -> &'static str {
        match self {
            Self::GsiStd => "gsi_std",
            Self::CartoVoyager => "carto_voyager",
            Self::GsiPhoto => "gsi_photo",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarConfig {
    /// 気象庁ナウキャストのタイルズーム（推奨: 6〜10）
    pub zoom: u8,
    /// 背景地図のスタイル
    #[serde(default = "default_map_style")]
    pub map_style: MapStyle,
}

fn default_map_style() -> MapStyle {
    MapStyle::CartoVoyager
}

impl Default for RadarConfig {
    fn default() -> Self {
        // ズーム 11 = 約 16km 四方 = 市内の区レベル。
        // 地元を見るのに丁度よい粒度。広域は `-` で段階的に。
        // zoom >= 11 は JMA タイルが z=10 までしか無いので、内部でクロップ拡大する。
        Self {
            zoom: 11,
            map_style: MapStyle::CartoVoyager,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            location: Location::default(),
            ui: UiConfig::default(),
            radar: RadarConfig::default(),
        }
    }
}

impl Config {
    /// 設定ディレクトリ（XDG 準拠）。
    /// 優先順位:
    ///   1. $XDG_CONFIG_HOME/termrain
    ///   2. $HOME/.config/termrain
    /// Mac でも `~/Library/...` ではなく `~/.config/termrain` を使う（ユーザー希望）。
    pub fn dir() -> Option<PathBuf> {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg).join("termrain"));
            }
        }
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/termrain"))
    }

    /// 設定ファイルパス（~/.config/termrain/config.toml）
    pub fn path() -> Option<PathBuf> {
        Self::dir().map(|d| d.join("config.toml"))
    }

    /// 設定ファイルを読む。無ければデフォルトを **自動作成して** 返す。
    pub fn load_or_default() -> Result<Self> {
        let Some(path) = Self::path() else {
            return Ok(Self::default());
        };
        if path.exists() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("設定ファイル読込: {}", path.display()))?;
            let cfg: Config = toml::from_str(&text)
                .with_context(|| format!("設定ファイルのTOMLパース: {}", path.display()))?;
            return Ok(cfg);
        }
        // 初回起動: デフォルト設定をファイルに書き出して案内する
        let cfg = Self::default();
        if let Err(e) = cfg.save() {
            // 書き込み失敗してもアプリは動かしたいので警告だけ
            tracing::warn!("初回設定ファイル書き込み失敗 {}: {e:#}", path.display());
        } else {
            tracing::info!("初回設定ファイルを作成: {}", path.display());
        }
        Ok(cfg)
    }

    /// 設定ファイルへの保存（親ディレクトリが無ければ作る）
    pub fn save(&self) -> Result<()> {
        let Some(path) = Self::path() else {
            anyhow::bail!("HOME も XDG_CONFIG_HOME も取得できませんでした");
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(&path, text)
            .with_context(|| format!("設定ファイル書き込み: {}", path.display()))?;
        Ok(())
    }
}

/// キャッシュディレクトリ（XDG 準拠）。
/// ログや地図タイルキャッシュなど、消えても困らないデータの置き場所。
pub fn cache_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("termrain"));
        }
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache/termrain"))
}
