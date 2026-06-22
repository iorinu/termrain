// アプリの「現在の表示状態」を保持する構造体と、UI ↔ 非同期タスク間の
// メッセージ列挙体を置く。ロジックは持たず、データ定義に専念する。

use std::sync::Arc;

use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

use crate::api::{CurrentWeather, DailyPoint, HourlyPoint, RadarGrid};
use crate::config::Config;
use crate::map::MapData;

pub struct AppState {
    pub config: Config,
    pub provider_name: String,
    pub current: Option<CurrentWeather>,
    pub hourly: Vec<HourlyPoint>,
    pub daily: Vec<DailyPoint>,
    pub radar: Option<RadarGrid>,
    pub map: Arc<MapData>,
    /// Kitty/Sixel graphics 用の画像レンダラー。
    /// 端末がサポートしていない場合は halfblocks 等にフォールバック。
    pub image_picker: Option<Picker>,
    /// 直近の合成済みレーダー画像（StatefulProtocol 化済み）。
    pub radar_protocol: Option<StatefulProtocol>,
    /// 時系列スクラブ位置。0=最新、+1で5分後、-1で5分前。
    pub radar_time_offset: i32,
    /// アニメーション再生中かどうか。p キーで toggle。
    pub radar_playing: bool,
    /// 起動時の Splash 画面表示中か。データ取得 or 2秒経過で false に。
    pub splash_active: bool,
    /// `?` キーでヘルプモーダルを開いている状態
    pub show_help: bool,
    /// スピナーのフレーム番号。tick で +1 され、読み込み中表示に使う。
    pub spinner_frame: usize,
    /// 雨雲レーダーの取得中フラグ。spawn_radar で true、Msg::Radar 受信で false。
    /// 時刻スクラブやズーム中に「いま処理中」を UI で示すために使う。
    pub radar_loading: bool,
    /// 合成画像に要求するアスペクト比（横/縦）。端末サイズから計算し、
    /// ワイドターミナルではレーダーを横長にしてパネルを使い切る。
    pub radar_aspect: f64,
    pub last_error: Option<String>,
    pub quit: bool,
}

/// Braille スピナー文字。120ms ごとに次へ進める。
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl AppState {
    pub fn spinner(&self) -> &'static str {
        SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()]
    }
    /// 何かしらまだ読み込み中（spinner を回す対象がある）か
    pub fn is_loading(&self) -> bool {
        self.current.is_none()
            || self.radar.is_none()
            || self.hourly.is_empty()
            || self.daily.is_empty()
    }
}

// 取得結果をメインに伝えるためのメッセージ
pub enum Msg {
    Current(CurrentWeather),
    Hourly(Vec<HourlyPoint>),
    Daily(Vec<DailyPoint>),
    Radar(RadarGrid),
    Map(Arc<MapData>),
    Error(String),
    /// Splash 演出を解除する（タイマー or 主要データ取得完了で送られる）
    DismissSplash,
}
