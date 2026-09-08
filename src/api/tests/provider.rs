use super::*;

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
