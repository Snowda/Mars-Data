// One sol of MSL weather: the sample type and how it is decoded from the feed.
mod fetch;
mod report;
mod values;

use arrayvec::ArrayString;
use jiff::civil::Date;
use jiff::civil::Time;
use serde::de::value::{Error as ValueError, StrDeserializer};
use serde::de::{DeserializeOwned, IntoDeserializer};
use serde::{Deserialize, Serialize};
use serde_json::value::Value;

pub use fetch::load_weather_data;
pub use values::{AtmoOpacity, PressureDirection, SolarLongitude, Temperature, UvIndex, WindDirection};

// The feed writes this where it has no reading.
const NO_READING: &str = "--";

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
    season: Option<ArrayString<32>>,
    sunrise: Option<Time>,
    sunset: Option<Time>,
}

// Reads a field that may arrive as a JSON number or a numeric string (the MSL
// feed encodes all measurements as strings, and uses "--" for missing values).
fn parse_f64(report: &Value, key: &str) -> Option<f64> {
    let value: &Value = report.get(key)?;
    return value.as_f64().or_else(|| value.as_str().and_then(|s| s.parse::<f64>().ok()));
}

// Same encoding as parse_f64, for fields the feed reports as whole numbers (sol
// count, solar longitude). Parsing as an integer avoids a lossy f64 truncation.
fn parse_u32(report: &Value, key: &str) -> Option<u32> {
    let value: &Value = report.get(key)?;
    return value.as_u64().and_then(|n| u32::try_from(n).ok())
        .or_else(|| value.as_str().and_then(|s| s.parse::<u32>().ok()));
}

// A string field the feed reported: present, a string, and not the NO_READING sentinel.
fn feed_str<'a>(report: &'a Value, key: &str) -> Option<&'a str> {
    return report.get(key).and_then(Value::as_str).filter(|s: &&str| *s != NO_READING);
}

// Decodes a feed label through the enum's serde names and aliases.
fn feed_enum<T: DeserializeOwned>(report: &Value, key: &str) -> Option<T> {
    let deserializer: StrDeserializer<'_, ValueError> = feed_str(report, key)?.into_deserializer();
    return T::deserialize(deserializer).ok();
}

impl WeatherSample {
    fn parse(report: &Value) -> Self {
        let terrestrial_date: Option<Date> =
            feed_str(report, "terrestrial_date").and_then(|s| Date::strptime("%Y-%m-%d", s).ok());

        let sol: u32 = parse_u32(report, "sol").unwrap_or(0);

        let min_temp: Option<Temperature> = parse_f64(report, "min_temp").map(Temperature::from_celsius);
        let max_temp: Option<Temperature> = parse_f64(report, "max_temp").map(Temperature::from_celsius);
        let min_ground_temp: Option<Temperature> = parse_f64(report, "min_gts_temp").map(Temperature::from_celsius);
        let max_ground_temp: Option<Temperature> = parse_f64(report, "max_gts_temp").map(Temperature::from_celsius);

        let pressure: Option<f64> = parse_f64(report, "pressure");

        let pressure_change_direction: Option<PressureDirection> = feed_enum(report, "pressure_string");

        let solar_longitude: Option<SolarLongitude> = parse_u32(report, "ls")
            .and_then(|degrees: u32| u16::try_from(degrees).ok())
            .and_then(|degrees: u16| SolarLongitude::try_from(degrees).ok());
        let abs_humidity: Option<f64> = parse_f64(report, "abs_humidity");
        let wind_speed: Option<f64> = parse_f64(report, "wind_speed");

        let wind_direction: Option<WindDirection> = feed_enum(report, "wind_direction");
        let atmo_opacity: Option<AtmoOpacity> = feed_enum(report, "atmo_opacity");
        let uv_index: Option<UvIndex> = feed_enum(report, "local_uv_irradiance_index");

        let season: Option<ArrayString<32>> = feed_str(report, "season")
            .map(|s| {
                // Fill char-by-char and stop when the next char would overflow.
                // A byte slice at a fixed offset can split a multi-byte UTF-8
                // char and panic; try_push only ever appends whole chars.
                let mut buffer: ArrayString<32> = ArrayString::new();
                for c in s.chars() {
                    if buffer.try_push(c).is_err() {
                        break;
                    }
                }
                return buffer;
            });

        let sunrise: Option<Time> = feed_str(report, "sunrise").and_then(|s| Time::strptime("%H:%M", s).ok());
        let sunset: Option<Time> = feed_str(report, "sunset").and_then(|s| Time::strptime("%H:%M", s).ok());

        return Self {
            terrestrial_date,
            sol,
            min_temp,
            max_temp,
            min_ground_temp,
            max_ground_temp,
            pressure,
            pressure_change_direction,
            solar_longitude,
            abs_humidity,
            wind_speed,
            wind_direction,
            atmo_opacity,
            uv_index,
            season,
            sunrise,
            sunset,
        };
    }
}

