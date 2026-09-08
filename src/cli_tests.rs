use super::*;

#[test]
fn parses_dump_location_language_and_save_options() {
    let args = Args::try_parse_from([
        "termrain", "--dump", "--lat", "35.6812", "--lon", "139.7671", "--lang", "ja", "--save",
    ])
    .unwrap();

    assert!(args.dump);
    assert_eq!(args.lat, Some(35.6812));
    assert_eq!(args.lon, Some(139.7671));
    assert_eq!(args.lang, Some(crate::i18n::Language::Japanese));
    assert!(args.save);
}

#[test]
fn requires_latitude_and_longitude_as_a_pair() {
    assert!(Args::try_parse_from(["termrain", "--lat", "35.0"]).is_err());
    assert!(Args::try_parse_from(["termrain", "--lon", "139.0"]).is_err());
    assert!(Args::try_parse_from(["termrain", "--lat", "35.0", "--lon", "139.0"]).is_ok());
}

#[test]
fn parses_completion_shell_and_city_options() {
    let args = Args::try_parse_from([
        "termrain",
        "--city",
        "Tokyo",
        "--list-city",
        "Ueno",
        "--completion",
        "zsh",
    ])
    .unwrap();

    assert_eq!(args.city.as_deref(), Some("Tokyo"));
    assert_eq!(args.list_city.as_deref(), Some("Ueno"));
    assert_eq!(args.completion, Some(Shell::Zsh));
}
