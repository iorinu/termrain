// CLI 引数の定義。clap の derive 機能で構造体から自動生成する。
//
// なぜ derive を使うか:
//   - 引数の追加・変更が構造体の書き換えだけで済む
//   - --help の出力も自動で整形される
//   - 型安全（受け取った値が String / f64 などに型付けされる）

use clap::Parser;
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[command(
    name = "termrain",
    version,
    about = "Terminal weather forecast and rain radar TUI"
)]
pub struct Args {
    /// Look up a city by name (e.g. "Tokyo", "Paris"). Resolved via geocoding.
    #[arg(long)]
    pub city: Option<String>,

    /// Latitude (must be combined with --lon)
    #[arg(long, requires = "lon")]
    pub lat: Option<f64>,

    /// Longitude (must be combined with --lat)
    #[arg(long, requires = "lat")]
    pub lon: Option<f64>,

    /// Force the JMA provider even outside Japan (experimental)
    #[arg(long)]
    pub force_jma: bool,

    /// Skip the TUI and dump the current weather as JSON (for debugging)
    #[arg(long)]
    pub dump: bool,

    /// Override the display language (en / ja / english / japanese)
    #[arg(long = "lang", value_name = "LANG")]
    pub lang: Option<crate::i18n::Language>,

    /// Save the CLI options to ~/.config/termrain/config.toml for next time
    #[arg(long)]
    pub save: bool,

    /// Print up to 10 city candidates and exit (helps disambiguate names).
    /// Example: `termrain --list-city Ueno` → Tokyo / Mie etc.
    #[arg(long = "list-city", value_name = "QUERY")]
    pub list_city: Option<String>,

    /// Print a shell completion script to stdout and exit.
    /// Example: `termrain --completion zsh > ~/.zsh/completions/_termrain`
    #[arg(long = "completion", value_name = "SHELL")]
    pub completion: Option<Shell>,
}
