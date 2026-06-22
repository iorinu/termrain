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
