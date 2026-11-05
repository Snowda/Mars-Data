// One sol of MSL weather: the sample type and how it is decoded from the feed.
mod decode;
mod error;
mod fetch;
mod report;
mod values;

use std::ops::ControlFlow;

use arrayvec::ArrayString;
use jiff::civil::Date;
use jiff::civil::Time;
use serde::{Deserialize, Serialize};

pub use decode::parse_weather_body;
pub use error::FetchError;
pub use fetch::load_weather_data;
pub use values::{AtmoOpacity, PressureDirection, SolarLongitude, Temperature, UvIndex, WindDirection};

// The feed writes this where it has no reading.
const NO_READING: &str = "--";
// Byte capacity of the season label; longer labels are cut on a char boundary.
const SEASON_CAP: usize = 32;
/// Upper bound on one sample's JSON, so a serialized sample fits a stack buffer.
///
/// The worst case (24-digit floats, a control-char season escaped to 194 bytes) measures 720 bytes.
pub const MAX_SAMPLE_JSON_BYTES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeatherSample {
    terrestrial_date: Option<Date>,
    sol: u32,
    min_temp: Option<Temperature>,
    max_temp: Option<Temperature>,
    min_ground_temp: Option<Temperature>,
    max_ground_temp: Option<Temperature>,
    pressure: Option<f64>,
    pressure_change_direction: Option<PressureDirection>,
    // Kept as mars_season on the wire, so the published API does not change.
    #[serde(rename = "mars_season")]
    solar_longitude: Option<SolarLongitude>,
    abs_humidity: Option<f64>,
    wind_speed: Option<f64>,
    wind_direction: Option<WindDirection>,
    atmo_opacity: Option<AtmoOpacity>,
    uv_index: Option<UvIndex>,
    season: Option<ArrayString<SEASON_CAP>>,
    sunrise: Option<Time>,
    sunset: Option<Time>,
}

// Fill char-by-char and stop when the next char would overflow.
// A byte slice at a fixed offset can split a multi-byte UTF-8
// char and panic; try_push only ever appends whole chars.
fn truncate_season(s: &str) -> ArrayString<SEASON_CAP> {
    let filled: ControlFlow<ArrayString<SEASON_CAP>, ArrayString<SEASON_CAP>> =
        s.chars().try_fold(ArrayString::new(), |mut buffer: ArrayString<SEASON_CAP>, c: char| {
            return match buffer.try_push(c) {
                Ok(()) => ControlFlow::Continue(buffer),
                Err(_) => ControlFlow::Break(buffer),
            };
        });
    return match filled {
        ControlFlow::Continue(buffer) | ControlFlow::Break(buffer) => buffer,
    };
}

#[cfg(test)]
mod tests {
    use std::str;

    use arrayvec::{ArrayString, ArrayVec};
    use jiff::civil::Date;
    use jiff::civil::Time;
    use serde::Deserialize;
    use serde_json::{Value, json};

    use super::AtmoOpacity;
    use super::FetchError;
    use super::MAX_SAMPLE_JSON_BYTES;
    use super::PressureDirection;
    use super::SEASON_CAP;
    use super::SolarLongitude;
    use super::Temperature;
    use super::UvIndex;
    use super::WeatherSample;
    use super::WindDirection;
    use super::parse_weather_body;
    use crate::mars::openapi_contract::{self, assert_conforms};

    const THIRTY_ASCII: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    // 30 ASCII bytes then a 4-byte char that straddles byte 32.
    const SEASON_STRADDLING_BYTE_32: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\u{1F680}";
    // Eight 4-byte chars: exactly the 32-byte season capacity.
    const EIGHT_ROCKETS: &str = "\u{1F680}\u{1F680}\u{1F680}\u{1F680}\u{1F680}\u{1F680}\u{1F680}\u{1F680}";
    // rustc rounds a float literal exactly as str::parse does, so this is the correctly rounded 1.4e26.
    const FUZZ_PRESSURE: f64 = 1.4e26;
    // JSON escapes it as \u0001, the longest form one byte of season text can take.
    const CONTROL_CHAR: char = '\u{1}';

    fn sample_json(sample: &WeatherSample) -> Value {
        let Ok(value) = serde_json::to_value(sample) else {
            panic!("WeatherSample must serialize");
        };
        return value;
    }

