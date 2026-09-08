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
