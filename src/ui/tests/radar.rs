use super::*;
use crate::api::RadarGrid;
use crate::app::AppState;
use crate::config::Config;
use crate::map::MapData;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui_image::picker::Picker;
use std::sync::Arc;

fn fallback_state() -> AppState {
    AppState {
        config: Config::default(),
        provider_name: "test".into(),
        current: None,
        hourly: Vec::new(),
        daily: Vec::new(),
        radar: Some(RadarGrid {
            width: 2,
            height: 2,
            data: vec![vec![0.0, 1.0], vec![2.0, 0.0]],
            map_dots: vec![vec![false, false], vec![false, false]],
            composite_image: None,
            bounds: ((-1.0, -1.0), (1.0, 1.0)),
            observed_at: chrono::Local::now(),
        }),
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
fn renders_text_fallback_without_an_image_protocol() {
    let backend = TestBackend::new(40, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = fallback_state();

    terminal
        .draw(|frame| draw(frame, frame.area(), &mut state))
        .unwrap();

    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Radar"));
}

#[test]
fn shows_an_error_status_instead_of_loading_after_radar_failure() {
    let backend = TestBackend::new(40, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = fallback_state();
    state.radar = None;
    state.radar_error = Some("radar request failed".into());

    terminal
        .draw(|frame| draw(frame, frame.area(), &mut state))
        .unwrap();

    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("Error"));
    assert!(!text.contains("Loading"));
}

#[test]
fn renders_carto_attribution_in_the_text_fallback() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = fallback_state();
    state.config.radar.map_style = crate::config::MapStyle::CartoVoyager;

    terminal
        .draw(|frame| draw(frame, frame.area(), &mut state))
        .unwrap();

    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("OpenStreetMap contributors"));
    assert!(text.contains("CARTO"));
}

#[test]
fn keeps_carto_attribution_visible_in_a_narrow_text_fallback() {
    let backend = TestBackend::new(40, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = fallback_state();
    state.config.radar.map_style = crate::config::MapStyle::CartoVoyager;

    terminal
        .draw(|frame| draw(frame, frame.area(), &mut state))
        .unwrap();

    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("OpenStreetMap contributors"));
    assert!(text.contains("CARTO"));
}

#[test]
fn keeps_carto_attribution_visible_in_a_narrow_image_panel() {
    let backend = TestBackend::new(40, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = fallback_state();
    state.config.radar.map_style = crate::config::MapStyle::CartoVoyager;
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        4,
        4,
        image::Rgba([0, 0, 0, 255]),
    ));
    let picker = Picker::halfblocks();
    state.radar_protocol = Some(picker.new_resize_protocol(image.clone()));
    state.image_picker = Some(picker);
    state.radar.as_mut().unwrap().composite_image = Some(image);

    terminal
        .draw(|frame| draw(frame, frame.area(), &mut state))
        .unwrap();

    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("OpenStreetMap contributors"));
    assert!(text.contains("CARTO"));
}

#[test]
fn keeps_a_compact_carto_attribution_in_a_short_panel() {
    let backend = TestBackend::new(80, 5);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = fallback_state();
    state.config.radar.map_style = crate::config::MapStyle::CartoVoyager;

    terminal
        .draw(|frame| draw(frame, frame.area(), &mut state))
        .unwrap();

    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(text.contains("OSM contributors"));
    assert!(text.contains("CARTO"));
}
