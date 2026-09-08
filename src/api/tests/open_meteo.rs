use super::*;
use crate::api::{WeatherIcon, WeatherProvider};
use crate::config::MapStyle;
use crate::i18n::Language;

#[test]
fn maps_wmo_code_ranges_to_icons() {
    assert_eq!(wmo_to_icon(0), WeatherIcon::Sunny);
    assert_eq!(wmo_to_icon(2), WeatherIcon::PartlyCloudy);
    assert_eq!(wmo_to_icon(45), WeatherIcon::Cloudy);
    assert_eq!(wmo_to_icon(61), WeatherIcon::Rain);
    assert_eq!(wmo_to_icon(71), WeatherIcon::Snow);
    assert_eq!(wmo_to_icon(95), WeatherIcon::Thunder);
    assert_eq!(wmo_to_icon(100), WeatherIcon::Unknown);
}

#[test]
fn maps_wmo_codes_to_both_supported_languages() {
    assert_eq!(wmo_to_text(0, Language::Japanese), "快晴");
    assert_eq!(wmo_to_text(61, Language::Japanese), "雨");
    assert_eq!(wmo_to_text(95, Language::English), "Thunderstorm");
    assert_eq!(wmo_to_text(100, Language::English), "Unknown");
}

#[test]
fn deserializes_the_forecast_response_shape_used_by_the_provider() {
    let response: ForecastResponse = serde_json::from_str(
        r#"{
            "utc_offset_seconds": 7200,
            "current": {
                "time": "2026-06-08T18:00",
                "temperature_2m": 21.5,
                "relative_humidity_2m": 72,
                "weather_code": 61,
                "wind_speed_10m": 3.2,
                "wind_direction_10m": 180
            },
            "hourly": {
                "time": ["2026-06-08T18:00"],
                "temperature_2m": [21.5],
                "precipitation": [0.4],
                "precipitation_probability": [80],
                "weather_code": [61]
            },
            "daily": {
                "time": ["2026-06-08"],
                "weather_code": [61],
                "temperature_2m_max": [24.0],
                "temperature_2m_min": [18.0],
                "precipitation_probability_max": [90]
            }
        }"#,
    )
    .unwrap();

    assert_eq!(response.utc_offset_seconds, Some(7200));
    assert_eq!(response.current.unwrap().weather_code, 61);
    assert_eq!(response.hourly.unwrap().precipitation, vec![0.4]);
    assert_eq!(response.daily.unwrap().temperature_2m_max, vec![Some(24.0)]);
}

fn forecast_fixture() -> ForecastResponse {
    serde_json::from_str(
        r#"{
            "utc_offset_seconds": 7200,
            "current": {
                "time": "2026-06-08T18:00",
                "temperature_2m": 21.5,
                "relative_humidity_2m": 72,
                "weather_code": 61,
                "wind_speed_10m": 3.2,
                "wind_direction_10m": 180
            },
            "hourly": {
                "time": ["2026-06-08T18:00"],
                "temperature_2m": [21.5],
                "precipitation": [0.4],
                "precipitation_probability": [80],
                "weather_code": [61]
            },
            "daily": {
                "time": ["2026-06-08"],
                "weather_code": [61],
                "temperature_2m_max": [24.0],
                "temperature_2m_min": [18.0],
                "precipitation_probability_max": [90]
            }
        }"#,
    )
    .unwrap()
}

