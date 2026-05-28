#![cfg(feature = "integration_tests")]

use std::net::TcpListener;
use std::time::Duration;

use mars_data::mars::api::serve;
use mars_data::mars::config::{Config, ServerConfig, WeatherConfig};
use mars_data::mars::weather::WeatherSample;
use reqwest::StatusCode;

const HEALTH_POLL_ATTEMPTS: u32 = 20;
const WEATHER_POLL_ATTEMPTS: u32 = 30;
const POLL_INTERVAL_MS: u64 = 500;

// Reserves an OS-assigned free port, then releases it so the server can bind it.
fn free_port() -> u16 {
    let listener: TcpListener = TcpListener::bind("127.0.0.1:0").unwrap();
    return listener.local_addr().unwrap().port();
}

fn test_config(port: u16) -> Config {
    return Config {
        server: ServerConfig {
            host: "127.0.0.1".parse().unwrap(),
            port,
            refresh_interval_secs: 3600,
        },
        weather: WeatherConfig { request_timeout_secs: 10, connect_timeout_secs: 5 },
    };
}

async fn poll_until_ok(url: &str, attempts: u32) -> Option<reqwest::Response> {
    for _ in 0..attempts {
        if let Ok(response) = reqwest::get(url).await
            && response.status() == StatusCode::OK
        {
            return Some(response);
        }
        tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
    return None;
}

#[tokio::test]
async fn serve_exposes_health_and_weather_endpoints() {
    let port: u16 = free_port();
    tokio::spawn(serve(test_config(port)));

    let health_url: String = format!("http://127.0.0.1:{}/health", port);
    let health: Option<reqwest::Response> = poll_until_ok(&health_url, HEALTH_POLL_ATTEMPTS).await;
    assert!(health.is_some(), "health endpoint never returned 200");

    let weather_url: String = format!("http://127.0.0.1:{}/weather", port);
    let weather: Option<reqwest::Response> = poll_until_ok(&weather_url, WEATHER_POLL_ATTEMPTS).await;
    let response: reqwest::Response = weather.expect("weather endpoint never returned 200");

    let body: WeatherSample = response.json().await.expect("weather body should deserialize");
    let encoded: String = serde_json::to_string(&body).unwrap();
    let decoded: WeatherSample = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, body);
}
