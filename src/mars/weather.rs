use std::fmt::{self, Display, Formatter};
use std::time::Duration;

use arrayvec::ArrayString;
use chrono::NaiveDate;
use chrono::NaiveTime;
use reqwest::StatusCode;
use serde_json::value::Value;
use tracing::info;

const WEATHER_URL: &str = "https://mars.nasa.gov/rss/api/?feed=weather&category=msl&feedtype=json";
const REQUEST_TIMEOUT_SECS: u64 = 5;
const CONNECT_TIMEOUT_SECS: u64 = 3;

#[derive(Clone, Debug, PartialEq)]
pub enum WindDirection {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
    Unknown,
}

impl WindDirection {
    fn parse(s: &str) -> Self {
        return match s {
            "N"  => WindDirection::N,
            "NE" => WindDirection::NE,
            "E"  => WindDirection::E,
            "SE" => WindDirection::SE,
            "S"  => WindDirection::S,
            "SW" => WindDirection::SW,
            "W"  => WindDirection::W,
            "NW" => WindDirection::NW,
            _    => WindDirection::Unknown,
        };
    }
}

impl Display for WindDirection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return match self {
            WindDirection::N       => write!(f, "N"),
            WindDirection::NE      => write!(f, "NE"),
            WindDirection::E       => write!(f, "E"),
            WindDirection::SE      => write!(f, "SE"),
            WindDirection::S       => write!(f, "S"),
            WindDirection::SW      => write!(f, "SW"),
            WindDirection::W       => write!(f, "W"),
            WindDirection::NW      => write!(f, "NW"),
            WindDirection::Unknown => write!(f, "Unknown"),
        };
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AtmoOpacity {
    Sunny,
    Cloudy,
    Dusty,
    Unknown,
}

impl AtmoOpacity {
    fn parse(s: &str) -> Self {
        return match s {
            "Sunny" => AtmoOpacity::Sunny,
            "Cloudy" => AtmoOpacity::Cloudy,
            "Dusty" => AtmoOpacity::Dusty,
            _ => AtmoOpacity::Unknown,
        };
    }
}

impl Display for AtmoOpacity {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return match self {
            AtmoOpacity::Sunny   => write!(f, "Sunny"),
            AtmoOpacity::Cloudy  => write!(f, "Cloudy"),
            AtmoOpacity::Dusty   => write!(f, "Dusty"),
            AtmoOpacity::Unknown => write!(f, "Unknown"),
        };
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum UvIndex {
    Low,
    Moderate,
    High,
    VeryHigh,
    Extreme,
    Unknown,
}

impl UvIndex {
    fn parse(s: &str) -> Self {
        return match s {
            "Low"       => UvIndex::Low,
            "Moderate"  => UvIndex::Moderate,
            "High"      => UvIndex::High,
            "Very_High" => UvIndex::VeryHigh,
            "Extreme"   => UvIndex::Extreme,
            _           => UvIndex::Unknown,
        };
    }
}

impl Display for UvIndex {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return match self {
            UvIndex::Low      => write!(f, "Low"),
            UvIndex::Moderate => write!(f, "Moderate"),
            UvIndex::High     => write!(f, "High"),
            UvIndex::VeryHigh => write!(f, "Very High"),
            UvIndex::Extreme  => write!(f, "Extreme"),
            UvIndex::Unknown  => write!(f, "Unknown"),
        };
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PressureDirection {
    Rising,
    Falling,
}

impl Display for PressureDirection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return match self {
            PressureDirection::Rising  => write!(f, "Rising"),
            PressureDirection::Falling => write!(f, "Falling"),
        };
    }
}

#[derive(Clone, Debug)]
pub struct Temperature {
    celsius: f64,
}

impl Temperature {
    fn from_celsius(celsius: f64) -> Self {
        return Temperature { celsius };
    }
}

impl Display for Temperature {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return write!(f, "{} °C", self.celsius);
    }
}

