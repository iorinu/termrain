use super::{
    Jma, binarize_map_tile, blend, build_composite_image, downsample_tile, draw_cross,
    draw_legend_bar, jma_weather_code_text, lonlat_to_tile, map_dot_tile_zoom,
    nowcast_color_to_mmh, parse_jma_compact, rain_to_yahoo, sample_bilinear, text_to_icon,
    tile_to_lonlat,
};
use crate::api::{WeatherIcon, WeatherProvider};
use crate::config::MapStyle;

#[test]
fn map_dot_tiles_use_the_map_coordinate_zoom() {
    assert_eq!(map_dot_tile_zoom(13, 10), 13);
    assert_eq!(map_dot_tile_zoom(8, 8), 8);
}

#[test]
fn selects_the_nearest_forecast_area() {
    let area = Jma::nearest_area(35.6812, 139.7671);

    assert_eq!(area.office, "130000");
    assert_eq!(area.name, "東京");
}

#[test]
fn stores_the_optional_carto_api_key_on_the_provider() {
    let provider = Jma::new();
    assert!(provider.carto_api_key.lock().unwrap().is_none());

    provider.set_carto_api_key(Some("test-key".into()));

    assert_eq!(
        provider.carto_api_key.lock().unwrap().as_deref(),
        Some("test-key")
    );
}

#[tokio::test]
async fn refuses_carto_radar_without_an_api_key_before_network_access() {
    let provider = Jma::new();
    provider.set_map_style(MapStyle::CartoVoyager);

    let error = provider
        .radar(35.6812, 139.7671, 11, 0, 1.0)
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "CARTO Voyager requires an API key. Check [radar].carto_api_key in the configuration file."
    );
}

#[test]
fn maps_jma_weather_text_to_icons_by_priority() {
    assert_eq!(text_to_icon("晴れ"), WeatherIcon::Sunny);
    assert_eq!(text_to_icon("晴れ時々曇り"), WeatherIcon::PartlyCloudy);
    assert_eq!(text_to_icon("曇り"), WeatherIcon::Cloudy);
    assert_eq!(text_to_icon("雨のち晴れ"), WeatherIcon::Rain);
    assert_eq!(text_to_icon("雪"), WeatherIcon::Snow);
    assert_eq!(text_to_icon("雷雨"), WeatherIcon::Thunder);
    assert_eq!(text_to_icon("観測不能"), WeatherIcon::Unknown);
}

#[test]
fn maps_known_jma_weather_codes_and_unknown_codes() {
    assert_eq!(jma_weather_code_text("100"), "晴れ");
    assert_eq!(jma_weather_code_text("203"), "曇り時々雨");
    assert_eq!(jma_weather_code_text("400"), "雪");
    assert_eq!(jma_weather_code_text("999"), "不明");
}

#[test]
fn converts_tile_coordinates_at_world_boundaries() {
    assert_eq!(lonlat_to_tile(0.0, 0.0, 0), (0, 0, 0));
    assert_eq!(lonlat_to_tile(-180.0, 0.0, 2), (2, 0, 2));
    assert_eq!(lonlat_to_tile(179.999, 0.0, 2), (2, 3, 2));

    let (lat, lon) = tile_to_lonlat(2, 2, 2);
    assert!((lat - 0.0).abs() < 1e-10);
    assert!((lon - 0.0).abs() < 1e-10);
}

#[test]
fn parses_jma_compact_time_as_utc() {
    let actual = parse_jma_compact("20260608070000").unwrap();
    let expected = chrono::DateTime::parse_from_rfc3339("2026-06-08T07:00:00+00:00")
        .unwrap()
        .timestamp();

    assert_eq!(actual.timestamp(), expected);
}

#[test]
fn converts_jma_rain_palette_and_ignores_transparency() {
    assert_eq!(nowcast_color_to_mmh(33, 140, 254, 255), 5.0);
    assert_eq!(nowcast_color_to_mmh(255, 40, 0, 255), 50.0);
    assert_eq!(nowcast_color_to_mmh(0, 0, 0, 0), 0.0);
    assert_eq!(nowcast_color_to_mmh(0, 0, 0, 255), 0.0);
}

#[test]
fn blends_channels_at_alpha_boundaries() {
    assert_eq!(blend(0, 255, 0.0), 0);
    assert_eq!(blend(0, 255, 0.5), 127);
    assert_eq!(blend(10, 240, 1.0), 240);
}

#[test]
fn maps_rain_intensity_boundaries_to_the_expected_palette() {
    assert_eq!(rain_to_yahoo(0.09), None);
    assert_eq!(rain_to_yahoo(0.1), Some((200, 230, 255, 170)));
    assert_eq!(rain_to_yahoo(1.0), Some((160, 210, 250, 190)));
    assert_eq!(rain_to_yahoo(80.0), Some((120, 30, 130, 250)));
}

#[test]
fn bilinear_sampling_interpolates_the_four_neighbouring_pixels() {
    let mut img = image::RgbaImage::new(2, 2);
    img.put_pixel(0, 0, image::Rgba([0, 0, 0, 0]));
    img.put_pixel(1, 0, image::Rgba([100, 100, 100, 100]));
    img.put_pixel(0, 1, image::Rgba([200, 200, 200, 200]));
    img.put_pixel(1, 1, image::Rgba([255, 255, 255, 255]));

    assert_eq!(sample_bilinear(&img, 0.5, 0.5), image::Rgba([138; 4]));
}

#[test]
fn draws_a_clipped_center_cross() {
    let mut img = image::RgbaImage::from_pixel(3, 3, image::Rgba([0, 0, 0, 0]));
    draw_cross(&mut img, 1, 1, 1, image::Rgba([255, 220, 0, 255]));

    let colored = img.pixels().filter(|p| p.0 == [255, 220, 0, 255]).count();
    assert_eq!(colored, 5);
    assert_eq!(img.get_pixel(1, 1), &image::Rgba([255, 220, 0, 255]));
}

#[test]
fn downsample_and_binarize_preserve_a_signal_in_the_first_cell() {
    let mut rain = image::RgbaImage::from_pixel(256, 256, image::Rgba([0, 0, 0, 0]));
    rain.put_pixel(0, 0, image::Rgba([33, 140, 254, 255]));
    let grid = downsample_tile(&rain);
    assert_eq!(grid.len(), 128);
    assert_eq!(grid[0].len(), 128);
    assert_eq!(grid[0][0], 5.0);

    let mut map = image::RgbaImage::from_pixel(256, 256, image::Rgba([255, 255, 255, 255]));
    map.put_pixel(0, 0, image::Rgba([0, 0, 0, 255]));
    let dots = binarize_map_tile(&map);
    assert!(dots[0][0]);
    assert!(!dots[0][1]);
}

#[test]
fn composite_image_has_expected_size_and_user_marker() {
    let image = build_composite_image(
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
fn legend_bar_writes_the_first_and_last_color_stops() {
    let mut img = image::RgbaImage::from_pixel(100, 50, image::Rgba([0, 0, 0, 0]));
    draw_legend_bar(&mut img);

    assert_eq!(img.get_pixel(2, 38), &image::Rgba([200, 230, 255, 230]));
    assert_eq!(img.get_pixel(81, 38), &image::Rgba([120, 30, 130, 230]));
}
