// API 層の共通部分。プロバイダー（JMA / Open-Meteo）を抽象化する。
//
// なぜ trait で抽象化するか:
//   - UI 側は「天気データ」だけ受け取れれば良く、どのプロバイダーかは知りたくない
//   - 国内/国外で切り替える条件を 1 箇所（factory 関数）に閉じ込められる
//   - 後から別プロバイダー（OpenWeatherMap など）を足しやすい

pub mod geocoding;
pub mod jma;
pub mod open_meteo;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Local};

/// 現在の天気
#[derive(Debug, Clone)]
pub struct CurrentWeather {
    pub observed_at: DateTime<Local>,
    pub condition: String,    // "晴れ" "曇り" など人間向け文字列
    pub icon: WeatherIcon,    // UI でアイコン表示するための列挙
    pub temperature_c: f64,
    pub humidity_pct: Option<f64>,
    pub wind_speed_ms: Option<f64>,
    /// 風向き（北=0°、東=90°）。将来 UI で矢印アイコン化するため保持。
    #[allow(dead_code)]
    pub wind_direction_deg: Option<f64>,
}

/// 時間別予報の 1 ポイント
#[derive(Debug, Clone)]
pub struct HourlyPoint {
    pub time: DateTime<Local>,
    pub temperature_c: f64,
    pub precipitation_mm: f64,
    pub precipitation_prob_pct: Option<f64>,
    pub icon: WeatherIcon,
}

/// 日別予報の 1 日
#[derive(Debug, Clone)]
pub struct DailyPoint {
    pub date: chrono::NaiveDate,
    /// 天気テキスト（"曇り一時雨" など）。将来 UI で詳細表示用に保持。
    #[allow(dead_code)]
    pub condition: String,
    pub icon: WeatherIcon,
    pub temp_max_c: Option<f64>,
    pub temp_min_c: Option<f64>,
    pub precipitation_prob_pct: Option<f64>,
}

/// 雨雲レーダー用のグリッドデータ
/// `data[y][x]` = mm/h の降水量
#[derive(Debug, Clone)]
pub struct RadarGrid {
    pub width: usize,
    pub height: usize,
    pub data: Vec<Vec<f64>>,
    /// 地図ドット（true = 線/文字あり）。空の場合は背景地図なし。
    /// data と同じ width × height のサイズで対応。
    pub map_dots: Vec<Vec<bool>>,
    /// Kitty/Sixel graphics 用の合成済み画像（地図 + 雨雲）。
    /// 対応端末がある場合、こちらを優先表示する。
    pub composite_image: Option<image::DynamicImage>,
    /// 左下と右上の (lat, lon)
    pub bounds: ((f64, f64), (f64, f64)),
    pub observed_at: DateTime<Local>,
}

/// アイコン分類。文字列の天気予報を粗く分類して UI 表示に使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherIcon {
    Sunny,
    PartlyCloudy,
    Cloudy,
    Rain,
    Thunder,
    Snow,
    Unknown,
}

impl WeatherIcon {
    /// UI 表示用の絵文字（1〜2文字）
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Sunny => "☀",
            Self::PartlyCloudy => "⛅",
            Self::Cloudy => "☁",
            Self::Rain => "🌧",
            Self::Thunder => "⛈",
            Self::Snow => "❄",
            Self::Unknown => "・",
        }
    }
}

/// 天気予報プロバイダー
#[async_trait]
pub trait WeatherProvider: Send + Sync {
    /// プロバイダー名（UI のステータスバー表示用）
    fn name(&self) -> &'static str;

    async fn current(&self, lat: f64, lon: f64) -> Result<CurrentWeather>;
    async fn hourly(&self, lat: f64, lon: f64) -> Result<Vec<HourlyPoint>>;
    async fn daily(&self, lat: f64, lon: f64) -> Result<Vec<DailyPoint>>;
    /// `time_offset` は targetTimes 配列内での相対インデックス。
    /// 0=最新（現在）、負=過去、正=未来予測。範囲外は最寄りにクランプ。
    async fn radar(&self, lat: f64, lon: f64, zoom: u8, time_offset: i32) -> Result<RadarGrid>;

    /// 背景地図スタイルの切替（JMA だけが対応、Open-Meteo は無視）
    fn set_map_style(&self, _style: crate::config::MapStyle) {}

    /// 表示言語の切替（Open-Meteo が wmo_to_text の言語切替に使う）
    fn set_language(&self, _lang: crate::i18n::Language) {}
}

/// 国コードからプロバイダーを選択。
/// "JP" → 気象庁、それ以外 → Open-Meteo。
pub fn select_provider(country: &str, force_jma: bool) -> Box<dyn WeatherProvider> {
    if force_jma || country.eq_ignore_ascii_case("JP") {
        Box::new(jma::Jma::new())
    } else {
        Box::new(open_meteo::OpenMeteo::new())
    }
}
