// 国際化 (i18n)
//
// UI 文字列を Language ごとにまとめて保持する。各パネルからは
// `i18n::strings(state.config.ui.language)` で対応するテーブルを参照する。
//
// 動的な天気概況 (例: JMA の「大阪府は…」) は外部 API のデータなので、
// 言語切替が効かない部分もある。その場合は API レイヤで言語別ソースを切替える。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[value(alias = "en")]
    English,
    #[value(alias = "ja")]
    Japanese,
}

impl Default for Language {
    fn default() -> Self {
        Self::English
    }
}

impl Language {
    /// Open-Meteo の geocoding/forecast 用言語コード
    pub fn api_code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Japanese => "ja",
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct Strings {
    // Panel titles
    pub current_title: &'static str,
    pub radar_title: &'static str,
    pub hourly_title: &'static str,
    pub weekly_title: &'static str,
    // Common labels
    pub temp: &'static str,
    pub humidity: &'static str,
    pub wind: &'static str,
    pub observed: &'static str,
    pub loading: &'static str,
    pub fetching: &'static str,
    pub no_data: &'static str,
    pub no_precip: &'static str,
    // Footer keys
    pub key_help: &'static str,
    pub key_quit: &'static str,
    pub key_refresh: &'static str,
    pub key_move: &'static str,
    pub key_time: &'static str,
    pub key_play: &'static str,
    // Help modal
    pub help_title: &'static str,
    pub help_keys_section: &'static str,
    pub help_legend_section: &'static str,
    pub help_sources_section: &'static str,
    pub help_close_hint: &'static str,
    pub help_q_esc: &'static str,
    pub help_q_esc_desc: &'static str,
    pub help_qmark_desc: &'static str,
    pub help_r_desc: &'static str,
    pub help_zoom_desc: &'static str,
    pub help_move_desc: &'static str,
    pub help_scrub_desc: &'static str,
    pub help_play_desc: &'static str,
    pub help_map_desc: &'static str,
    pub help_source_rain_jp: &'static str,
    pub help_source_rain_global: &'static str,
    pub help_source_map: &'static str,
    pub help_source_rain_jp_value: &'static str,
    pub help_source_rain_global_value: &'static str,
    pub help_source_map_value: &'static str,
    // Hourly chart inline labels
    pub hourly_temp_label: &'static str,
    pub hourly_precip_label: &'static str,
    pub precip_amount_label: &'static str, // "降水量 (最大 {x}mm/h)"
    pub precip_prob_label: &'static str,   // "降水確率 (...)"
    pub no_precip_data: &'static str,
    // Splash
    pub splash_tagline: &'static str,
    pub splash_starting: &'static str,
}

const EN: Strings = Strings {
    current_title: "Now",
    radar_title: "Radar",
    hourly_title: "Hourly  🌡 Temp  💧 Rain",
    weekly_title: "Forecast",

    temp: "Temp ",
    humidity: "Humid",
    wind: "Wind ",
    observed: "Obs",
    loading: "Loading…",
    fetching: "Fetching…",
    no_data: "No data",
    no_precip: "No rain",

    key_help: "Help",
    key_quit: "Quit",
    key_refresh: "Reload",
    key_move: "Pan",
    key_time: "Time",
    key_play: "Play",

    help_title: " ❓ Help ",
    help_keys_section: "Keys",
    help_legend_section: "Rain legend (mm/h)",
    help_sources_section: "Data sources",
    help_close_hint: "  Press any key to close",
    help_q_esc: "q / Esc",
    help_q_esc_desc: "Quit",
    help_qmark_desc: "Toggle this help",
    help_r_desc: "Refetch at current time",
    help_zoom_desc: "Zoom (city ~ block)",
    help_move_desc: "Move location (~2km)",
    help_scrub_desc: "Scrub radar time back / forward",
    help_play_desc: "Toggle radar animation",
    help_map_desc: "Map style (CARTO / GSI / Aerial)",
    help_source_rain_jp: "Rain (JP)",
    help_source_rain_global: "Rain (intl.)",
    help_source_map: "Map",
    help_source_rain_jp_value: "JMA Nowcast",
    help_source_rain_global_value: "Open-Meteo",
    help_source_map_value: "CARTO / GSI",

    hourly_temp_label: "Temp°C",
    hourly_precip_label: "Rain",
    precip_amount_label: "Precipitation (max {:.1} mm/h)",
    precip_prob_label: "Probability of precip (JMA omits volume, using probability)",
    no_precip_data: "No precipitation data",

    splash_tagline: "  Terminal weather & rain radar",
    splash_starting: "  Starting…",
};

const JA: Strings = Strings {
    current_title: "現在の天気",
    radar_title: "雨雲レーダー",
    hourly_title: "時間別予報  🌡 気温  💧 降水",
    weekly_title: "週間予報",

    temp: "気温",
    humidity: "湿度",
    wind: "風速",
    observed: "観測",
    loading: "読み込み中…",
    fetching: "取得中…",
    no_data: "データなし",
    no_precip: "降水なし",

    key_help: "ヘルプ",
    key_quit: "終了",
    key_refresh: "更新",
    key_move: "移動",
    key_time: "時刻",
    key_play: "再生",

    help_title: " ❓ ヘルプ ",
    help_keys_section: "キー操作",
    help_legend_section: "雨雲の色凡例 (mm/h)",
    help_sources_section: "データソース",
    help_close_hint: "  何かキーを押して閉じる",
    help_q_esc: "q / Esc",
    help_q_esc_desc: "終了",
    help_qmark_desc: "ヘルプを開く/閉じる",
    help_r_desc: "現在時刻に戻して再取得",
    help_zoom_desc: "ズーム（市〜街区レベル）",
    help_move_desc: "地点の移動（約2km）",
    help_scrub_desc: "雨雲を時系列で前後にスクラブ",
    help_play_desc: "雨雲アニメ再生 toggle",
    help_map_desc: "地図スタイル (CARTO / 標準 / 航空)",
    help_source_rain_jp: "雨雲 (国内)",
    help_source_rain_global: "雨雲 (海外)",
    help_source_map: "地図",
    help_source_rain_jp_value: "気象庁ナウキャスト",
    help_source_rain_global_value: "Open-Meteo",
    help_source_map_value: "CARTO / 国土地理院",

    hourly_temp_label: "気温℃",
    hourly_precip_label: "降水",
    precip_amount_label: "降水量 (最大 {:.1}mm/h)",
    precip_prob_label: "降水確率 (%、JMAは降水量未配信のため代用)",
    no_precip_data: "降水データなし",

    splash_tagline: "  ターミナルの天気予報・雨雲レーダー",
    splash_starting: "  起動中…",
};

pub fn strings(lang: Language) -> &'static Strings {
    match lang {
        Language::English => &EN,
        Language::Japanese => &JA,
    }
}