// Decodes an MSL feed body into its latest sol. The body is untrusted upstream input (fuzzed in fuzz/).
pub fn parse_weather_body(body: &[u8]) -> Result<WeatherSample, String> {
    let json: Value = match serde_json::from_slice(body) {
        Ok(parsed_json) => parsed_json,
        Err(e) => return Err(format!("Failed to parse JSON response: {}", e)),
    };

    return match json.get("soles").and_then(Value::as_array).and_then(|soles| soles.first()) {
        Some(latest) => Ok(WeatherSample::parse(latest)),
        None => Err("No sol data found in response".to_string()),
    };
}

#[cfg(test)]
mod tests {
    use arrayvec::ArrayString;
    use jiff::civil::Date;
    use jiff::civil::Time;
    use serde_json::{Value, json};

    use super::AtmoOpacity;
    use super::PressureDirection;
    use super::SolarLongitude;
    use super::Temperature;
    use super::UvIndex;
    use super::WeatherSample;
    use super::WindDirection;
    use super::parse_weather_body;
    use crate::mars::openapi_contract::{self, assert_conforms};

    fn sample_json(sample: &WeatherSample) -> Value {
        let Ok(value) = serde_json::to_value(sample) else {
            panic!("WeatherSample must serialize");
        };
        return value;
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
        let empty: WeatherSample = WeatherSample::parse(&json!({}));
        assert_conforms(&openapi_contract::spec(), "WeatherSample", &sample_json(&empty));
    }

    #[test]
    fn spec_example_matches_the_schema_and_deserializes() -> Result<(), serde_json::Error> {
        let spec: Value = openapi_contract::spec();
        let example: &Value =
            &spec["paths"]["/weather"]["get"]["responses"]["200"]["content"]["application/json"]["example"];
        assert_conforms(&spec, "WeatherSample", example);
        let decoded: WeatherSample = serde_json::from_value(example.clone())?;
        assert_eq!(&sample_json(&decoded), example, "example survives a round trip unchanged");
        return Ok(());
    }

    #[test]
    fn weather_sample_json_round_trip_preserves_fields() -> Result<(), serde_json::Error> {
        let sample: WeatherSample = full_sample();
        let encoded: String = serde_json::to_string(&sample)?;
        let decoded: WeatherSample = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, sample);
        return Ok(());
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

        let sample = WeatherSample::parse(&report);

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

