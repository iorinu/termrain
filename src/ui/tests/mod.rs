use super::*;

#[test]
fn radar_aspect_is_clamped_for_narrow_and_wide_terminals() {
    assert_eq!(desired_radar_aspect(1, 1, None), 1.0);
    assert_eq!(desired_radar_aspect(300, 20, None), 2.4);
}

#[test]
fn radar_aspect_uses_the_reported_font_size() {
    let font = ratatui_image::FontSize {
        width: 10,
        height: 20,
    };

    let aspect = desired_radar_aspect(160, 40, Some(font));

    assert!((aspect - 1.785714).abs() < 1e-6);
}

#[test]
fn summarizes_network_errors_for_the_single_line_footer() {
    assert_eq!(
        summarize_error(
            "request failed\nfor url (https://example.com/very/long)",
            64
        ),
        "request failed"
    );
    assert_eq!(summarize_error("abcdefghijk", 5), "abcd…");
    assert_eq!(summarize_error("晴れ", 4), "晴れ");
}
