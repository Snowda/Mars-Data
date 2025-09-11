use std::fmt::Display;
use std::time::Duration;

use serde_json::value::Value;
use reqwest::{StatusCode};

#[derive(Clone, Debug)]
struct Temperature {
    celsius: f64,
    fahrenheit: f64,
}

impl Temperature {
    fn new(celsius: f64, fahrenheit: f64) -> Self {
        Temperature { celsius, fahrenheit }
    }
}

impl Display for Temperature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} °C ({} °F)", self.celsius, self.fahrenheit)
    }
}

#[derive(Clone, Debug)]
pub struct WeatherSample {
    terrestrial_date: String, // TODO change data type
    sol: i32, // TODO change data type
    min_temp: Option<Temperature>,
    max_temp: Option<Temperature>,
    pressure: Option<f64>,
    pressure_change_direction: Option<i8>,
    mars_season: Option<i32>,
    abs_humidity: Option<f64>,
    wind_speed: Option<f64>,
    wind_direction: Option<String>,
    atmo_opacity: Option<String>,
    season: Option<String>,
    sunrise: Option<String>,
    sunset: Option<String>,
}

impl WeatherSample {
    pub fn print_report(&self) -> &Self { // TODO metric or fahrenheit as input preference
        println!("Date: {:?} (sol {:?})", self.terrestrial_date, self.sol);

        match &self.min_temp {
            Some(temp) => println!("Today's lowest temperature was {}", temp),
            None => println!("No minimum temperature data available"),
        }

        match &self.max_temp {
            Some(temp) => println!("Today's highest temperature was {}", temp),
            None => println!("No maximum temperature data available"),
        }

        match &self.pressure_change_direction {
            Some(direction) => {
                if direction > &0 {
                    println!("Atmospheric pressure is rising.");
                } else if direction < &0 {
                    println!("Atmospheric pressure is falling.");
                } else {
                    println!("Atmospheric pressure is stable.");
                }
            },
            None => println!("No pressure change direction data available"),
        }

        println!("Atmospheric pressure is {:?} and {:?}", self.pressure, self.pressure_change_direction);
        println!("Mars season: {:?}", self.mars_season);

        match &self.abs_humidity {
            Some(humidity) => println!("Humidity: {} ", humidity),
            None => println!("No Humidity Available")
        };

        match (&self.wind_speed, &self.wind_direction) {
            (Some(speed), Some(direction)) => println!("The wind is blowing {} at a speed of {} km/s", direction, speed),
            _ => println!("No wind data available"),
        }

        println!("The weather is {:?}", self.atmo_opacity);
        println!("{:?}", self.season);
        println!("The sun rises at {:?}", self.sunrise);
        println!("The sun sets at {:?}", self.sunset);
        return self;
    }

    fn parse(report: &Value) -> Self {
        let terrestrial_date: Option<&Value> = report.get("terrestrial_date"); // TODO break early if terrestrial_date is None
        let sol: Option<&Value> = report.get("sol"); // TODO break early if sol is None
        let min_c: Option<&Value> = report.get("min_temp");
        let max_c: Option<&Value> = report.get("max_temp");
        let min_f: Option<&Value> = report.get("min_temp_fahrenheit");
        let max_f: Option<&Value> = report.get("max_temp_fahrenheit");

        let min_temp: Option<Temperature> = match (min_c, min_f) {
            (Some(m_c), Some(m_f)) => Some(Temperature::new(m_c.as_f64().unwrap(), m_f.as_f64().unwrap())),
            _ => None
        };
        let max_temp: Option<Temperature> = match (max_c, max_f) {
            (Some(m_c), Some(m_f)) => Some(Temperature::new(m_c.as_f64().unwrap(), m_f.as_f64().unwrap())),
            _ => None
        };

        let pressure: Option<&Value> = report.get("pressure");
        let pressure_string: Option<&Value> = report.get("pressure_string");
        let mars_season: Option<&Value> = report.get("ls");
        let abs_humidity: Option<&Value> = report.get("abs_humidity");
        let wind_speed: Option<&Value> = report.get("wind_speed");
        let wind_direction: Option<&Value> = report.get("wind_direction");
        let atmo_opacity: Option<&Value> = report.get("atmo_opacity");
        let season: Option<&Value> = report.get("season");
        let sunrise: Option<&Value> = report.get("sunrise");
        let sunset: Option<&Value> = report.get("sunset");

        let atmo_status: Option<i8> = match pressure_string {
            Some(pressure_str) => {
                if pressure_str == "Higher" {
                    Some(1)
                } else {
                    Some(-1)
                }
            },
            None => None
        };

        return Self {
            terrestrial_date: terrestrial_date.map_or("Unknown".to_string(), |v| v.as_str().unwrap_or("Unknown").to_string()),
            sol: sol.map_or(0, |v| v.as_u64().unwrap_or(0) as i32),
            min_temp,
            max_temp,
            pressure: pressure.map(|v| v.as_f64().unwrap_or(0.0)),
            pressure_change_direction: atmo_status,
            mars_season: mars_season.map(|v| v.as_i64().unwrap_or(0) as i32),
            abs_humidity: abs_humidity.map(|v| v.as_f64().unwrap_or(0.0)),
            wind_speed: wind_speed.map(|v| v.as_f64().unwrap_or(0.0)),
            wind_direction: wind_direction.map(|v| v.as_str().unwrap_or("Unknown").to_string()),
            atmo_opacity: atmo_opacity.map(|v| v.as_str().unwrap_or("Unknown").to_string()),
            season: season.map(|v| v.as_str().unwrap_or("Unknown").to_string()),
            sunrise: sunrise.map(|v| v.as_str().unwrap_or("Unknown").to_string()),
            sunset: sunset.map(|v| v.as_str().unwrap_or("Unknown").to_string()),
        };
    }
}

pub async fn load_weather_data(demo_key: &str) -> Result<WeatherSample, String> { // TODO typed error codes
    let weather_url: String = format!("https://api.nasa.gov/insight_weather/?api_key={:?}&feedtype=json&ver=1.0", demo_key);

    let client_builder = reqwest::Client::builder()
        .gzip(true)
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(3))
        .build();
   
    return match client_builder {  // TODO better code style than match chain
        Ok(client) => {
            let resp = client.get(weather_url).send();

            match resp.await { // TODO poll instead of await
                Ok(response) => {
                    match response.status() {
                        StatusCode::OK => {
                            let json: Value = match response.json().await { // TODO replace await with polling
                                Ok(parsed_json) => parsed_json,
                                Err(e) => return Err(format!("Failed to parse JSON response: {}", e))
                            };
                            // TODO validate deserialisation
                            // TODO handle correct JSON format
                            // TODO handle non existing keys

                            match json.get("report") {
                                Some(report) => {
                                    Ok(WeatherSample::parse(report))
                                },
                                None => Err("Report data not found in response".to_string()),
                            }
                        },
                        // TODO handle more status codes
                        StatusCode::PAYLOAD_TOO_LARGE => {
                            // TODO handle status codes, eg 200. If not, quit early
                            // TODO better more comprehensive handling
                            Err(format!("Payload too large: Status code {}", StatusCode::PAYLOAD_TOO_LARGE))
                        },
                        s => {
                            // TODO handle status codes, eg 200. If not, quit early
                            // TODO better more comprehensive handling
                            Err(format!("Unexpected status code: {}", s))
                        }
                    }
                },
                Err(error) => Err(format!("Request error: {}", error)),
            }
        },
        Err(error) => {
            Err(format!("Failed to build HTTP client: {}", error))
        },
    };
}