#[derive(Clone, Debug)]
pub struct WeatherSample {
    terrestrial_date: Option<NaiveDate>,
    sol: u32,
    min_temp: Option<Temperature>,
    max_temp: Option<Temperature>,
    min_ground_temp: Option<Temperature>,
    max_ground_temp: Option<Temperature>,
    pressure: Option<f64>,
    pressure_change_direction: Option<PressureDirection>,
    mars_season: Option<i32>,
    abs_humidity: Option<f64>,
    wind_speed: Option<f64>,
    wind_direction: Option<WindDirection>,
    atmo_opacity: Option<AtmoOpacity>,
    uv_index: Option<UvIndex>,
    season: Option<ArrayString<32>>,
    sunrise: Option<NaiveTime>,
    sunset: Option<NaiveTime>,
}

// Reads a field that may arrive as a JSON number or a numeric string (the MSL
// feed encodes all measurements as strings, and uses "--" for missing values).
fn parse_f64(report: &Value, key: &str) -> Option<f64> {
    let value: &Value = report.get(key)?;
    return value.as_f64().or_else(|| value.as_str().and_then(|s| s.parse::<f64>().ok()));
}

impl WeatherSample {
    pub fn print_report(&self) -> &Self {
        info!("Date: {:?} (sol {})", self.terrestrial_date, self.sol);

        match &self.min_temp {
            Some(temp) => info!("Today's lowest air temperature was {}", temp),
            None => info!("No minimum air temperature data available"),
        }

        match &self.max_temp {
            Some(temp) => info!("Today's highest air temperature was {}", temp),
            None => info!("No maximum air temperature data available"),
        }

        match &self.min_ground_temp {
            Some(temp) => info!("Today's lowest ground temperature was {}", temp),
            None => info!("No minimum ground temperature data available"),
        }

        match &self.max_ground_temp {
            Some(temp) => info!("Today's highest ground temperature was {}", temp),
            None => info!("No maximum ground temperature data available"),
        }

        match &self.pressure_change_direction {
            Some(PressureDirection::Rising)  => info!("Atmospheric pressure is rising."),
            Some(PressureDirection::Falling) => info!("Atmospheric pressure is falling."),
            None => info!("No pressure change direction data available"),
        }

        info!("Atmospheric pressure is {:?} and {:?}", self.pressure, self.pressure_change_direction);
        info!("Mars season: {:?}", self.mars_season);

        match &self.abs_humidity {
            Some(humidity) => info!("Humidity: {}", humidity),
            None => info!("No Humidity Available"),
        }

        match (&self.wind_speed, &self.wind_direction) {
            (Some(speed), Some(direction)) => info!("The wind is blowing {} at a speed of {} km/s", direction, speed),
            _ => info!("No wind data available"),
        }

        match &self.uv_index {
            Some(uv) => info!("UV irradiance index: {}", uv),
            None => info!("No UV data available"),
        }

        info!("The weather is {:?}", self.atmo_opacity);
        info!("{:?}", self.season);
        info!("The sun rises at {:?}", self.sunrise);
        info!("The sun sets at {:?}", self.sunset);
        return self;
    }