    // Wraps one sol report in a feed body and decodes it as the server does.
    fn decode(report: &Value) -> WeatherSample {
        let mut body: ArrayVec<u8, MAX_SAMPLE_JSON_BYTES> = ArrayVec::new();
        let written: Result<(), serde_json::Error> = serde_json::to_writer(&mut body, &json!({ "soles": [report] }));
        assert!(written.is_ok(), "a test report must fit in {MAX_SAMPLE_JSON_BYTES} bytes");
        let Ok(sample) = parse_weather_body(&body) else {
            panic!("a feed with one sol report must decode");
        };
        return sample;
    }

    fn longitude(degrees: u16) -> Option<SolarLongitude> {
        return SolarLongitude::try_from(degrees).ok();
    }

    // Every field populated; report.rs's tests start from it too.
    pub fn full_sample() -> WeatherSample {
        return WeatherSample {
            terrestrial_date: Date::new(2025, 9, 10).ok(),
            sol: 100,
            min_temp: Some(Temperature::from_celsius(-80.0)),
            max_temp: Some(Temperature::from_celsius(-10.0)),
            min_ground_temp: Some(Temperature::from_celsius(-96.0)),
            max_ground_temp: Some(Temperature::from_celsius(10.0)),
            pressure: Some(750.0),
            pressure_change_direction: Some(PressureDirection::Rising),
            solar_longitude: longitude(5),
            abs_humidity: Some(0.3),
            wind_speed: Some(12.0),
            wind_direction: Some(WindDirection::NW),
            atmo_opacity: Some(AtmoOpacity::Sunny),
            uv_index: Some(UvIndex::Moderate),
            season: ArrayString::from("month 5").ok(),
            sunrise: Time::new(6, 0, 0, 0).ok(),
            sunset: Time::new(18, 0, 0, 0).ok(),
        };
    }

    #[test]
    fn populated_sample_matches_the_openapi_schema() {
        assert_conforms(&openapi_contract::spec(), "WeatherSample", &sample_json(&full_sample()));
    }

    #[test]
    fn all_missing_sample_matches_the_openapi_schema() {
        let empty: WeatherSample = decode(&json!({}));
        assert_conforms(&openapi_contract::spec(), "WeatherSample", &sample_json(&empty));
    }

    #[test]
    fn spec_example_matches_the_schema_and_deserializes() -> Result<(), serde_json::Error> {
        let spec: Value = openapi_contract::spec();
        let example: &Value =
            &spec["paths"]["/weather"]["get"]["responses"]["200"]["content"]["application/json"]["example"];
        assert_conforms(&spec, "WeatherSample", example);
        let decoded: WeatherSample = WeatherSample::deserialize(example)?;
        assert_eq!(&sample_json(&decoded), example, "example survives a round trip unchanged");
        return Ok(());
    }

    #[test]
    fn weather_sample_json_round_trip_preserves_fields() -> Result<(), serde_json::Error> {
        let sample: WeatherSample = full_sample();
        let mut encoded: ArrayVec<u8, MAX_SAMPLE_JSON_BYTES> = ArrayVec::new();
        serde_json::to_writer(&mut encoded, &sample)?;
        let decoded: WeatherSample = serde_json::from_slice(&encoded)?;
        assert_eq!(decoded, sample);
        return Ok(());
    }

    // The longest JSON a sample can produce: longest floats, extreme date and time, a season of
    // control chars that each escape to six bytes.
    fn worst_case_sample() -> WeatherSample {
        let longest: f64 = -f64::MIN_POSITIVE;
        let mut season: ArrayString<SEASON_CAP> = ArrayString::new();
        (0..SEASON_CAP).for_each(|_| season.push(CONTROL_CHAR));
        return WeatherSample {
            terrestrial_date: Some(Date::MIN),
            sol: u32::MAX,
            min_temp: Some(Temperature::from_celsius(longest)),
            max_temp: Some(Temperature::from_celsius(longest)),
            min_ground_temp: Some(Temperature::from_celsius(longest)),
            max_ground_temp: Some(Temperature::from_celsius(longest)),
            pressure: Some(longest),
            pressure_change_direction: Some(PressureDirection::Falling),
            solar_longitude: longitude(359),
            abs_humidity: Some(longest),
            wind_speed: Some(longest),
            wind_direction: Some(WindDirection::Unknown),
            atmo_opacity: Some(AtmoOpacity::Unknown),
            uv_index: Some(UvIndex::VeryHigh),
            season: Some(season),
            sunrise: Some(Time::MAX),
            sunset: Some(Time::MAX),
        };
    }

