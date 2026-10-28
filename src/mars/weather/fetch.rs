// Fetches the configured MSL feed over HTTP and decodes its latest sol.
use std::sync::Once;

use reqwest::StatusCode;
use rustls::crypto::aws_lc_rs;

use super::WeatherSample;
use super::parse_weather_body;
use crate::mars::config::WeatherConfig;

// The MSL feed is a few hundred KiB; cap the accepted body well above that so a
// hostile or compromised upstream cannot exhaust memory with an unbounded body.
// The cap is on decompressed bytes, so it also bounds gzip-bomb expansion.
const MAX_RESPONSE_BODY_BYTES: usize = 8 * 1024 * 1024;

static INSTALL_CRYPTO_PROVIDER: Once = Once::new();

// reqwest (rustls-no-provider) resolves TLS via the process-default CryptoProvider.
// Install aws-lc-rs once; Err means one is already installed, which is fine.
fn ensure_crypto_provider() {
    INSTALL_CRYPTO_PROVIDER.call_once(|| {
        let _ = aws_lc_rs::default_provider().install_default();
    });
}

pub async fn load_weather_data(config: &WeatherConfig) -> Result<WeatherSample, String> { // TODO typed error codes
    ensure_crypto_provider();

    let client_builder = reqwest::Client::builder()
        .gzip(true)
        .timeout(config.request_timeout_secs.as_duration())
        .connect_timeout(config.connect_timeout_secs.as_duration())
        .build();

    return match client_builder { // TODO better code style than match chain
        Ok(client) => {
            let resp = client.get(config.url()).send();

            match resp.await { // TODO poll instead of await
                Ok(mut response) => {
                    match response.status() {
                        StatusCode::OK => {
                            // Read the (decompressed) body chunk-by-chunk with a hard
                            // cap so an oversized response cannot exhaust memory.
                            let mut body: Vec<u8> = Vec::new();
                            while let Some(chunk) =
                                response.chunk().await.map_err(|e| format!("Failed to read response body: {}", e))?
                            {
                                if body.len() + chunk.len() > MAX_RESPONSE_BODY_BYTES {
                                    return Err(format!("Response body exceeded the {}-byte cap", MAX_RESPONSE_BODY_BYTES));
                                }
                                body.extend_from_slice(chunk.as_ref());
                            }

                            parse_weather_body(&body)
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
    use std::net::SocketAddr;

    use axum::Router;
    use axum::http::StatusCode;
    use axum::routing::get;
    use tokio::net::TcpListener;
    use url::Url;

    use super::MAX_RESPONSE_BODY_BYTES;
    use super::WeatherSample;
    use super::load_weather_data;
    use crate::mars::config::WeatherConfig;
    use crate::mars::weather::WindDirection;

    // An old sol the live feed no longer reports first, so a match proves the stub was fetched.
    const STUB_SOL: u32 = 4083;
    const STUB_PATH: &str = "/stub-feed";
    const STUB_BODY: &[u8] = br#"{"soles":[{"sol":"4083","wind_direction":"SE"}]}"#;
    const TOO_LARGE_PATH: &str = "/too-large";
    const OVERSIZED_PATH: &str = "/oversized";

    // Serves STUB_BODY at STUB_PATH, a 413 at TOO_LARGE_PATH, and a body one byte over the cap at OVERSIZED_PATH.
    async fn serve_stub() -> Result<SocketAddr, String> {
        let listener: TcpListener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
        let addr: SocketAddr = listener.local_addr().map_err(|e| e.to_string())?;
        let app: Router = Router::new()
            .route(STUB_PATH, get(|| async { STUB_BODY }))
            .route(TOO_LARGE_PATH, get(|| async { StatusCode::PAYLOAD_TOO_LARGE }))
            .route(OVERSIZED_PATH, get(|| async { vec![b'x'; MAX_RESPONSE_BODY_BYTES + 1] }));
        tokio::spawn(async move { axum::serve(listener, app).await });
        return Ok(addr);
    }

    fn config_for(url: &str) -> Result<WeatherConfig, String> {
        let url: Url = Url::parse(url).map_err(|e| e.to_string())?;
        return Ok(WeatherConfig { url: Some(url), ..WeatherConfig::default() });
    }

    #[tokio::test]
    async fn load_weather_data_fetches_the_configured_url() -> Result<(), String> {
        let addr: SocketAddr = serve_stub().await?;
        let config: WeatherConfig = config_for(&format!("http://{addr}{STUB_PATH}"))?;
        let sample: WeatherSample = load_weather_data(&config).await?;
        assert_eq!(sample.sol, STUB_SOL, "the sample must come from the stub, not the default feed");
        assert_eq!(sample.wind_direction, Some(WindDirection::SE));
        return Ok(());
    }

    #[tokio::test]
    async fn load_weather_data_requests_the_configured_path() -> Result<(), String> {
        // Same host, other path: a 404 proves the path part of the URL is used too.
        let addr: SocketAddr = serve_stub().await?;
        let config: WeatherConfig = config_for(&format!("http://{addr}/elsewhere"))?;
        assert_eq!(load_weather_data(&config).await, Err("Unexpected status code: 404 Not Found".to_owned()));
        return Ok(());
    }

    #[tokio::test]
    async fn load_weather_data_reports_payload_too_large() -> Result<(), String> {
        let addr: SocketAddr = serve_stub().await?;
        let config: WeatherConfig = config_for(&format!("http://{addr}{TOO_LARGE_PATH}"))?;
        let expected: String = "Payload too large: Status code 413 Payload Too Large".to_owned();
        assert_eq!(load_weather_data(&config).await, Err(expected));
        return Ok(());
    }

    #[tokio::test]
    async fn load_weather_data_stops_reading_past_the_body_cap() -> Result<(), String> {
        let addr: SocketAddr = serve_stub().await?;
        let config: WeatherConfig = config_for(&format!("http://{addr}{OVERSIZED_PATH}"))?;
        let expected: String = format!("Response body exceeded the {MAX_RESPONSE_BODY_BYTES}-byte cap");
        assert_eq!(load_weather_data(&config).await, Err(expected), "one byte over the cap must be refused");
        return Ok(());
    }

    #[tokio::test]
    async fn load_weather_data_reports_a_refused_connection() -> Result<(), String> {
        // Bind then drop a listener so the port is known to have nothing behind it.
        let closed: TcpListener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
        let addr: SocketAddr = closed.local_addr().map_err(|e| e.to_string())?;
        drop(closed);
        let config: WeatherConfig = config_for(&format!("http://{addr}{STUB_PATH}"))?;
        let result: Result<WeatherSample, String> = load_weather_data(&config).await;
        assert!(result.as_ref().is_err_and(|e| e.starts_with("Request error")), "got {result:?}");
        return Ok(());
    }
}
