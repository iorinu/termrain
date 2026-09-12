use super::*;
use crate::i18n::Language;
use std::ffi::OsString;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvRestore {
    xdg_config_home: Option<OsString>,
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        unsafe {
            match &self.xdg_config_home {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
        }
    }
}

#[test]
fn default_config_keeps_the_tokyo_and_radar_defaults() {
    let config = Config::default();

    assert_eq!(config.location.name, "Tokyo");
    assert_eq!(config.location.country, "JP");
    assert_eq!(config.radar.zoom, 11);
    assert_eq!(config.radar.map_style, MapStyle::OpenFreeMap);
    assert_eq!(config.radar.open_free_map_road_scale, 0.7);
    assert_eq!(config.radar.carto_api_key, None);
    assert_eq!(config.ui.unit, "metric");
    assert_eq!(config.ui.refresh_interval, 600);
}

#[test]
fn map_styles_cycle_and_keep_their_urls_and_cache_keys() {
    assert_eq!(MapStyle::OpenFreeMap.next(), MapStyle::OpenStreetMap);
    assert_eq!(MapStyle::OpenStreetMap.next(), MapStyle::CartoVoyager);
    assert_eq!(MapStyle::CartoVoyager.next(), MapStyle::GsiStd);
    assert_eq!(MapStyle::GsiStd.next(), MapStyle::GsiPhoto);
    assert_eq!(MapStyle::GsiPhoto.next(), MapStyle::OpenFreeMap);
    assert_eq!(
        MapStyle::GsiStd.effective_for_country("JP"),
        MapStyle::GsiStd
    );
    assert_eq!(
        MapStyle::GsiStd.effective_for_country("FR"),
        MapStyle::OpenFreeMap
    );
    assert_eq!(
        MapStyle::OpenFreeMap.next_for_country("FR"),
        MapStyle::OpenStreetMap
    );
    assert_eq!(
        MapStyle::OpenStreetMap.next_for_country("FR"),
        MapStyle::CartoVoyager
    );
    assert_eq!(
        MapStyle::CartoVoyager.next_for_country("FR"),
        MapStyle::OpenFreeMap
    );

    assert_eq!(
        MapStyle::GsiStd.tile_url(10, 1, 2),
        "https://cyberjapandata.gsi.go.jp/xyz/std/10/1/2.png"
    );
    assert_eq!(MapStyle::GsiStd.cache_key(), "gsi_std");
    assert_eq!(MapStyle::OpenStreetMap.cache_key(), "openstreetmap");
    assert_eq!(MapStyle::CartoVoyager.cache_key(), "carto_voyager");
    assert_eq!(
        MapStyle::CartoVoyager.tile_url(5, 28, 12),
        "https://basemaps.cartocdn.com/rastertiles/voyager/5/28/12.png"
    );
    assert_eq!(
        MapStyle::CartoVoyager
            .tile_url_with_carto_api_key(5, 28, 12, Some("test-key&scope=tiles"),),
        "https://basemaps.cartocdn.com/rastertiles/voyager/5/28/12.png?key=test-key%26scope%3Dtiles"
    );
    assert_eq!(
        MapStyle::CartoVoyager.tile_url_with_carto_api_key(5, 28, 12, Some("   ")),
        "https://basemaps.cartocdn.com/rastertiles/voyager/5/28/12.png"
    );
    assert_eq!(
        MapStyle::OpenStreetMap.tile_url_with_carto_api_key(5, 28, 12, Some("secret")),
        "https://tile.openstreetmap.org/5/28/12.png"
    );
    assert_eq!(MapStyle::OpenFreeMap.cache_key(), "openfreemap_liberty");
    assert_eq!(
        MapStyle::OpenFreeMap.tile_url(5, 28, 12),
        "https://tiles.openfreemap.org/planet/5/28/12.pbf"
    );
    assert_eq!(MapStyle::GsiPhoto.cache_key(), "gsi_photo");
}

#[test]
fn carto_label_keeps_both_required_attributions_in_each_language() {
    for language in [Language::English, Language::Japanese] {
        let label = MapStyle::CartoVoyager.label_for_language(language);
        assert!(label.contains("© OpenStreetMap contributors"));
        assert!(label.contains("© CARTO"));
    }
}

#[test]
fn default_and_carto_settings_are_compatible() {
    let default_style = Config::default().radar.map_style;
    assert_eq!(
        default_style.label(),
        "OpenFreeMap Liberty (© OpenMapTiles, © OpenStreetMap)"
    );
    assert_eq!(
        default_style.tile_url(5, 28, 12),
        "https://tiles.openfreemap.org/planet/5/28/12.pbf"
    );
    assert_eq!(default_style.cache_key(), "openfreemap_liberty");

    let legacy: Config = toml::from_str(
        r#"
            [radar]
            zoom = 11
            map_style = "carto_voyager"
        "#,
    )
    .unwrap();
    assert_eq!(legacy.radar.map_style, MapStyle::CartoVoyager);
    assert_eq!(legacy.radar.open_free_map_road_scale, 0.7);
    assert!(
        toml::to_string(&legacy)
            .unwrap()
            .contains("map_style = \"carto_voyager\"")
    );
}

#[test]
fn config_round_trips_through_toml_without_changing_values() {
    let original = Config {
        location: Location {
            name: "Paris".into(),
            latitude: 48.8566,
            longitude: 2.3522,
            country: "FR".into(),
        },
        ui: UiConfig {
            unit: "imperial".into(),
            refresh_interval: 120,
            language: crate::i18n::Language::Japanese,
        },
        radar: RadarConfig {
            zoom: 8,
            map_style: MapStyle::GsiPhoto,
            open_free_map_road_scale: 0.6,
            carto_api_key: Some("test-key".into()),
        },
    };

    let text = toml::to_string_pretty(&original).unwrap();
    let restored: Config = toml::from_str(&text).unwrap();

    assert_eq!(restored.location.name, original.location.name);
    assert_eq!(restored.location.latitude, original.location.latitude);
    assert_eq!(restored.location.longitude, original.location.longitude);
    assert_eq!(restored.location.country, original.location.country);
    assert_eq!(restored.ui.unit, original.ui.unit);
    assert_eq!(restored.ui.refresh_interval, original.ui.refresh_interval);
    assert_eq!(restored.ui.language, original.ui.language);
    assert_eq!(restored.radar.zoom, original.radar.zoom);
    assert_eq!(restored.radar.map_style, original.radar.map_style);
    assert_eq!(
        restored.radar.open_free_map_road_scale,
        original.radar.open_free_map_road_scale
    );
    assert_eq!(restored.radar.carto_api_key, original.radar.carto_api_key);
}

#[test]
fn saves_and_loads_config_from_the_xdg_file_path() {
    let _lock = ENV_LOCK.lock().unwrap();
    let previous = EnvRestore {
        xdg_config_home: std::env::var_os("XDG_CONFIG_HOME"),
    };
    let root = std::env::temp_dir().join(format!(
        "termrain-config-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &root) };

    let original = Config::default();
    original.save().unwrap();
    let loaded = Config::load_or_default().unwrap();

    assert_eq!(loaded.location.name, original.location.name);
    assert_eq!(loaded.location.latitude, original.location.latitude);
    assert_eq!(loaded.location.longitude, original.location.longitude);
    assert_eq!(loaded.radar.map_style, original.radar.map_style);
    assert!(Config::path().unwrap().is_file());

    drop(previous);
    std::fs::remove_dir_all(root).unwrap();
}