    fn parse(report: &Value) -> Self {
        let terrestrial_date: Option<NaiveDate> = report
            .get("terrestrial_date")
            .and_then(Value::as_str)
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let sol: u32 = parse_f64(report, "sol").unwrap_or(0.0) as u32;

        let min_temp: Option<Temperature> = parse_f64(report, "min_temp").map(Temperature::from_celsius);
        let max_temp: Option<Temperature> = parse_f64(report, "max_temp").map(Temperature::from_celsius);
        let min_ground_temp: Option<Temperature> = parse_f64(report, "min_gts_temp").map(Temperature::from_celsius);
        let max_ground_temp: Option<Temperature> = parse_f64(report, "max_gts_temp").map(Temperature::from_celsius);

        let pressure: Option<f64> = parse_f64(report, "pressure");

        let pressure_change_direction: Option<PressureDirection> = report
            .get("pressure_string")
            .and_then(Value::as_str)
            .map(|s| if s == "Higher" { PressureDirection::Rising } else { PressureDirection::Falling });

        let mars_season: Option<i32> = parse_f64(report, "ls").map(|v| v as i32);
        let abs_humidity: Option<f64> = parse_f64(report, "abs_humidity");
        let wind_speed: Option<f64> = parse_f64(report, "wind_speed");

        let wind_direction: Option<WindDirection> = report
            .get("wind_direction")
            .map(|v| WindDirection::parse(v.as_str().unwrap_or("")));

        let atmo_opacity: Option<AtmoOpacity> = report
            .get("atmo_opacity")
            .map(|v| AtmoOpacity::parse(v.as_str().unwrap_or("")));

        let uv_index: Option<UvIndex> = report
            .get("local_uv_irradiance_index")
            .and_then(Value::as_str)
            .map(UvIndex::parse);

        let season: Option<ArrayString<32>> = report
            .get("season")
            .and_then(Value::as_str)
            .map(|s| {
                let mut buffer: ArrayString<32> = ArrayString::new();
                buffer.push_str(&s[..s.len().min(buffer.capacity())]);
                return buffer;
            });

        let sunrise: Option<NaiveTime> = report
            .get("sunrise")
            .and_then(Value::as_str)
            .and_then(|s| NaiveTime::parse_from_str(s, "%H:%M").ok());

        let sunset: Option<NaiveTime> = report
            .get("sunset")
            .and_then(Value::as_str)
            .and_then(|s| NaiveTime::parse_from_str(s, "%H:%M").ok());

        return Self {
            terrestrial_date,
            sol,
            min_temp,
            max_temp,
            min_ground_temp,
            max_ground_temp,
            pressure,
            pressure_change_direction,
            mars_season,
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

pub async fn load_weather_data() -> Result<WeatherSample, String> { // TODO typed error codes
    let client_builder = reqwest::Client::builder()
        .gzip(true)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .build();

    return match client_builder { // TODO better code style than match chain
        Ok(client) => {
            let resp = client.get(WEATHER_URL).send();

            match resp.await { // TODO poll instead of await
                Ok(response) => {
                    match response.status() {
                        StatusCode::OK => {
                            let json: Value = match response.json().await { // TODO replace await with polling
                                Ok(parsed_json) => parsed_json,
                                Err(e) => return Err(format!("Failed to parse JSON response: {}", e)),
                            };

                            match json.get("soles").and_then(Value::as_array).and_then(|soles| soles.first()) {
                                Some(latest) => Ok(WeatherSample::parse(latest)),
                                None => Err("No sol data found in response".to_string()),
                            }
                        },
                        StatusCode::PAYLOAD_TOO_LARGE => {
                            Err(format!("Payload too large: Status code {}", StatusCode::PAYLOAD_TOO_LARGE))
                        },
                        s => {
                            Err(format!("Unexpected status code: {}", s))
                        },
                    }
                },
                Err(error) => Err(format!("Request error: {}", error)),
            }
        },
        Err(error) => Err(format!("Failed to build HTTP client: {}", error)),
    };
}

#[cfg(test)]
mod tests {
    use arrayvec::ArrayString;
    use chrono::NaiveDate;
    use chrono::NaiveTime;
    use serde_json::json;

    use super::AtmoOpacity;
    use super::PressureDirection;
    use super::Temperature;
    use super::UvIndex;
    use super::WeatherSample;
    use super::WindDirection;

    fn full_sample() -> WeatherSample {
        return WeatherSample {
            terrestrial_date: NaiveDate::from_ymd_opt(2025, 9, 10),
            sol: 100,
            min_temp: Some(Temperature::from_celsius(-80.0)),
            max_temp: Some(Temperature::from_celsius(-10.0)),
            min_ground_temp: Some(Temperature::from_celsius(-96.0)),
            max_ground_temp: Some(Temperature::from_celsius(10.0)),
            pressure: Some(750.0),
            pressure_change_direction: Some(PressureDirection::Rising),
            mars_season: Some(5),
            abs_humidity: Some(0.3),
            wind_speed: Some(12.0),
            wind_direction: Some(WindDirection::NW),
            atmo_opacity: Some(AtmoOpacity::Sunny),
            uv_index: Some(UvIndex::Moderate),
            season: Some(ArrayString::from("month 5").unwrap()),
            sunrise: NaiveTime::from_hms_opt(6, 0, 0),
            sunset: NaiveTime::from_hms_opt(18, 0, 0),
        };
    }

    #[test]
    fn temperature_from_celsius_stores_value() {
        let t = Temperature::from_celsius(20.0);
        assert_eq!(t.celsius, 20.0);
    }

    #[test]
    fn temperature_display_formats_celsius() {
        let t = Temperature::from_celsius(-12.5);
        assert_eq!(format!("{}", t), "-12.5 °C");
    }

    #[test]
    fn print_report_returns_same_instance() {
        let sample = full_sample();
        let returned = sample.print_report();
        assert!(std::ptr::eq(returned, &sample));
    }

    #[test]
    fn print_report_handles_all_none_optionals() {
        let mut sample = full_sample();
        sample.min_temp = None;
        sample.max_temp = None;
        sample.min_ground_temp = None;
        sample.max_ground_temp = None;
        sample.pressure_change_direction = None;
        sample.abs_humidity = None;
        sample.wind_speed = None;
        sample.wind_direction = None;
        sample.uv_index = None;
        let returned = sample.print_report();
        assert!(std::ptr::eq(returned, &sample));
    }

    #[test]
    fn print_report_reports_falling_pressure() {
        let mut sample = full_sample();
        sample.pressure_change_direction = Some(PressureDirection::Falling);
        assert!(std::ptr::eq(sample.print_report(), &sample));
    }

    #[test]
    fn print_report_handles_partial_wind_data() {
        let mut sample = full_sample();
        sample.wind_speed = Some(5.0);
        sample.wind_direction = None;
        assert!(std::ptr::eq(sample.print_report(), &sample));
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

        assert_eq!(sample.terrestrial_date, NaiveDate::from_ymd_opt(2025, 9, 10));
        assert_eq!(sample.sol, 100);
        assert_eq!(sample.pressure, Some(750.0));
        assert_eq!(sample.pressure_change_direction, Some(PressureDirection::Rising));
        assert_eq!(sample.mars_season, Some(5));
        assert_eq!(sample.abs_humidity, Some(0.3));
        assert_eq!(sample.wind_speed, Some(12.0));
        assert_eq!(sample.wind_direction, Some(WindDirection::NW));
        assert_eq!(sample.atmo_opacity, Some(AtmoOpacity::Sunny));
        assert_eq!(sample.uv_index, Some(UvIndex::Moderate));
        assert_eq!(sample.season, Some(ArrayString::from("month 5").unwrap()));
        assert_eq!(sample.sunrise, NaiveTime::from_hms_opt(6, 0, 0));
        assert_eq!(sample.sunset, NaiveTime::from_hms_opt(18, 0, 0));
        match &sample.min_temp {
            Some(temp) => assert_eq!(temp.celsius, -80.0),
            None => panic!("min_temp should be populated"),
        }
        match &sample.max_temp {
            Some(temp) => assert_eq!(temp.celsius, -10.0),
            None => panic!("max_temp should be populated"),
        }
        match &sample.min_ground_temp {
            Some(temp) => assert_eq!(temp.celsius, -96.0),
            None => panic!("min_ground_temp should be populated"),
        }
        match &sample.max_ground_temp {
            Some(temp) => assert_eq!(temp.celsius, 10.0),
            None => panic!("max_ground_temp should be populated"),
        }
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
        assert!(sample.mars_season.is_none());
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
    fn parse_unavailable_wind_direction_maps_to_unknown() {
        let report = json!({ "wind_direction": "--" });
        let sample = WeatherSample::parse(&report);
        assert_eq!(sample.wind_direction, Some(WindDirection::Unknown));
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
}
