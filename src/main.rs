
use std::time::Duration;

use serde::{Serialize, Deserialize};
use serde_json::value::Value;
use astro::consts::SPEED_OF_LIGHT;
use astro::planet::{geocent_apprnt_ecl_coords, Planet};
use astro::time::{Date, CalType, julian_day};
use chrono::{DateTime, Datelike, Utc};
use chrono::Duration as ChronoDuration;
use reqwest::{Client, get};
use reqwest::StatusCode;
use reqwest::Error as ReqError;

#[derive(Debug)]
struct RootData {
    sol_keys: Vec<String>,
    validatity_checks
}


#[tokio::main]
async fn main() {
    println!("{}", mars_comms_delay(None));
    println!("{}", mars_comms_return_delay(None));
    let DEMO_KEY = "DEMO_KEY"; // TODO replace
    let weather_url = format!("https://api.nasa.gov/insight_weather/?api_key={:?}&feedtype=json&ver=1.0", DEMO_KEY);

    let client_builder = reqwest::Client::builder()
        .gzip(true)
        .timeout(Duration::from_secs(2)) // 2 second timeout
        .build();
    // TODO better code style than match chain

    match client_builder {
        Ok(client) => {
            let resp = client.get(weather_url).send();

            match resp.await { // TODO poll instead of await
                Ok(response) => {
                    match response.status() { // TODO handle the result or break
                        StatusCode::OK => {
                            let json: Value = response.json().await.unwrap(); // TODO poll and remove unwrap
                            // TODO validate deserialisation
                            // TODO handle correct JSON format
                            // TODO handle non existing keys

                            match json.get("report") {
                                Some(report) => {
                                    let terrestrial_date = report.get("terrestrial_date");
                                    let sol = report.get("sol");
                                    let min_temp = report.get("min_temp");
                                    let max_temp = report.get("max_temp");
                                    let min_temp_fahrenheit = report.get("min_temp_fahrenheit");
                                    let max_temp_fahrenheit = report.get("max_temp_fahrenheit");
                                    let pressure = report.get("pressure");
                                    let pressure_string = report.get("pressure_string");
                                    let mars_season = report.get("ls");
                                    let abs_humidity = report.get("abs_humidity");
                                    let wind_speed = report.get("wind_speed");
                                    let wind_direction = report.get("wind_direction");
                                    let atmo_opacity = report.get("atmo_opacity");
                                    let season = report.get("season");
                                    let sunrise = report.get("sunrise");
                                    let sunset = report.get("sunset");

                                    println!("Date: {:?} (sol {:?})", terrestrial_date, sol);

                                    println!("Today's lowest temperature was {:?} C", min_temp);
                                    println!("Today's highest temperature was {:?} C", min_temp);

                                    let atmo_status = if str(pressure_string) == "Higher" {
                                        "rising"
                                    } else {
                                        "falling"
                                    };

                                    println!("Atmospheric pressure is {:?} and {:?}", pressure, atmo_status);
                                    println!("Mars season: {:?}", mars_season);

                                    if !abs_humidity.is_none() {
                                        println!("Humidity{:?} ", abs_humidity);
                                    }

                                    if !wind_speed.is_none() {
                                        println!("The wind is blowing {:?} at a speed of {:?}km/s", wind_direction, wind_speed);
                                    }
                                    println!("The weather is {:?}", atmo_opacity);
                                    println!("{:?}", season);
                                    println!("The sun rises at {:?}", sunrise);
                                    println!("The sun sets at {:?}", sunset);
                                },
                                None => {
                                    // TODO
                                },
                            }
                        },
                        // TODO handle more status codes
                        StatusCode::PAYLOAD_TOO_LARGE => {
                            // TODO handle status codes, eg 200. If not, quit early
                            // TODO better more comprehensive handling
                            return
                        },
                        s => {
                            // TODO handle status codes, eg 200. If not, quit early
                            // TODO better more comprehensive handling
                            return
                        }
                    };
                },
                ReqError => {
                    // TODO
                },
            }
            
        },
        Err(error) => {
            // TODO handle the error out correctly
        },
    };
}

fn mars_comms_delay(start_time: Option<DateTime<Utc>>) -> f64 {
    // Calculates the speed of light communications delay to mars
    let dt = match start_time {
        Some(time) => time,
        None => Utc::now(),
    };

    let (_is_common_era, year) = dt.year_ce();

    let gregorian = Date{year: year, month: dt.month(), decimal_day: dt.day(),
        cal_type: CalType::Gregorian};
    let julian_day = julian_day(&gregorian);
    let (_, rad_vec_mars) = geocent_apprnt_ecl_coords(&Planet::Mars, julian_day);

    let microseconds = rad_vec_mars / SPEED_OF_LIGHT;
    return microseconds;
}

fn mars_comms_return_delay(start_time: Option<DateTime<Utc>>) -> f64 { // TODO different return type
    // Calculates the speed of light communications delay to mars
    let dt: DateTime<Utc> = match start_time {
        Some(time) => time,
        None => Utc::now(),
    };

    let microseconds = mars_comms_delay(None);
    // Return trip from time of initial arrival
    return mars_comms_delay(Some(dt + ChronoDuration::microseconds(microseconds))); // TODO apply the microsecond difference to the start time and calculate again
}
