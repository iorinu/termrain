use super::*;
use crate::api::open_meteo::OpenMeteo;
use crate::config::Config;
use crate::map::MapData;

fn test_state() -> AppState {
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
        radar_loading: false,
        radar_error: None,
        radar_request_id: 0,
        radar_aspect: 1.0,
        last_error: None,
        quit: false,
    }
}

#[test]
fn handles_basic_keys_without_starting_network_requests() {
    let provider: Arc<dyn WeatherProvider> = Arc::new(OpenMeteo::new());
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut state = test_state();

    assert!(handle_event(
        &mut state,
        Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('?'),
            KeyModifiers::NONE,
        )),
        &provider,
        &tx,
    ));
    assert!(state.show_help);

    assert!(handle_event(
        &mut state,
        Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
        )),
        &provider,
        &tx,
    ));
    assert!(!state.show_help);

    assert!(handle_event(
        &mut state,
        Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('p'),
            KeyModifiers::NONE,
        )),
        &provider,
        &tx,
    ));
    assert!(state.radar_playing);

    assert!(!handle_event(
        &mut state,
        Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
        )),
        &provider,
        &tx,
    ));
}

#[test]
fn quit_key_sets_the_quit_state() {
    let provider: Arc<dyn WeatherProvider> = Arc::new(OpenMeteo::new());
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut state = test_state();

    assert!(handle_event(
        &mut state,
        Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
        )),
        &provider,
        &tx,
    ));
    assert!(state.quit);
}

#[test]
fn clears_old_radar_and_error_before_map_style_reload() {
    let mut state = test_state();
    state.radar = Some(crate::api::RadarGrid {
        width: 1,
        height: 1,
        data: vec![vec![1.0]],
        map_dots: vec![vec![false]],
        composite_image: None,
        bounds: ((0.0, 0.0), (1.0, 1.0)),
        observed_at: chrono::Local::now(),
    });
    state.radar_error = Some("old radar error".into());
    state.last_error = Some("old radar error".into());

    clear_radar_display_for_style_change(&mut state);

    assert!(state.radar.is_none());
    assert!(state.radar_protocol.is_none());
    assert!(state.radar_error.is_none());
    assert!(state.last_error.is_none());
}

#[test]
fn keeps_an_unrelated_error_when_clearing_radar_for_map_style_reload() {
    let mut state = test_state();
    state.radar_error = Some("old radar error".into());
    state.last_error = Some("weather request failed".into());

    clear_radar_display_for_style_change(&mut state);

    assert!(state.radar_error.is_none());
    assert_eq!(state.last_error.as_deref(), Some("weather request failed"));
}

#[test]
fn clears_the_previous_radar_error_when_starting_a_radar_retry() {
    let mut state = test_state();
    state.radar_error = Some("old radar error".into());
    state.last_error = Some("old radar error".into());

    state.begin_radar_request();

    assert!(state.radar_error.is_none());
    assert!(state.last_error.is_none());
}