        let sample = WeatherSample::parse(&report);

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
        let sample = WeatherSample::parse(&report);
        assert_eq!(sample.pressure, Some(819.0));
    }

    #[test]
    fn parse_unavailable_sentinel_yields_none() {
        // The feed uses "--" to mark missing measurements.
        let report = json!({ "abs_humidity": "--", "wind_speed": "--" });
        let sample = WeatherSample::parse(&report);
        assert!(sample.abs_humidity.is_none());
        assert!(sample.wind_speed.is_none());
    }

    #[test]
    fn parse_pressure_string_lower_is_falling() {
        let report = json!({ "pressure_string": "Lower" });
        let sample = WeatherSample::parse(&report);
        assert_eq!(sample.pressure_change_direction, Some(PressureDirection::Falling));
    }

    #[test]
    fn parse_non_numeric_temp_yields_none() {
        let report = json!({ "min_temp": "cold" });
        let sample = WeatherSample::parse(&report);
        assert!(sample.min_temp.is_none());
    }

    #[test]
    fn parse_non_numeric_pressure_yields_none() {
        let report = json!({ "pressure": "high" });
        let sample = WeatherSample::parse(&report);
        assert!(sample.pressure.is_none());
    }

    #[test]
    fn parse_sentinel_enum_fields_yield_none() {
        // "--" marks a missing reading, the same as for the numeric fields; Unknown is for values we do not recognise.
        let report: Value = json!({
            "wind_direction": "--", "atmo_opacity": "--", "local_uv_irradiance_index": "--", "pressure_string": "--"
        });
        let sample: WeatherSample = WeatherSample::parse(&report);
        assert_eq!(sample.wind_direction, None);
        assert_eq!(sample.atmo_opacity, None);
        assert_eq!(sample.uv_index, None);
        assert_eq!(sample.pressure_change_direction, None, "a missing trend is not a falling one");
    }

    #[test]
    fn parse_unrecognised_pressure_string_yields_none() {
        let sample: WeatherSample = WeatherSample::parse(&json!({ "pressure_string": "Sideways" }));
        assert_eq!(sample.pressure_change_direction, None, "only Higher and Lower name a trend");
    }

    #[test]
    fn parse_invalid_terrestrial_date_yields_none() {
        let report = json!({ "terrestrial_date": 42 });
        let sample = WeatherSample::parse(&report);
        assert_eq!(sample.terrestrial_date, None);
    }

    #[test]
    fn parse_non_integer_sol_falls_back_to_zero() {
        let report = json!({ "sol": "not-a-number" });
        let sample = WeatherSample::parse(&report);
        assert_eq!(sample.sol, 0);
    }

    #[test]
    fn parse_sol_from_json_number() {
        // The feed may emit sol as a bare JSON number rather than a string.
        let report = json!({ "sol": 100 });
        let sample = WeatherSample::parse(&report);
        assert_eq!(sample.sol, 100);
    }

    #[test]
    fn parse_fractional_sol_string_falls_back_to_zero() {
        // sol is an integer count: a fractional string is rejected (not truncated).
        let report = json!({ "sol": "100.9" });
        let sample = WeatherSample::parse(&report);
        assert_eq!(sample.sol, 0);
    }

    #[test]
    fn parse_ls_from_json_number() {
        let report = json!({ "ls": 5 });
        let sample = WeatherSample::parse(&report);
        assert_eq!(sample.solar_longitude, longitude(5));
    }

    #[test]
    fn parse_fractional_ls_string_yields_none() {
        // ls is an integer degree: a fractional string is rejected (not truncated).
        let report = json!({ "ls": "5.5" });
        let sample = WeatherSample::parse(&report);
        assert_eq!(sample.solar_longitude, None);
    }

    #[test]
    fn parse_ls_outside_one_orbit_yields_none() {
        let at = |ls: Value| -> Option<SolarLongitude> { return WeatherSample::parse(&json!({ "ls": ls })).solar_longitude };
        assert!(longitude(359).is_some(), "359 is the last degree before the orbit wraps");
        assert_eq!(at(json!(359)), longitude(359));
        assert_eq!(at(json!(360)), None, "360 degrees is 0 again, so the feed must not report it");
        assert_eq!(at(json!("-1")), None, "solar longitude is never negative");
        assert_eq!(at(json!(70_000)), None, "a value past u16 must not wrap into range");
    }

    #[test]
    fn parse_all_wind_directions() {
        for (s, expected) in [
            ("N",  WindDirection::N),
            ("NE", WindDirection::NE),
            ("E",  WindDirection::E),
            ("SE", WindDirection::SE),
            ("S",  WindDirection::S),
            ("SW", WindDirection::SW),
            ("W",  WindDirection::W),
            ("NW", WindDirection::NW),
            ("X",  WindDirection::Unknown),
        ] {
            let report = json!({ "wind_direction": s });
            assert_eq!(WeatherSample::parse(&report).wind_direction, Some(expected));
        }
    }

    #[test]
    fn parse_all_atmo_opacity_variants() {
        for (s, expected) in [
            ("Sunny",  AtmoOpacity::Sunny),
            ("Cloudy", AtmoOpacity::Cloudy),
            ("Dusty",  AtmoOpacity::Dusty),
            ("Foggy",  AtmoOpacity::Unknown),
        ] {
            let report = json!({ "atmo_opacity": s });
            assert_eq!(WeatherSample::parse(&report).atmo_opacity, Some(expected));
        }
    }

    #[test]
    fn parse_multibyte_season_truncates_on_char_boundary() {
        // 30 ASCII bytes + one 4-byte char = 34 bytes. The old fixed-offset byte
        // slice panicked here ("byte index 32 is not a char boundary"); the emoji
        // straddles byte 32 so it must be dropped whole, leaving the 30 ASCII bytes.
        let season: String = format!("{}{}", "a".repeat(30), '\u{1F680}');
        let report = json!({ "season": season });
        let Some(parsed) = WeatherSample::parse(&report).season else {
            panic!("season should be populated");
        };
        assert_eq!(parsed.as_str(), "a".repeat(30));
        assert!(parsed.len() <= parsed.capacity());
    }

    #[test]
    fn parse_multibyte_season_fills_to_exact_capacity() {
        // Eight 4-byte chars = exactly 32 bytes: all fit, none are split.
        let season: String = "\u{1F680}".repeat(8);
        let report = json!({ "season": season });
        let Some(parsed) = WeatherSample::parse(&report).season else {
            panic!("season should be populated");
        };
        assert_eq!(parsed.len(), 32);
        assert_eq!(parsed.as_str(), "\u{1F680}".repeat(8));
    }

    #[test]
    fn parse_all_uv_index_variants() {
        for (s, expected) in [
            ("Low",       UvIndex::Low),
            ("Moderate",  UvIndex::Moderate),
            ("High",      UvIndex::High),
            ("Very_High", UvIndex::VeryHigh),
            ("Extreme",   UvIndex::Extreme),
            ("Blinding",  UvIndex::Unknown),
        ] {
            let report = json!({ "local_uv_irradiance_index": s });
            assert_eq!(WeatherSample::parse(&report).uv_index, Some(expected));
        }
    }

    #[test]
    fn parse_weather_body_takes_the_first_sol() -> Result<(), String> {
        let body: &[u8] = br#"{"soles":[{"sol":"4000","wind_direction":"N"},{"sol":"3999"}]}"#;
        let sample: WeatherSample = parse_weather_body(body)?;
        assert_eq!(sample.sol, 4000);
        assert_eq!(sample.wind_direction, Some(WindDirection::N));
        return Ok(());
    }

    #[test]
    fn parse_weather_body_reads_json_floats_exactly() -> Result<(), String> {
        // Fuzz regression (weather_body/crash-495f…): without serde_json float_roundtrip this lands 1 ULP high.
        let sample: WeatherSample = parse_weather_body(br#"{"soles":[{"pressure":1.4e+26}]}"#)?;
        let exact: f64 = "1.4e26".parse::<f64>().map_err(|e| e.to_string())?;
        assert_eq!(sample.pressure.map(f64::to_bits), Some(exact.to_bits()));
        return Ok(());
    }

    #[test]
    fn parse_weather_body_rejects_invalid_json_and_missing_soles() {
        assert!(parse_weather_body(b"{\"soles\":").is_err_and(|e| e.starts_with("Failed to parse JSON response")));
        for body in [&b"{}"[..], br#"{"soles":[]}"#, br#"{"soles":"x"}"#, b"[]"] {
            assert_eq!(parse_weather_body(body), Err("No sol data found in response".to_owned()));
        }
    }
}
