// バックグラウンドの非同期取得 (天気・レーダー・地図) と、
// その結果を AppState に反映する apply_msg をまとめる。
//
// tokio::spawn したタスクから mpsc::UnboundedSender でメインへ Msg を返し、
// メインスレッドの apply_msg がそれを state に書き戻す、という単方向フロー。

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::api::WeatherProvider;
use crate::config::Config;
use crate::map::MapData;

use super::state::{AppState, Msg};

pub fn apply_msg(state: &mut AppState, msg: Msg) {
    match msg {
        Msg::Current(c) => state.current = Some(c),
        Msg::Hourly(h) => state.hourly = h,
        Msg::Daily(d) => state.daily = d,
        Msg::Radar {
            request_id,
            grid: r,
        } if should_apply_radar(request_id, state.radar_request_id) => {
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
        Msg::Radar { .. } => {}
        Msg::Map(m) => state.map = m,
        Msg::Error(e) => state.last_error = Some(e),
        Msg::DismissSplash => state.splash_active = false,
    }
}

/// 古い非同期取得が、新しい表示状態を上書きしないようにする。
fn should_apply_radar(request_id: u64, latest_request_id: u64) -> bool {
    request_id == latest_request_id
}

/// 地図データ（海岸線）を非同期にロードする。
/// 失敗してもアプリは続行できる（地図なしでレーダーは描ける）。
pub fn spawn_map_load(tx: mpsc::UnboundedSender<Msg>) {
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

pub fn spawn_fetch(
    provider: Arc<dyn WeatherProvider>,
    cfg: Config,
    radar_request_id: u64,
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
                    let _ = tx.send(Msg::Radar {
                        request_id: radar_request_id,
                        grid: r,
                    });
                }
                Err(e) => {
                    let _ = tx.send(Msg::Error(format!("radar: {e:#}")));
                }
            }
        });
    }
}

pub fn spawn_radar(
    provider: Arc<dyn WeatherProvider>,
    cfg: Config,
    radar_request_id: u64,
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
                let _ = tx.send(Msg::Radar {
                    request_id: radar_request_id,
                    grid: r,
                });
            }
            Err(e) => {
                let _ = tx.send(Msg::Error(format!("radar: {e:#}")));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::should_apply_radar;

    #[test]
    fn accepts_only_the_latest_radar_request() {
        assert!(should_apply_radar(7, 7));
        assert!(!should_apply_radar(6, 7));
        assert!(!should_apply_radar(8, 7));
    }
}
