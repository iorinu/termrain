use super::*;
use crate::config::MapStyle;
use crate::i18n::Language;

#[test]
fn selects_provider_from_country_and_force_flag() {
    assert_eq!(select_provider("JP", false).name(), "気象庁 (JMA)");
    assert_eq!(select_provider("jp", false).name(), "気象庁 (JMA)");
    assert_eq!(
        select_provider("US", false).name(),
        "Open-Meteo + RainViewer"
    );
    assert_eq!(select_provider("US", true).name(), "気象庁 (JMA)");
}

#[test]
fn keeps_provider_specific_radar_offset_ranges() {
    assert_eq!(select_provider("JP", false).radar_offset_range(), (-6, 12));
    assert_eq!(select_provider("US", false).radar_offset_range(), (-12, 0));
}

#[test]
fn builds_authenticated_carto_urls_without_leaking_keys_to_other_styles() {
    let url = build_map_tile_url(
        MapStyle::CartoVoyager,
        5,
        28,
        12,
        Some("test-key"),
        Language::English,
    )
    .unwrap();
    assert_eq!(
        url,
        "https://basemaps.cartocdn.com/rastertiles/voyager/5/28/12.png?key=test-key"
    );

    let url = build_map_tile_url(
        MapStyle::OpenStreetMap,
        5,
        28,
        12,
        Some("test-key"),
        Language::English,
    )
    .unwrap();
    assert!(!url.contains("test-key"));
}

#[test]
fn rejects_carto_tiles_without_a_non_blank_api_key() {
    let error = build_map_tile_url(
        MapStyle::CartoVoyager,
        5,
        28,
        12,
        Some("   "),
        Language::English,
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "CARTO Voyager requires an API key. Check [radar].carto_api_key in the configuration file."
    );
    assert!(!error.to_string().contains("test-key"));
}

#[test]
fn redacts_carto_tile_errors_before_they_reach_logs() {
    let error = sanitize_carto_tile_error(
        MapStyle::CartoVoyager,
        anyhow::anyhow!(
            "error reading response body for url (https://basemaps.cartocdn.com/?key=secret-key)"
        ),
    );

    assert_eq!(error.to_string(), "CARTO Voyager tile request failed");
    assert!(!error.to_string().contains("secret-key"));
}
