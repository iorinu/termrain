// アプリ本体: 状態管理 + 入力イベント + 描画ループ
//
// 構造:
//   AppState  ... 描画に必要な「現在の表示状態」
//   run()     ... 端末を raw mode に切り替え、イベントループを回す
//
// 通信(reqwest)は I/O 待ちが長いので tokio::spawn でバックグラウンドへ。
// 結果はチャンネル (mpsc) 経由でメインスレッドに戻す。

use anyhow::{Context, Result};
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{Stdout, stdout};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

use crate::api::{
    CurrentWeather, DailyPoint, HourlyPoint, RadarGrid, WeatherProvider, select_provider,
};
use crate::cli::Args;
use crate::config::Config;
use crate::map::MapData;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

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
enum Msg {
    Current(CurrentWeather),
    Hourly(Vec<HourlyPoint>),
    Daily(Vec<DailyPoint>),
    Radar(RadarGrid),
    Map(Arc<MapData>),
    Error(String),
    /// Splash 演出を解除する（タイマー or 主要データ取得完了で送られる）
    DismissSplash,
}

pub async fn run(args: Args) -> Result<()> {
    // 1) 設定読込（無ければデフォルト）
    let mut config = Config::load_or_default()?;

    // 1.5) --lang は他の処理に先んじて反映（geocoding の language にも使うため）
    if let Some(lang) = args.lang {
        config.ui.language = lang;
    }

    // --list-city: 候補を表示して終了
    if let Some(query) = args.list_city.as_ref() {
        let client = reqwest::Client::builder()
            .user_agent("termrain/0.1")
            .build()?;
        let hits =
            crate::api::geocoding::search_many(&client, query, config.ui.language, 10).await?;
        if hits.is_empty() {
            eprintln!("No matches for: {query}");
        } else {
            println!("Candidates for \"{}\":\n", query);
            for (i, h) in hits.iter().enumerate() {
                let place = match (h.admin1.as_deref(), h.country_name.as_deref()) {
                    (Some(a), Some(c)) => format!("{}, {}", a, c),
                    (Some(a), None) => a.to_string(),
                    (None, Some(c)) => c.to_string(),
                    _ => String::new(),
                };
                println!(
                    "  {:>2}. {:<24} {:<32}  lat={:>8.4}  lon={:>9.4}",
                    i + 1,
                    h.name,
                    place,
                    h.latitude,
                    h.longitude,
                );
            }
            println!("\nUse one of:");
            if let Some(top) = hits.first() {
                println!(
                    "  termrain --lat {:.4} --lon {:.4}",
                    top.latitude, top.longitude
                );
            }
        }
        return Ok(());
    }

    // 2) CLI 引数で上書き
    if let Some(city) = &args.city {
        let client = reqwest::Client::builder()
            .user_agent("termrain/0.1")
            .build()?;
        match crate::api::geocoding::search_many(&client, city, config.ui.language, 5).await {
            Ok(hits) if !hits.is_empty() => {
                let hit = &hits[0];
                config.location.name = hit.name.clone();
                config.location.latitude = hit.latitude;
                config.location.longitude = hit.longitude;
                config.location.country = hit.country.clone();
                // 同名候補が複数あれば「他にもこれだけある」と案内
                if hits.len() > 1 {
                    let chosen_loc = match (&hit.admin1, &hit.country_name) {
                        (Some(a), Some(c)) => format!("{}, {}", a, c),
                        (Some(a), None) => a.clone(),
                        (None, Some(c)) => c.clone(),
                        _ => String::new(),
                    };
                    eprintln!(
                        "\"{}\" → {} ({}) を採用。他の候補は `termrain --list-city {}` で確認可。",
                        city, hit.name, chosen_loc, city
                    );
                }
            }
            Ok(_) => {
                eprintln!("該当する地点が見つかりません: {city}");
            }
            Err(e) => {
                eprintln!("地点検索に失敗: {e:#}");
            }
        }
    }
    if let (Some(lat), Some(lon)) = (args.lat, args.lon) {
        config.location.latitude = lat;
        config.location.longitude = lon;
        // 都市名が未指定なら緯度経度ベースの判定で国も切り替え。
        // 名前は「Custom」程度にしておき、座標はヘッダー側で別途表示する。
        if args.city.is_none() {
            config.location.name = "Custom".into();
            if lat > 24.0 && lat < 46.0 && lon > 122.0 && lon < 146.0 {
                config.location.country = "JP".into();
            } else {
                config.location.country = "".into();
            }
        }
    }

    // 2.5) --save: ここまでで決まった設定を ~/.config/termrain/config.toml に保存
    //      （CLI 引数で指定した内容を次回以降のデフォルトにする）
    if args.save {
        if let Err(e) = config.save() {
            eprintln!("設定の保存に失敗: {e:#}");
        } else if let Some(p) = Config::path() {
            eprintln!("設定を保存しました: {}", p.display());
        }
    }

    // 3) プロバイダー選択
    let provider: Arc<dyn WeatherProvider> =
        Arc::from(select_provider(&config.location.country, args.force_jma));
    let provider_name = provider.name().to_string();
    // 設定で指定された地図スタイル・言語をプロバイダーに反映
    provider.set_map_style(config.radar.map_style);
    provider.set_language(config.ui.language);

    // --dump モード: TUI を立ち上げず標準出力に出して終了
    if args.dump {
        let lat = config.location.latitude;
        let lon = config.location.longitude;
        let cur = provider.current(lat, lon).await?;
        println!("{:#?}", cur);
        let h = provider.hourly(lat, lon).await?;
        println!("hourly: {} points", h.len());
        let d = provider.daily(lat, lon).await?;
        println!("daily: {} days", d.len());
        return Ok(());
    }

    // 4) Picker 初期化（raw mode より前、stdio クエリのため）
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

    // 5) AppState を作って TUI を起動
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

fn apply_msg(state: &mut AppState, msg: Msg) {
    match msg {
        Msg::Current(c) => state.current = Some(c),
        Msg::Hourly(h) => state.hourly = h,
        Msg::Daily(d) => state.daily = d,
        Msg::Radar(r) => {
            // 合成画像があれば StatefulProtocol 化（描画時にパネル領域に動的フィット）
            if let (Some(picker), Some(img)) =
                (state.image_picker.as_mut(), r.composite_image.as_ref())
            {
                let p = picker.new_resize_protocol(img.clone());
                state.radar_protocol = Some(p);
            }
            state.radar = Some(r);
            state.radar_loading = false;
        }
        Msg::Map(m) => state.map = m,
        Msg::Error(e) => state.last_error = Some(e),
        Msg::DismissSplash => state.splash_active = false,
    }
}

/// 地図データ（海岸線）を非同期にロードする。
/// 失敗してもアプリは続行できる（地図なしでレーダーは描ける）。
fn spawn_map_load(tx: mpsc::UnboundedSender<Msg>) {
    tokio::spawn(async move {
        let client = match reqwest::Client::builder()
            .user_agent("termrain/0.1")
            .timeout(std::time::Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Msg::Error(format!("map client: {e:#}")));
                return;
            }
        };
        match MapData::load(&client).await {
            Ok(m) => {
                let _ = tx.send(Msg::Map(Arc::new(m)));
            }
            Err(e) => {
                let _ = tx.send(Msg::Error(format!("map: {e:#}")));
            }
        }
    });
}