#[test]
fn converts_forecast_fixture_into_ui_weather_models() {
    let current = convert_current_weather(forecast_fixture(), Language::English).unwrap();
    let hourly = convert_hourly(forecast_fixture()).unwrap();
    let daily = convert_daily(forecast_fixture(), Language::Japanese).unwrap();

    assert_eq!(current.condition, "Rain");
    assert_eq!(current.icon, WeatherIcon::Rain);
    assert_eq!(current.temperature_c, 21.5);
    assert_eq!(current.humidity_pct, Some(72.0));
    assert_eq!(current.wind_speed_ms, Some(3.2));
    assert_eq!(current.wind_direction_deg, Some(180.0));
    assert_eq!(current.observed_at.timestamp(), 1780934400);

    assert_eq!(hourly.len(), 1);
    assert_eq!(hourly[0].temperature_c, 21.5);
    assert_eq!(hourly[0].precipitation_mm, 0.4);
    assert_eq!(hourly[0].precipitation_prob_pct, Some(80.0));
    assert_eq!(hourly[0].icon, WeatherIcon::Rain);

    assert_eq!(daily.len(), 1);
    assert_eq!(daily[0].condition, "雨");
    assert_eq!(daily[0].icon, WeatherIcon::Rain);
    assert_eq!(daily[0].temp_max_c, Some(24.0));
    assert_eq!(daily[0].temp_min_c, Some(18.0));
    assert_eq!(daily[0].precipitation_prob_pct, Some(90.0));
}

#[test]
fn converts_local_api_time_using_the_provider_offset() {
    let actual = parse_local_with_offset("2026-06-08T18:00", Some(7200)).unwrap();
    let expected = chrono::DateTime::parse_from_rfc3339("2026-06-08T16:00:00+00:00")
        .unwrap()
        .timestamp();

    assert_eq!(actual.timestamp(), expected);
    assert!(parse_local_with_offset("invalid", Some(7200)).is_err());
}

#[test]
fn falls_back_to_openfreemap_for_non_japanese_gsi_styles() {
    let provider = OpenMeteo::new();
    provider.set_map_style(MapStyle::GsiStd);
    assert_eq!(*provider.map_style.lock().unwrap(), MapStyle::OpenFreeMap);

    provider.set_map_style(MapStyle::GsiPhoto);
    assert_eq!(*provider.map_style.lock().unwrap(), MapStyle::OpenFreeMap);

    provider.set_map_style(MapStyle::OpenStreetMap);
    assert_eq!(*provider.map_style.lock().unwrap(), MapStyle::OpenStreetMap);

    provider.set_map_style(MapStyle::CartoVoyager);
    assert_eq!(*provider.map_style.lock().unwrap(), MapStyle::CartoVoyager);

    provider.set_map_style(MapStyle::OpenFreeMap);
    assert_eq!(*provider.map_style.lock().unwrap(), MapStyle::OpenFreeMap);
}

#[test]
fn exposes_the_rainviewer_supported_offset_range() {
    assert_eq!(OpenMeteo::new().radar_offset_range(), (-12, 0));
}

#[test]
fn composite_image_has_expected_size_and_user_marker() {
    let image = build_composite_image_rv(
        1.0,
        1,
        1,
        1,
        1,
        1,
        1,
        -45.0,
        45.0,
        -45.0,
        45.0,
        0.0,
        0.0,
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
    )
    .unwrap()
    .to_rgba8();

    assert_eq!(image.dimensions(), (1024, 1024));
    assert_eq!(image.get_pixel(512, 512), &image::Rgba([255, 220, 0, 255]));
}

#[test]
fn composites_a_semitransparent_radar_tile_over_a_map_tile() {
    let mut map_images = std::collections::HashMap::new();
    map_images.insert(
        (0, 0),
        Arc::new(image::RgbaImage::from_pixel(
            256,
            256,
            image::Rgba([255, 255, 255, 255]),
        )),
    );
    let mut radar_images = std::collections::HashMap::new();
    radar_images.insert(
        (0, 0),
        Arc::new(image::RgbaImage::from_pixel(
            256,
            256,
            image::Rgba([255, 0, 0, 255]),
        )),
    );

    let image = build_composite_image_rv(
        1.0,
        0,
        0,
        0,
        0,
        0,
        0,
        -90.0,
        90.0,
        -45.0,
        45.0,
        0.0,
        0.0,
        &map_images,
        &radar_images,
    )
    .unwrap()
    .to_rgba8();

    assert_eq!(
        image.get_pixel(100, 100),
        &image::Rgba([255, 114, 114, 255])
    );
}
