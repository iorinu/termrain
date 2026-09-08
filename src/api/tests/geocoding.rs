use super::{Resp, RespHit, to_hit};

#[test]
fn converts_geocoding_response_hits_without_losing_location_metadata() {
    let response: Resp = serde_json::from_str(
        r#"{
            "results": [{
                "name": "Tokyo",
                "latitude": 35.6812,
                "longitude": 139.7671,
                "country_code": "JP",
                "country": "Japan",
                "admin1": "Tokyo"
            }]
        }"#,
    )
    .unwrap();
    let hit = to_hit(response.results.unwrap().pop().unwrap());

    assert_eq!(hit.name, "Tokyo");
    assert_eq!(hit.admin1.as_deref(), Some("Tokyo"));
    assert_eq!(hit.country_name.as_deref(), Some("Japan"));
    assert_eq!(hit.country, "JP");
    assert_eq!(hit.latitude, 35.6812);
    assert_eq!(hit.longitude, 139.7671);
}

#[test]
fn uses_empty_country_code_when_the_api_omits_it() {
    let hit = to_hit(RespHit {
        name: "Unknown".into(),
        latitude: 0.0,
        longitude: 0.0,
        country_code: None,
        country: None,
        admin1: None,
    });

    assert_eq!(hit.country, "");
    assert!(hit.country_name.is_none());
    assert!(hit.admin1.is_none());
}