fn handle_event(
    state: &mut AppState,
    ev: Event,
    provider: &Arc<dyn WeatherProvider>,
    tx: &mpsc::UnboundedSender<Msg>,
) -> bool {
    // リサイズ時: 理想アスペクト比が大きく変わったらレーダーを再取得する
    // （ドラッグ中のイベント連発は radar_loading ガードで間引く）
    if let Event::Resize(w, h) = ev {
        let font = state.image_picker.as_ref().map(|p| p.font_size());
        let desired = crate::ui::desired_radar_aspect(w, h, font);
        if (desired - state.radar_aspect).abs() > 0.15 && !state.radar_loading {
            state.radar_aspect = desired;
            state.radar_loading = true;
            spawn_radar(
                provider.clone(),
                state.config.clone(),
                state.radar_time_offset,
                state.radar_aspect,
                tx.clone(),
            );
        }
        return true;
    }
    let Event::Key(k) = ev else { return false };
    if k.kind != KeyEventKind::Press {
        return false;
    }
    // ヘルプ中はほぼ全部のキーでヘルプを閉じる（q/Esc は終了優先）
    if state.show_help {
        if matches!(k.code, KeyCode::Char('q') | KeyCode::Esc) {
            state.quit = true;
            return true;
        }
        state.show_help = false;
        return true;
    }
    match k.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.quit = true;
        }
        KeyCode::Char('?') => {
            state.show_help = true;
        }
        KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
            state.quit = true;
        }
        KeyCode::Char('r') => {
            state.last_error = None;
            state.radar_loading = true;
            spawn_fetch(
                provider.clone(),
                state.config.clone(),
                state.radar_time_offset,
                state.radar_aspect,
                tx.clone(),
            );
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            // 13 まで上げる。z=11-13 は JMA タイル z=10 を内部でクロップして拡大表示。
            state.config.radar.zoom = (state.config.radar.zoom + 1).min(13);
            state.radar_loading = true;
            spawn_radar(
                provider.clone(),
                state.config.clone(),
                state.radar_time_offset,
                state.radar_aspect,
                tx.clone(),
            );
        }
        KeyCode::Char('-') | KeyCode::Char('_') => {
            state.config.radar.zoom = state.config.radar.zoom.saturating_sub(1).max(6);
            state.radar_loading = true;
            spawn_radar(
                provider.clone(),
                state.config.clone(),
                state.radar_time_offset,
                state.radar_aspect,
                tx.clone(),
            );
        }
        // 移動量は 0.02 度（約 2km）。タイルキャッシュが効くので連打しても軽い。
        KeyCode::Char('h') => shift_location(state, -0.02, 0.0, provider.clone(), tx.clone()),
        KeyCode::Char('l') => shift_location(state, 0.02, 0.0, provider.clone(), tx.clone()),
        KeyCode::Char('j') => shift_location(state, 0.0, -0.02, provider.clone(), tx.clone()),
        KeyCode::Char('k') => shift_location(state, 0.0, 0.02, provider.clone(), tx.clone()),
        // 時系列スクラブ: , (<) 過去、. (>) 未来。範囲はプロバイダ依存。
        KeyCode::Char(',') | KeyCode::Char('<') => {
            let (off_min, _) = provider.radar_offset_range();
            state.radar_time_offset = (state.radar_time_offset - 1).max(off_min);
            state.radar_loading = true;
            spawn_radar(
                provider.clone(),
                state.config.clone(),
                state.radar_time_offset,
                state.radar_aspect,
                tx.clone(),
            );
        }
        KeyCode::Char('.') | KeyCode::Char('>') => {
            let (_, off_max) = provider.radar_offset_range();
            state.radar_time_offset = (state.radar_time_offset + 1).min(off_max);
            state.radar_loading = true;
            spawn_radar(
                provider.clone(),
                state.config.clone(),
                state.radar_time_offset,
                state.radar_aspect,
                tx.clone(),
            );
        }
        // アニメーション再生 toggle。tokio interval で進行は外側で。
        KeyCode::Char('p') => {
            state.radar_playing = !state.radar_playing;
        }
        // 地図スタイル切替 (GSI → CARTO → 衛星写真 → GSI ...)
        KeyCode::Char('m') | KeyCode::Char('M') => {
            state.config.radar.map_style = state.config.radar.map_style.next();
            provider.set_map_style(state.config.radar.map_style);
            state.radar_loading = true;
            spawn_radar(
                provider.clone(),
                state.config.clone(),
                state.radar_time_offset,
                state.radar_aspect,
                tx.clone(),
            );
        }
        _ => return false,
    }
    true
}

