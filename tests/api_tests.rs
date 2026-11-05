#![cfg(feature = "integration_tests")]

use std::fmt::Write;
use std::io;
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::time::Duration;

use arrayvec::{ArrayString, ArrayVec};
use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper::{StatusCode, Uri};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use mars_data::mars::api::serve;
use mars_data::mars::config::{Config, Seconds, ServerConfig, WeatherConfig};
use mars_data::mars::weather::{MAX_SAMPLE_JSON_BYTES, WeatherSample};

// "http://127.0.0.1:65535/weather" is 30 bytes.
const MAX_URL_BYTES: usize = 64;
const HEALTH_POLL_ATTEMPTS: u32 = 20;
const WEATHER_POLL_ATTEMPTS: u32 = 30;
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const REQUEST_TIMEOUT: Seconds = Seconds::new(10).expect("non-zero literal");
const CONNECT_TIMEOUT: Seconds = Seconds::new(5).expect("non-zero literal");

// Reserves an OS-assigned free port, then releases it so the server can bind it.
fn free_port() -> io::Result<u16> {
    let listener: TcpListener = TcpListener::bind("127.0.0.1:0")?;
    return Ok(listener.local_addr()?.port());
}

fn test_config(port: u16) -> Config {
    return Config {
        server: ServerConfig { host: IpAddr::V4(Ipv4Addr::LOCALHOST), port, ..ServerConfig::default() },
        weather: WeatherConfig {
            request_timeout_secs: REQUEST_TIMEOUT,
            connect_timeout_secs: CONNECT_TIMEOUT,
            ..WeatherConfig::default()
        },
    };
}

fn local_url(port: u16, path: &str) -> ArrayString<MAX_URL_BYTES> {
    let mut url: ArrayString<MAX_URL_BYTES> = ArrayString::new();
    if write!(url, "http://127.0.0.1:{port}{path}").is_err() {
        panic!("a loopback URL with a short path fits {MAX_URL_BYTES} bytes");
    }
    return url;
}

// The body of the first 200 reply, or None if every attempt failed.
async fn poll_until_ok(url: &str, attempts: u32) -> Option<Bytes> {
    let client: Client<HttpConnector, Empty<Bytes>> = Client::builder(TokioExecutor::new()).build_http();
    let uri: Uri = url.parse().ok()?;
    for _ in 0..attempts {
        if let Ok(response) = client.get(uri.clone()).await
            && response.status() == StatusCode::OK
            && let Ok(collected) = response.into_body().collect().await
        {
            return Some(collected.to_bytes());
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    return None;
}

#[tokio::test]
// serde_json::Error converts into io::Error, so one io::Result carries both failure kinds.
async fn serve_exposes_health_and_weather_endpoints() -> io::Result<()> {
    let port: u16 = free_port()?;
    tokio::spawn(serve(test_config(port)));

    let health_url: ArrayString<MAX_URL_BYTES> = local_url(port, "/health");
    let health: Option<Bytes> = poll_until_ok(&health_url, HEALTH_POLL_ATTEMPTS).await;
    assert!(health.is_some(), "health endpoint never returned 200");

    let weather_url: ArrayString<MAX_URL_BYTES> = local_url(port, "/weather");
    let weather: Option<Bytes> = poll_until_ok(&weather_url, WEATHER_POLL_ATTEMPTS).await;
    let Some(response) = weather else {
        panic!("weather endpoint never returned 200");
    };

    let body: WeatherSample = serde_json::from_slice(&response)?;
    let mut encoded: ArrayVec<u8, MAX_SAMPLE_JSON_BYTES> = ArrayVec::new();
    serde_json::to_writer(&mut encoded, &body)?;
    let decoded: WeatherSample = serde_json::from_slice(&encoded)?;
    assert_eq!(decoded, body, "the served sample must survive a JSON round trip");
    return Ok(());
}
