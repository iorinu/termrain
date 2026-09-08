use super::{apply_msg, should_apply_radar};
use crate::api::RadarGrid;
use crate::app::state::{AppState, Msg};
use crate::config::Config;
use crate::map::MapData;
use std::sync::Arc;

fn state_with_loading_radar(request_id: u64) -> AppState {
    AppState {
        config: Config::default(),
        provider_name: "test".into(),
        current: None,
        hourly: Vec::new(),
        daily: Vec::new(),
        radar: None,
        map: Arc::new(MapData::default()),
        image_picker: None,
        radar_protocol: None,
        radar_time_offset: 0,
        radar_playing: false,
        splash_active: false,
        show_help: false,
        spinner_frame: 0,
        radar_loading: true,
        radar_request_id: request_id,
        radar_aspect: 1.0,
        last_error: None,
        quit: false,
    }
}

fn radar_grid(value: f64) -> RadarGrid {
    RadarGrid {
        width: 1,
        height: 1,
        data: vec![vec![value]],
        map_dots: vec![vec![false]],
        composite_image: None,
        bounds: ((0.0, 0.0), (1.0, 1.0)),
        observed_at: chrono::Local::now(),
    }
}

#[test]
fn accepts_only_the_latest_radar_request() {
    assert!(should_apply_radar(7, 7));
    assert!(!should_apply_radar(6, 7));
    assert!(!should_apply_radar(8, 7));
}

#[test]
fn ignores_a_stale_radar_result_and_keeps_loading_state() {
    let mut state = state_with_loading_radar(7);

    apply_msg(
        &mut state,
        Msg::Radar {
            request_id: 6,
            grid: radar_grid(6.0),
        },
    );

    assert!(state.radar.is_none());
    assert!(state.radar_loading);
}

#[test]
fn applies_the_latest_radar_result_and_clears_loading_state() {
    let mut state = state_with_loading_radar(7);

    apply_msg(
        &mut state,
        Msg::Radar {
            request_id: 7,
            grid: radar_grid(7.0),
        },
    );

    assert_eq!(state.radar.as_ref().unwrap().data[0][0], 7.0);
    assert!(!state.radar_loading);
}