    #[test]
    fn worst_case_sample_fits_the_json_bound() -> Result<(), serde_json::Error> {
        let mut encoded: ArrayVec<u8, MAX_SAMPLE_JSON_BYTES> = ArrayVec::new();
        serde_json::to_writer(&mut encoded, &worst_case_sample())?;
        // Each field must hit its longest form, or the bound is not proved: the season escapes to 194 bytes.
        let text: &str = str::from_utf8(&encoded).unwrap_or_default();
        assert!(text.contains(r#""season":"\u0001"#), "the season must escape each control char: {text}");
        assert!(encoded.len() > MAX_SAMPLE_JSON_BYTES / 2, "a worst case under half the bound means the bound is loose");        return Ok(());
    }

    #[test]
    fn solar_longitude_keeps_the_mars_season_wire_name() {
        assert_eq!(sample_json(&full_sample())["mars_season"], json!(5), "the wire name must not change");
    }

    #[test]
    fn parse_full_report_populates_all_fields() {
        let report = json!({
            "terrestrial_date": "2025-09-10",
            "sol": "100",
            "min_temp": "-80",
            "max_temp": "-10",
            "min_gts_temp": "-96",
            "max_gts_temp": "10",
            "pressure": "750",
            "pressure_string": "Higher",
            "ls": "5",
            "abs_humidity": "0.3",
            "wind_speed": "12",
            "wind_direction": "NW",
            "atmo_opacity": "Sunny",
            "local_uv_irradiance_index": "Moderate",
            "season": "month 5",
            "sunrise": "06:00",
            "sunset": "18:00"
        });

        let sample = decode(&report);

        assert_eq!(sample.terrestrial_date, Date::new(2025, 9, 10).ok());
        assert_eq!(sample.sol, 100);
        assert_eq!(sample.pressure, Some(750.0));
        assert_eq!(sample.pressure_change_direction, Some(PressureDirection::Rising));
        assert_eq!(sample.solar_longitude, longitude(5));
        assert_eq!(sample.abs_humidity, Some(0.3));
        assert_eq!(sample.wind_speed, Some(12.0));
        assert_eq!(sample.wind_direction, Some(WindDirection::NW));
        assert_eq!(sample.atmo_opacity, Some(AtmoOpacity::Sunny));
        assert_eq!(sample.uv_index, Some(UvIndex::Moderate));
        assert_eq!(sample.season, ArrayString::from("month 5").ok());
        assert_eq!(sample.sunrise, Time::new(6, 0, 0, 0).ok());
        assert_eq!(sample.sunset, Time::new(18, 0, 0, 0).ok());
        assert_eq!(sample.min_temp, Some(Temperature::from_celsius(-80.0)));
        assert_eq!(sample.max_temp, Some(Temperature::from_celsius(-10.0)));
        assert_eq!(sample.min_ground_temp, Some(Temperature::from_celsius(-96.0)));
        assert_eq!(sample.max_ground_temp, Some(Temperature::from_celsius(10.0)));
    }

    #[test]
    fn parse_empty_report_uses_defaults() {
        let report = json!({});

        let sample = decode(&report);

        assert_eq!(sample.terrestrial_date, None);
        assert_eq!(sample.sol, 0);
        assert!(sample.min_temp.is_none());
        assert!(sample.max_temp.is_none());
        assert!(sample.min_ground_temp.is_none());
        assert!(sample.max_ground_temp.is_none());
        assert!(sample.pressure.is_none());
        assert!(sample.pressure_change_direction.is_none());
        assert!(sample.solar_longitude.is_none());
        assert!(sample.abs_humidity.is_none());
        assert!(sample.wind_speed.is_none());
        assert!(sample.wind_direction.is_none());
        assert!(sample.atmo_opacity.is_none());
        assert!(sample.uv_index.is_none());
        assert!(sample.season.is_none());
        assert!(sample.sunrise.is_none());
        assert!(sample.sunset.is_none());
    }

    #[test]
    fn parse_numeric_string_is_coerced() {
        let report = json!({ "pressure": "819" });
        let sample = decode(&report);
        assert_eq!(sample.pressure, Some(819.0));
    }

    #[test]
    fn parse_unavailable_sentinel_yields_none() {
        // The feed uses "--" to mark missing measurements.
        let report = json!({ "abs_humidity": "--", "wind_speed": "--" });
        let sample = decode(&report);
        assert!(sample.abs_humidity.is_none());
        assert!(sample.wind_speed.is_none());
    }

    #[test]
    fn parse_pressure_string_lower_is_falling() {
        let report = json!({ "pressure_string": "Lower" });
        let sample = decode(&report);
        assert_eq!(sample.pressure_change_direction, Some(PressureDirection::Falling));
    }

    #[test]
    fn parse_non_numeric_temp_yields_none() {
        let report = json!({ "min_temp": "cold" });
        let sample = decode(&report);
        assert!(sample.min_temp.is_none());
    }

    #[test]
    fn parse_non_numeric_pressure_yields_none() {
        let report = json!({ "pressure": "high" });
        let sample = decode(&report);
        assert!(sample.pressure.is_none());
    }

    #[test]
    fn parse_sentinel_enum_fields_yield_none() {
        // "--" marks a missing reading, the same as for the numeric fields; Unknown is for values we do not recognise.
        let report: Value = json!({
            "wind_direction": "--", "atmo_opacity": "--", "local_uv_irradiance_index": "--", "pressure_string": "--"
        });
        let sample: WeatherSample = decode(&report);
        assert_eq!(sample.wind_direction, None);
        assert_eq!(sample.atmo_opacity, None);
        assert_eq!(sample.uv_index, None);
        assert_eq!(sample.pressure_change_direction, None, "a missing trend is not a falling one");
    }

    #[test]
    fn parse_unrecognised_pressure_string_yields_none() {
        let sample: WeatherSample = decode(&json!({ "pressure_string": "Sideways" }));
        assert_eq!(sample.pressure_change_direction, None, "only Higher and Lower name a trend");
    }

    #[test]
    fn parse_invalid_terrestrial_date_yields_none() {
        let report = json!({ "terrestrial_date": 42 });
        let sample = decode(&report);
        assert_eq!(sample.terrestrial_date, None);
    }

    #[test]
    fn parse_non_integer_sol_falls_back_to_zero() {
        let report = json!({ "sol": "not-a-number" });
        let sample = decode(&report);
        assert_eq!(sample.sol, 0);
    }

    #[test]
    fn parse_sol_from_json_number() {
        // The feed may emit sol as a bare JSON number rather than a string.
        let report = json!({ "sol": 100 });
        let sample = decode(&report);
        assert_eq!(sample.sol, 100);
    }

    #[test]
    fn parse_fractional_sol_string_falls_back_to_zero() {
        // sol is an integer count: a fractional string is rejected (not truncated).
        let report = json!({ "sol": "100.9" });
        let sample = decode(&report);
        assert_eq!(sample.sol, 0);
    }

    #[test]
    fn parse_ls_from_json_number() {
        let report = json!({ "ls": 5 });
        let sample = decode(&report);
        assert_eq!(sample.solar_longitude, longitude(5));
    }

    #[test]
    fn parse_fractional_ls_string_yields_none() {
        // ls is an integer degree: a fractional string is rejected (not truncated).
        let report = json!({ "ls": "5.5" });
        let sample = decode(&report);
        assert_eq!(sample.solar_longitude, None);
    }

    #[test]
    fn parse_ls_outside_one_orbit_yields_none() {
        let at = |ls: Value| -> Option<SolarLongitude> { return decode(&json!({ "ls": ls })).solar_longitude };
        assert!(longitude(359).is_some(), "359 is the last degree before the orbit wraps");
        assert_eq!(at(json!(359)), longitude(359));
        assert_eq!(at(json!(360)), None, "360 degrees is 0 again, so the feed must not report it");
        assert_eq!(at(json!("-1")), None, "solar longitude is never negative");
        assert_eq!(at(json!(70_000)), None, "a value past u16 must not wrap into range");
    }

    #[test]
    fn parse_all_wind_directions() {
        [
            ("N",  WindDirection::N),
            ("NE", WindDirection::NE),
            ("E",  WindDirection::E),
            ("SE", WindDirection::SE),
            ("S",  WindDirection::S),
            ("SW", WindDirection::SW),
            ("W",  WindDirection::W),
            ("NW", WindDirection::NW),
            ("X",  WindDirection::Unknown),
        ]
        .into_iter()
        .for_each(|(s, expected): (&str, WindDirection)| {
            let report = json!({ "wind_direction": s });
            assert_eq!(decode(&report).wind_direction, Some(expected));
        });
    }

    #[test]
    fn parse_all_atmo_opacity_variants() {
        [
            ("Sunny",  AtmoOpacity::Sunny),
            ("Cloudy", AtmoOpacity::Cloudy),
            ("Dusty",  AtmoOpacity::Dusty),
            ("Foggy",  AtmoOpacity::Unknown),
        ]
        .into_iter()
        .for_each(|(s, expected): (&str, AtmoOpacity)| {
            let report = json!({ "atmo_opacity": s });
            assert_eq!(decode(&report).atmo_opacity, Some(expected));
        });
    }

    #[test]
    fn parse_multibyte_season_truncates_on_char_boundary() {
        // 30 ASCII bytes + one 4-byte char = 34 bytes. The old fixed-offset byte
        // slice panicked here ("byte index 32 is not a char boundary"); the emoji
        // straddles byte 32 so it must be dropped whole, leaving the 30 ASCII bytes.
        let report = json!({ "season": SEASON_STRADDLING_BYTE_32 });
        let Some(parsed) = decode(&report).season else {
            panic!("season should be populated");
        };
        assert_eq!(parsed.as_str(), THIRTY_ASCII);
        assert!(parsed.len() <= parsed.capacity());
    }

    #[test]
    fn parse_multibyte_season_fills_to_exact_capacity() {
        // Eight 4-byte chars = exactly 32 bytes: all fit, none are split.
        let report = json!({ "season": EIGHT_ROCKETS });
        let Some(parsed) = decode(&report).season else {
            panic!("season should be populated");
        };
        assert_eq!(parsed.len(), 32);
        assert_eq!(parsed.as_str(), EIGHT_ROCKETS);
    }

    #[test]
    fn parse_all_uv_index_variants() {
        [
            ("Low",       UvIndex::Low),
            ("Moderate",  UvIndex::Moderate),
            ("High",      UvIndex::High),
            ("Very_High", UvIndex::VeryHigh),
            ("Extreme",   UvIndex::Extreme),
            ("Blinding",  UvIndex::Unknown),
        ]
        .into_iter()
        .for_each(|(s, expected): (&str, UvIndex)| {
            let report = json!({ "local_uv_irradiance_index": s });
            assert_eq!(decode(&report).uv_index, Some(expected));
        });
    }

    #[test]
    fn parse_weather_body_takes_the_first_sol() -> Result<(), FetchError> {
        let body: &[u8] = br#"{"soles":[{"sol":"4000","wind_direction":"N"},{"sol":"3999"}]}"#;
        let sample: WeatherSample = parse_weather_body(body)?;
        assert_eq!(sample.sol, 4000);
        assert_eq!(sample.wind_direction, Some(WindDirection::N));
        return Ok(());
    }

    #[test]
    fn parse_weather_body_reads_json_floats_exactly() -> Result<(), FetchError> {
        // Fuzz regression (weather_body/crash-495f…): without serde_json float_roundtrip this lands 1 ULP high.
        let sample: WeatherSample = parse_weather_body(br#"{"soles":[{"pressure":1.4e+26}]}"#)?;
        assert_eq!(sample.pressure.map(f64::to_bits), Some(FUZZ_PRESSURE.to_bits()));
        return Ok(());
    }

    #[test]
    fn parse_weather_body_rejects_invalid_json_and_missing_soles() {
        assert!(matches!(parse_weather_body(b"{\"soles\":"), Err(FetchError::InvalidJson(_))), "truncated JSON");
        [&b"{}"[..], br#"{"soles":[]}"#, br#"{"soles":"x"}"#, b"[]"].into_iter().for_each(|body: &[u8]| {
            assert!(matches!(parse_weather_body(body), Err(FetchError::NoSolData)), "no first sol in {body:?}");
        });
    }
}
