// キーボード / リサイズイベントを受けて AppState を更新するハンドラ。
// 戻り値 true は「再描画が必要」を意味する。

use std::sync::Arc;

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc;

use crate::api::WeatherProvider;

use super::fetch::{spawn_fetch, spawn_radar};
use super::state::{AppState, Msg};

pub fn handle_event(
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

#[cfg(test)]
mod tests {
    // 親モジュール (input.rs) の関数とインポートを取り込む
    use super::*;

    use crate::api::{CurrentWeather, DailyPoint, HourlyPoint, RadarGrid};
    use crate::config::{Config, MapStyle};
    use crate::map::MapData;
    use anyhow::Result;
    use async_trait::async_trait;
    use crossterm::event::{KeyEvent, KeyEventKind, KeyModifiers};
    use std::sync::Mutex;

    // --- テスト用のモック WeatherProvider ---
    //
    // handle_event は spawn_fetch / spawn_radar 経由で provider.current / radar
    // などを呼ぶが、結果は spawn 内で「送って終わり」なのでテスト本体には
    // 影響しない。テストでは sync メソッド (set_map_style, radar_offset_range)
    // の挙動だけ検証すれば足りる。
    #[derive(Default)]
    struct MockProvider {
        // (-N, +N) の再生範囲。テストごとに上書き可
        offset_min: i32,
        offset_max: i32,
        // set_map_style が最後に渡された値を覚えておく
        last_map_style: Mutex<Option<MapStyle>>,
    }

    #[async_trait]
    impl WeatherProvider for MockProvider {
        fn name(&self) -> &'static str {
            "mock"
        }
        // 以下 async メソッドは spawn 内で呼ばれるだけで返り値は使われない。
        // Err でも Ok でも本体テストには影響しない。
        async fn current(&self, _lat: f64, _lon: f64) -> Result<CurrentWeather> {
            anyhow::bail!("mock: not used in tests")
        }
        async fn hourly(&self, _lat: f64, _lon: f64) -> Result<Vec<HourlyPoint>> {
            Ok(vec![])
        }
        async fn daily(&self, _lat: f64, _lon: f64) -> Result<Vec<DailyPoint>> {
            Ok(vec![])
        }
        async fn radar(
            &self,
            _lat: f64,
            _lon: f64,
            _zoom: u8,
            _time_offset: i32,
            _aspect: f64,
        ) -> Result<RadarGrid> {
            anyhow::bail!("mock: not used in tests")
        }
        fn set_map_style(&self, style: MapStyle) {
            *self.last_map_style.lock().unwrap() = Some(style);
        }
        fn radar_offset_range(&self) -> (i32, i32) {
            (self.offset_min, self.offset_max)
        }
    }

    // テスト用の AppState を最小構成で作るヘルパー。
    // `image_picker` は None 固定（テスト環境では端末クエリができないため）。
    fn make_state() -> AppState {
        AppState {
            config: Config::default(),
            provider_name: "mock".into(),
            current: None,
            hourly: Vec::new(),
            daily: Vec::new(),
            radar: None,
            map: Arc::new(MapData::default()),
            image_picker: None,
            radar_protocol: None,
            radar_time_offset: 0,
            radar_playing: false,
            splash_active: true,
            show_help: false,
            spinner_frame: 0,
            radar_loading: false,
            radar_aspect: 1.0,
            last_error: None,
            quit: false,
        }
    }

    // テスト用の (provider, tx, _rx) セット。
    // rx は受け側を生かしておかないと送信側 (spawn 内の tx.send) が
    // 即エラーになるが、エラーでもテスト本体は問題ないので戻り値で保持するだけ。
    fn make_provider_with_range(
        min: i32,
        max: i32,
    ) -> (
        Arc<dyn WeatherProvider>,
        mpsc::UnboundedSender<Msg>,
        mpsc::UnboundedReceiver<Msg>,
    ) {
        let provider: Arc<dyn WeatherProvider> = Arc::new(MockProvider {
            offset_min: min,
            offset_max: max,
            ..Default::default()
        });
        let (tx, rx) = mpsc::unbounded_channel::<Msg>();
        (provider, tx, rx)
    }

    fn make_provider() -> (
        Arc<dyn WeatherProvider>,
        mpsc::UnboundedSender<Msg>,
        mpsc::UnboundedReceiver<Msg>,
    ) {
        make_provider_with_range(-6, 12)
    }

    /// 簡易な KeyEvent (kind=Press) を作る
    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    /// Ctrl 付き KeyEvent
    fn key_ctrl(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::CONTROL))
    }

    // ====================================================================
    // spawn を伴わないテスト群 (#[test] で OK)
    // ====================================================================

    #[test]
    fn quit_on_q() {
        let mut s = make_state();
        let (p, tx, _rx) = make_provider();
        let redraw = handle_event(&mut s, key(KeyCode::Char('q')), &p, &tx);
        assert!(redraw);
        assert!(s.quit);
    }

    #[test]
    fn quit_on_esc() {
        let mut s = make_state();
        let (p, tx, _rx) = make_provider();
        handle_event(&mut s, key(KeyCode::Esc), &p, &tx);
        assert!(s.quit);
    }

    #[test]
    fn quit_on_ctrl_c() {
        let mut s = make_state();
        let (p, tx, _rx) = make_provider();
        handle_event(&mut s, key_ctrl(KeyCode::Char('c')), &p, &tx);
        assert!(s.quit);
    }

    #[test]
    fn show_help_on_question_mark() {
        let mut s = make_state();
        let (p, tx, _rx) = make_provider();
        assert!(handle_event(&mut s, key(KeyCode::Char('?')), &p, &tx));
        assert!(s.show_help);
        assert!(!s.quit);
    }

    #[test]
    fn toggle_playing_on_p() {
        let mut s = make_state();
        let (p, tx, _rx) = make_provider();
        assert!(!s.radar_playing);
        handle_event(&mut s, key(KeyCode::Char('p')), &p, &tx);
        assert!(s.radar_playing);
        handle_event(&mut s, key(KeyCode::Char('p')), &p, &tx);
        assert!(!s.radar_playing);
    }

    #[test]
    fn help_open_q_quits() {
        // ヘルプ表示中でも q は終了優先
        let mut s = make_state();
        s.show_help = true;
        let (p, tx, _rx) = make_provider();
        handle_event(&mut s, key(KeyCode::Char('q')), &p, &tx);
        assert!(s.quit);
        // show_help は閉じない（quit 優先で即終了するため）
        assert!(s.show_help);
    }

    #[test]
    fn help_open_other_key_closes_help() {
        // q / Esc 以外のキーはヘルプを閉じるだけ
        let mut s = make_state();
        s.show_help = true;
        let (p, tx, _rx) = make_provider();
        let redraw = handle_event(&mut s, key(KeyCode::Char('h')), &p, &tx);
        assert!(redraw);
        assert!(!s.show_help);
        assert!(!s.quit);
        // ヘルプを閉じただけで、配下の 'h' (西へ移動) は走らない
        assert_eq!(
            s.config.location.longitude,
            Config::default().location.longitude
        );
    }

    #[test]
    fn release_event_is_ignored() {
        // キーリリースは無視（false を返し、状態も変えない）
        let mut s = make_state();
        let (p, tx, _rx) = make_provider();
        let ev = Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ));
        let redraw = handle_event(&mut s, ev, &p, &tx);
        assert!(!redraw);
        assert!(!s.quit);
    }

    #[test]
    fn unmapped_key_returns_false() {
        let mut s = make_state();
        let (p, tx, _rx) = make_provider();
        let redraw = handle_event(&mut s, key(KeyCode::Char('x')), &p, &tx);
        assert!(!redraw);
        assert!(!s.quit);
    }

    // ====================================================================
    // spawn を伴うテスト群 (#[tokio::test] が必要)
    //
    // handle_event は内部で tokio::spawn を呼ぶブランチがあるため、
    // ランタイムが必要。state の最終値だけを検査し、spawn された
    // タスクの完了は待たない（mock provider は async メソッドで失敗を
    // 返すだけで副作用なし）。
    // ====================================================================

    #[tokio::test]
    async fn zoom_in_clamps_at_13() {
        let mut s = make_state();
        s.config.radar.zoom = 13;
        let (p, tx, _rx) = make_provider();
        handle_event(&mut s, key(KeyCode::Char('+')), &p, &tx);
        assert_eq!(s.config.radar.zoom, 13); // .min(13) で頭打ち
        assert!(s.radar_loading);
    }

    #[tokio::test]
    async fn zoom_out_clamps_at_6() {
        let mut s = make_state();
        s.config.radar.zoom = 6;
        let (p, tx, _rx) = make_provider();
        handle_event(&mut s, key(KeyCode::Char('-')), &p, &tx);
        assert_eq!(s.config.radar.zoom, 6); // saturating_sub + .max(6) で下限
        assert!(s.radar_loading);
    }

    #[tokio::test]
    async fn zoom_in_increments_inside_range() {
        let mut s = make_state();
        s.config.radar.zoom = 8;
        let (p, tx, _rx) = make_provider();
        handle_event(&mut s, key(KeyCode::Char('=')), &p, &tx); // + のエイリアス
        assert_eq!(s.config.radar.zoom, 9);
    }

    #[tokio::test]
    async fn time_offset_clamps_at_max() {
        // provider の (min, max) = (-3, 3) で max ちょうどから '.' を押しても進まない
        let mut s = make_state();
        s.radar_time_offset = 3;
        let (p, tx, _rx) = make_provider_with_range(-3, 3);
        handle_event(&mut s, key(KeyCode::Char('.')), &p, &tx);
        assert_eq!(s.radar_time_offset, 3);
        assert!(s.radar_loading);
    }

    #[tokio::test]
    async fn time_offset_clamps_at_min() {
        let mut s = make_state();
        s.radar_time_offset = -3;
        let (p, tx, _rx) = make_provider_with_range(-3, 3);
        handle_event(&mut s, key(KeyCode::Char(',')), &p, &tx);
        assert_eq!(s.radar_time_offset, -3);
    }

    #[tokio::test]
    async fn time_offset_advances_inside_range() {
        let mut s = make_state();
        s.radar_time_offset = 0;
        let (p, tx, _rx) = make_provider_with_range(-3, 3);
        handle_event(&mut s, key(KeyCode::Char('.')), &p, &tx);
        assert_eq!(s.radar_time_offset, 1);
        handle_event(&mut s, key(KeyCode::Char(',')), &p, &tx);
        assert_eq!(s.radar_time_offset, 0);
    }

    #[tokio::test]
    async fn shift_h_decreases_longitude() {
        // 'h' は西へ -0.02 度
        let mut s = make_state();
        let lon0 = s.config.location.longitude;
        let lat0 = s.config.location.latitude;
        let (p, tx, _rx) = make_provider();
        handle_event(&mut s, key(KeyCode::Char('h')), &p, &tx);
        assert!((s.config.location.longitude - (lon0 - 0.02)).abs() < 1e-9);
        // 緯度は変わらない
        assert_eq!(s.config.location.latitude, lat0);
        assert!(s.radar_loading);
    }

    #[tokio::test]
    async fn shift_k_increases_latitude() {
        // 'k' は北へ +0.02 度
        let mut s = make_state();
        let lat0 = s.config.location.latitude;
        let lon0 = s.config.location.longitude;
        let (p, tx, _rx) = make_provider();
        handle_event(&mut s, key(KeyCode::Char('k')), &p, &tx);
        assert!((s.config.location.latitude - (lat0 + 0.02)).abs() < 1e-9);
        assert_eq!(s.config.location.longitude, lon0);
    }

    #[tokio::test]
    async fn map_style_cycles_and_notifies_provider() {
        // GsiStd → CartoVoyager → GsiPhoto → GsiStd と巡回する
        let mut s = make_state();
        s.config.radar.map_style = MapStyle::GsiStd;
        let provider = Arc::new(MockProvider {
            offset_min: -6,
            offset_max: 12,
            ..Default::default()
        });
        let p: Arc<dyn WeatherProvider> = provider.clone();
        let (tx, _rx) = mpsc::unbounded_channel::<Msg>();

        handle_event(&mut s, key(KeyCode::Char('m')), &p, &tx);
        assert_eq!(s.config.radar.map_style, MapStyle::CartoVoyager);
        assert_eq!(
            *provider.last_map_style.lock().unwrap(),
            Some(MapStyle::CartoVoyager)
        );

        handle_event(&mut s, key(KeyCode::Char('m')), &p, &tx);
        assert_eq!(s.config.radar.map_style, MapStyle::GsiPhoto);

        handle_event(&mut s, key(KeyCode::Char('m')), &p, &tx);
        assert_eq!(s.config.radar.map_style, MapStyle::GsiStd);
    }

    #[tokio::test]
    async fn refresh_r_clears_last_error_and_marks_loading() {
        let mut s = make_state();
        s.last_error = Some("network down".into());
        let (p, tx, _rx) = make_provider();
        handle_event(&mut s, key(KeyCode::Char('r')), &p, &tx);
        assert!(s.last_error.is_none());
        assert!(s.radar_loading);
    }

    #[tokio::test]
    async fn resize_skipped_when_radar_loading() {
        // ガード: 既に loading 中ならリサイズ再フェッチを抑制
        let mut s = make_state();
        s.radar_aspect = 99.0; // 現実離れした値にして delta を確保
        s.radar_loading = true;
        let (p, tx, _rx) = make_provider();
        handle_event(&mut s, Event::Resize(200, 50), &p, &tx);
        // radar_aspect は更新されない
        assert_eq!(s.radar_aspect, 99.0);
    }

    #[tokio::test]
    async fn resize_updates_aspect_when_delta_is_large() {
        // 通常時、サイズが大きく変わったら radar_aspect を取り直す
        let mut s = make_state();
        s.radar_aspect = 99.0; // 計算結果と必ず 0.15 以上ずれる値
        s.radar_loading = false;
        let (p, tx, _rx) = make_provider();
        handle_event(&mut s, Event::Resize(200, 50), &p, &tx);
        assert!(s.radar_aspect < 99.0);
        assert!(s.radar_loading);
    }
}
