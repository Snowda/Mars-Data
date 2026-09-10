#![cfg(feature = "integration_tests")]

use std::io;
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::time::Duration;

use mars_data::mars::api::serve;
use mars_data::mars::config::{Config, ServerConfig, WeatherConfig};
use mars_data::mars::weather::WeatherSample;
use reqwest::StatusCode;

const HEALTH_POLL_ATTEMPTS: u32 = 20;
const WEATHER_POLL_ATTEMPTS: u32 = 30;
const POLL_INTERVAL_MS: u64 = 500;

// Reserves an OS-assigned free port, then releases it so the server can bind it.
fn free_port() -> io::Result<u16> {
    let listener: TcpListener = TcpListener::bind("127.0.0.1:0")?;
    return Ok(listener.local_addr()?.port());
}

const fn test_config(port: u16) -> Config {
    return Config {
        server: ServerConfig {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
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
async fn serve_exposes_health_and_weather_endpoints() -> Result<(), String> {
    // The client below builds before the server's refresh loop installs the
    // process-global provider, so install it here to avoid the startup race.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let port: u16 = free_port().map_err(|e| format!("failed to reserve a free port: {}", e))?;
    tokio::spawn(serve(test_config(port)));

    let health_url: String = format!("http://127.0.0.1:{}/health", port);
    let health: Option<reqwest::Response> = poll_until_ok(&health_url, HEALTH_POLL_ATTEMPTS).await;
    assert!(health.is_some(), "health endpoint never returned 200");

    let weather_url: String = format!("http://127.0.0.1:{}/weather", port);
    let weather: Option<reqwest::Response> = poll_until_ok(&weather_url, WEATHER_POLL_ATTEMPTS).await;
    let response: reqwest::Response = weather.ok_or_else(|| "weather endpoint never returned 200".to_owned())?;

    let body: WeatherSample = response.json().await.map_err(|e| format!("weather body should deserialize: {}", e))?;
    let encoded: String = serde_json::to_string(&body).map_err(|e| format!("weather sample should serialize: {}", e))?;
    let decoded: WeatherSample = serde_json::from_str(&encoded).map_err(|e| format!("weather sample should deserialize: {}", e))?;
    assert_eq!(decoded, body);
    return Ok(());
}
