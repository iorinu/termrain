// アプリ本体: 状態管理 + 入力イベント + 描画ループ
//
// 構造:
//   state    ... AppState / Msg などのデータ定義
//   fetch    ... 非同期取得タスクの spawn と Msg → state 反映
//   input    ... キー / リサイズイベントのハンドラ
//   startup  ... TUI 起動前の CLI 引数前処理 (--list-city / --dump / --save 等)
//   run()    ... 端末を raw mode に切り替え、イベントループを回す
//
// 通信(reqwest)は I/O 待ちが長いので tokio::spawn でバックグラウンドへ。
// 結果はチャンネル (mpsc) 経由でメインスレッドに戻す。

mod fetch;
mod input;
pub mod startup;
mod state;

pub use state::AppState;

use std::io::{Stdout, stdout};
use std::sync::Arc;

use anyhow::{Context, Result};
use crossterm::event::EventStream;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui_image::picker::Picker;
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

use crate::api::{WeatherProvider, select_provider};
use crate::cli::Args;
use crate::map::MapData;

use fetch::{apply_msg, spawn_fetch, spawn_map_load, spawn_radar};
use input::handle_event;
use state::Msg;

pub async fn run(args: Args) -> Result<()> {
    // 1) CLI 前処理（--list-city / --dump 等の早期終了はここで吸収）
    let Some(config) = startup::prepare(&args).await? else {
        return Ok(());
    };

    // 2) プロバイダー選択
    let provider: Arc<dyn WeatherProvider> =
        Arc::from(select_provider(&config.location.country, args.force_jma));
    let provider_name = provider.name().to_string();
    // 設定で指定された地図スタイル・言語をプロバイダーに反映
    provider.set_map_style(config.radar.map_style);
    provider.set_language(config.ui.language);

    // --dump モード: TUI を立ち上げず標準出力に出して終了
    if args.dump {
        return startup::run_dump(&provider, &config).await;
    }

    // 3) Picker 初期化（raw mode より前、stdio クエリのため）
    // 失敗してもアプリは続行可（その場合は画像表示なし、Brailleフォールバック描画）
    let image_picker = match Picker::from_query_stdio() {
        Ok(p) => {
            tracing::info!("画像レンダラー検出: {:?}", p.protocol_type());
            Some(p)
        }
        Err(e) => {
            tracing::warn!("Picker 初期化失敗（画像表示は無効）: {e:#}");
            None
        }
    };

    // 初回フェッチ用のアスペクト比は起動時の端末サイズから計算する
    let (term_w, term_h) = crossterm::terminal::size().unwrap_or((120, 40));
    let font_size = image_picker.as_ref().map(|p| p.font_size());
    let radar_aspect = crate::ui::desired_radar_aspect(term_w, term_h, font_size);

    // 4) AppState を作って TUI を起動
    let mut state = AppState {
        config,
        provider_name,
        current: None,
        hourly: Vec::new(),
        daily: Vec::new(),
        radar: None,
        map: Arc::new(MapData::default()),
        image_picker,
        radar_protocol: None,
        radar_time_offset: 0,
        radar_playing: false,
        splash_active: true,
        show_help: false,
        spinner_frame: 0,
        radar_loading: false,
        radar_aspect,
        last_error: None,
        quit: false,
    };

    let mut terminal = setup_terminal().context("ターミナル初期化失敗")?;

    // メッセージチャンネル
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();

    // 初回フェッチを spawn（天気 + 地図データ）
    spawn_fetch(
        provider.clone(),
        state.config.clone(),
        state.radar_time_offset,
        state.radar_aspect,
        tx.clone(),
    );
    spawn_map_load(tx.clone());

    // Splash を 2 秒で自動解除
    {
        let tx = tx.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(1800)).await;
            let _ = tx.send(Msg::DismissSplash);
        });
    }

    let mut events = EventStream::new();
    let mut auto_refresh = if state.config.ui.refresh_interval > 0 {
        Some(tokio::time::interval(Duration::from_secs(
            state.config.ui.refresh_interval,
        )))
    } else {
        None
    };

    // 雨雲アニメーション再生用の tick (700ms 間隔)。
    let mut anim_tick = tokio::time::interval(Duration::from_millis(700));
    anim_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // スピナー進行用の tick (120ms)
    let mut spinner_tick = tokio::time::interval(Duration::from_millis(120));
    spinner_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // 描画
    terminal.draw(|f| crate::ui::draw(f, &mut state))?;

    loop {
        tokio::select! {
            // メッセージ受信
            Some(msg) = rx.recv() => {
                apply_msg(&mut state, msg);
                terminal.draw(|f| crate::ui::draw(f, &mut state))?;
            }
            // 端末イベント
            Some(Ok(ev)) = events.next() => {
                if handle_event(&mut state, ev, &provider, &tx) {
                    terminal.draw(|f| crate::ui::draw(f, &mut state))?;
                }
                if state.quit {
                    break;
                }
            }
            // 自動更新
            _ = async {
                if let Some(t) = auto_refresh.as_mut() {
                    t.tick().await;
                } else {
                    // 自動更新が無効なら永遠に待つ
                    sleep(Duration::from_secs(60 * 60 * 24)).await;
                }
            } => {
                spawn_fetch(provider.clone(), state.config.clone(), state.radar_time_offset, state.radar_aspect, tx.clone());
            }
            // 雨雲アニメーション (playing 中のみ反映)
            _ = anim_tick.tick() => {
                if state.radar_playing {
                    let (off_min, off_max) = provider.radar_offset_range();
                    state.radar_time_offset += 1;
                    if state.radar_time_offset > off_max {
                        state.radar_time_offset = off_min;
                    }
                    state.radar_loading = true;
                    spawn_radar(provider.clone(), state.config.clone(), state.radar_time_offset, state.radar_aspect, tx.clone());
                }
            }
            // スピナー進行: 何かしらロード中 or splash 中なら再描画
            _ = spinner_tick.tick() => {
                state.spinner_frame = state.spinner_frame.wrapping_add(1);
                if state.splash_active || state.is_loading() || state.radar_loading {
                    terminal.draw(|f| crate::ui::draw(f, &mut state))?;
                }
            }
        }
    }

    restore_terminal(&mut terminal)?;
    Ok(())
}

// === ターミナル制御 ===

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
