// termrain: ターミナルで天気予報と雨雲レーダーを見る TUI アプリ
//
// このファイルはエントリポイント。役割は最小限に絞り、
//   1. CLI 引数のパース
//   2. ログ出力先の初期化
//   3. tokio ランタイムでアプリ本体を起動
// だけにしている。アプリのロジックは app モジュールに分離する。

mod api;
mod app;
mod cli;
mod config;
mod map;
mod render;
mod ui;

use anyhow::Result;
use clap::Parser;

// tokio::main で非同期ランタイムを起動する。
// flavor = "multi_thread" にしているのは、HTTP 取得 (reqwest) と
// 端末イベント受信を並行に走らせたいから。
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    // 1) CLI 引数（地点指定など）をまずパース
    let args = cli::Args::parse();

    // 2) ログを「ファイル」に出す。
    //    TUI は端末を全画面で乗っ取るので、println! や標準の tracing 出力
    //    （stdout/stderr 行き）は画面を壊してしまう。なのでファイル送りにする。
    let _guard = init_logging()?;

    tracing::info!("termrain 起動: args = {:?}", args);

    // 3) アプリ本体を実行
    app::run(args).await?;

    Ok(())
}

// ログ初期化。返り値の WorkerGuard を main 関数のスコープで保持しないと、
// 非同期書き込みが flush される前にプロセスが終わってログが落ちる可能性がある。
fn init_logging() -> Result<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::EnvFilter;

    let log_dir = config::cache_dir().unwrap_or_else(|| std::env::temp_dir().join("termrain"));
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "termrain.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // RUST_LOG が設定されていればそちらを優先、無ければ info 以上を出す
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false) // ファイル出力なのでカラーコードを無効化
        .init();

    Ok(guard)
}
