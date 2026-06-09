// CLI 引数の定義。clap の derive 機能で構造体から自動生成する。
//
// なぜ derive を使うか:
//   - 引数の追加・変更が構造体の書き換えだけで済む
//   - --help の出力も自動で整形される
//   - 型安全（受け取った値が String / f64 などに型付けされる）

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "termrain", version, about = "ターミナルで天気予報と雨雲レーダー")]
pub struct Args {
    /// 都市名で地点指定（例: "Tokyo", "Paris"）。指定時は Geocoding で解決する。
    #[arg(long)]
    pub city: Option<String>,

    /// 緯度（--lon と組で指定）
    #[arg(long, requires = "lon")]
    pub lat: Option<f64>,

    /// 経度（--lat と組で指定）
    #[arg(long, requires = "lat")]
    pub lon: Option<f64>,

    /// 強制的に JMA を使う（緯度経度が日本国外でも実験用に使いたい時など）
    #[arg(long)]
    pub force_jma: bool,

    /// TUI を起動せず、現在の天気を JSON でダンプ（デバッグ用）
    #[arg(long)]
    pub dump: bool,

    /// 表示言語を上書き (en / ja / english / japanese)
    #[arg(long = "lang", value_name = "LANG")]
    pub lang: Option<crate::i18n::Language>,

    /// CLI で指定した内容を ~/.config/termrain/config.toml に保存して次回も使う
    #[arg(long)]
    pub save: bool,

    /// 同名都市を区別するため、検索候補を最大10件表示して終了する。
    /// 例: `termrain --list-city Ueno`  → Tokyo / Mie などが並ぶ
    #[arg(long = "list-city", value_name = "QUERY")]
    pub list_city: Option<String>,
}