fn shift_location(
    state: &mut AppState,
    dlon: f64,
    dlat: f64,
    provider: Arc<dyn WeatherProvider>,
    tx: mpsc::UnboundedSender<Msg>,
) {
    state.config.location.longitude += dlon;
    state.config.location.latitude += dlat;
    state.radar_loading = true;
    spawn_radar(
        provider,
        state.config.clone(),
        state.radar_time_offset,
        state.radar_aspect,
        tx,
    );
}

fn spawn_fetch(
    provider: Arc<dyn WeatherProvider>,
    cfg: Config,
    time_offset: i32,
    aspect: f64,
    tx: mpsc::UnboundedSender<Msg>,
) {
    let lat = cfg.location.latitude;
    let lon = cfg.location.longitude;
    let zoom = cfg.radar.zoom;

    // 4 種類のフェッチを並列に投げる
    {
        let p = provider.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            match p.current(lat, lon).await {
                Ok(c) => {
                    let _ = tx.send(Msg::Current(c));
                }
                Err(e) => {
                    let _ = tx.send(Msg::Error(format!("current: {e:#}")));
                }
            }
        });
    }
    {
        let p = provider.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            match p.hourly(lat, lon).await {
                Ok(v) => {
                    let _ = tx.send(Msg::Hourly(v));
                }
                Err(e) => {
                    let _ = tx.send(Msg::Error(format!("hourly: {e:#}")));
                }
            }
        });
    }
    {
        let p = provider.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            match p.daily(lat, lon).await {
                Ok(v) => {
                    let _ = tx.send(Msg::Daily(v));
                }
                Err(e) => {
                    let _ = tx.send(Msg::Error(format!("daily: {e:#}")));
                }
            }
        });
    }
    {
        let p = provider;
        let tx = tx.clone();
        tokio::spawn(async move {
            match p.radar(lat, lon, zoom, time_offset, aspect).await {
                Ok(r) => {
                    let _ = tx.send(Msg::Radar(r));
                }
                Err(e) => {
                    let _ = tx.send(Msg::Error(format!("radar: {e:#}")));
                }
            }
        });
    }
}

fn spawn_radar(
    provider: Arc<dyn WeatherProvider>,
    cfg: Config,
    time_offset: i32,
    aspect: f64,
    tx: mpsc::UnboundedSender<Msg>,
) {
    let lat = cfg.location.latitude;
    let lon = cfg.location.longitude;
    let zoom = cfg.radar.zoom;
    tokio::spawn(async move {
        match provider.radar(lat, lon, zoom, time_offset, aspect).await {
            Ok(r) => {
                let _ = tx.send(Msg::Radar(r));
            }
            Err(e) => {
                let _ = tx.send(Msg::Error(format!("radar: {e:#}")));
            }
        }
    });
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
