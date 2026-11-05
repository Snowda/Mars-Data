// Fetches the configured MSL feed over HTTP and decodes its latest sol.
use std::time::Duration;

use http_body_util::{BodyExt, Empty, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::{Response, StatusCode, Uri};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use rustls::crypto::aws_lc_rs;
use tokio::time;

use super::FetchError;
use super::WeatherSample;
use super::parse_weather_body;
use crate::mars::config::WeatherConfig;

// The MSL feed is a few hundred KiB; cap the accepted body well above that so a
// hostile or compromised upstream cannot exhaust memory with an unbounded body.
const MAX_RESPONSE_BODY_BYTES: usize = 8 * 1024 * 1024;

type HttpsClient = Client<HttpsConnector<HttpConnector>, Empty<Bytes>>;

pub async fn load_weather_data(config: &WeatherConfig) -> Result<WeatherSample, FetchError> {
    let uri: Uri = config.url().parse().map_err(FetchError::InvalidUri)?;
    let client: HttpsClient = build_client(config.connect_timeout_secs.as_duration())?;
    let timeout: Duration = config.request_timeout_secs.as_duration();

    // TODO poll instead of await
    let body: Bytes = match time::timeout(timeout, fetch_body(&client, uri)).await {
        Ok(result) => result?,
        Err(_elapsed) => return Err(FetchError::Timeout(timeout)),
    };
    return parse_weather_body(&body);
}

fn build_client(connect_timeout: Duration) -> Result<HttpsClient, FetchError> {
    let mut http: HttpConnector = HttpConnector::new();
    // HttpsConnector routes https itself; the inner connector must accept every scheme.
    http.enforce_http(false);
    http.set_connect_timeout(Some(connect_timeout));
    let https: HttpsConnector<HttpConnector> = HttpsConnectorBuilder::new()
        .with_provider_and_webpki_roots(aws_lc_rs::default_provider())
        .map_err(FetchError::ClientBuild)?
        .https_or_http()
        .enable_http1()
        .wrap_connector(http);
    return Ok(Client::builder(TokioExecutor::new()).build(https));
}

async fn fetch_body(client: &HttpsClient, uri: Uri) -> Result<Bytes, FetchError> {
    let response: Response<Incoming> = client.get(uri).await.map_err(FetchError::Request)?;
    let status: StatusCode = response.status();
    if status != StatusCode::OK {
        return Err(FetchError::Status(status));
    }
    return read_capped_body(response.into_body()).await;
}

// Keeps hyper's frames as they arrive and stops at the first one past the cap; a body in one frame is not copied.
async fn read_capped_body(incoming: Incoming) -> Result<Bytes, FetchError> {
    return match Limited::new(incoming, MAX_RESPONSE_BODY_BYTES).collect().await {
        Ok(body) => Ok(body.to_bytes()),
        // Limited raises only its inner body's hyper::Error or its own LengthLimitError.
        Err(e) => Err(match e.downcast::<hyper::Error>() {
            Ok(cause) => FetchError::BodyRead(*cause),
            Err(_length_limit) => FetchError::BodyTooLarge { cap_bytes: MAX_RESPONSE_BODY_BYTES },
        }),
    };
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::io;
    use std::time::Duration;

    use arrayvec::ArrayString;
    use axum::Router;
    use axum::http::StatusCode;
    use axum::routing::get;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use url::Url;

    use super::FetchError;
    use super::MAX_RESPONSE_BODY_BYTES;
    use super::WeatherSample;
    use super::load_weather_data;
    use crate::mars::config::{Seconds, WeatherConfig};
    use crate::mars::test_support::{MAX_MESSAGE_BYTES, TestError, display};
    use crate::mars::weather::WindDirection;

    // An old sol the live feed no longer reports first, so a match proves the stub was fetched.
    const STUB_SOL: u32 = 4083;
    const STUB_PATH: &str = "/stub-feed";
    const STUB_BODY: &[u8] = br#"{"soles":[{"sol":"4083","wind_direction":"SE"}]}"#;
    const TOO_LARGE_PATH: &str = "/too-large";
    const OVERSIZED_PATH: &str = "/oversized";
    const AT_CAP_PATH: &str = "/at-cap";
    // Promises ten body bytes, sends two, then closes.
    const CUT_OFF_REPLY: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nab";
    const SHORT_TIMEOUT: Seconds = Seconds::new(1).expect("non-zero literal");
    const SILENT_HOLD: Duration = Duration::from_secs(5);
    const RAW_REQUEST_BUFFER: usize = 4096;

    // "http://" + "[ffff:...:ffff]:65535" + the longest path above, with room to spare.
    const MAX_URL_BYTES: usize = 128;

    // Serves STUB_BODY at STUB_PATH, a 413 at TOO_LARGE_PATH, and a body one byte over the cap at OVERSIZED_PATH.
    async fn serve_stub() -> Result<SocketAddr, io::Error> {
        let listener: TcpListener = TcpListener::bind("127.0.0.1:0").await?;
        let addr: SocketAddr = listener.local_addr()?;
        let app: Router = Router::new()
            .route(STUB_PATH, get(|| async { STUB_BODY }))
            .route(TOO_LARGE_PATH, get(|| async { StatusCode::PAYLOAD_TOO_LARGE }))
            .route(OVERSIZED_PATH, get(|| async { vec![b'x'; MAX_RESPONSE_BODY_BYTES + 1] }))
            .route(AT_CAP_PATH, get(|| async { vec![b'x'; MAX_RESPONSE_BODY_BYTES] }));
        tokio::spawn(async move { axum::serve(listener, app).await });
        return Ok(addr);
    }

    // Answers one connection with `reply`, holds it open for `hold`, then closes it.
    async fn serve_raw(reply: &'static [u8], hold: Duration) -> Result<SocketAddr, io::Error> {
        let listener: TcpListener = TcpListener::bind("127.0.0.1:0").await?;
        let addr: SocketAddr = listener.local_addr()?;
        tokio::spawn(async move {
            let Ok((mut stream, _peer)) = listener.accept().await else {
                return;
            };
            // hyper refuses a reply that arrives before its request is written.
            let mut request: [u8; RAW_REQUEST_BUFFER] = [0; RAW_REQUEST_BUFFER];
            let _read = stream.read(&mut request).await;
            let _sent = stream.write_all(reply).await;
            tokio::time::sleep(hold).await;
        });
        return Ok(addr);
    }

    // The URL text is built on the stack; only the parsed Url owns heap memory.
    fn config_for(addr: SocketAddr, path: &str) -> Result<WeatherConfig, url::ParseError> {
        let text: ArrayString<MAX_URL_BYTES> = display(&format_args!("http://{addr}{path}"));
        let url: Url = Url::parse(&text)?;
        return Ok(WeatherConfig { url: Some(url), ..WeatherConfig::default() });
    }

    fn message_of(result: Result<WeatherSample, FetchError>) -> ArrayString<MAX_MESSAGE_BYTES> {
        return result.err().map(|e: FetchError| display(&e)).unwrap_or_default();
    }

    #[tokio::test]
    async fn load_weather_data_fetches_the_configured_url() -> Result<(), TestError> {
        let addr: SocketAddr = serve_stub().await?;
        let config: WeatherConfig = config_for(addr, STUB_PATH)?;
        let sample: WeatherSample = load_weather_data(&config).await?;
        assert_eq!(sample.sol, STUB_SOL, "the sample must come from the stub, not the default feed");
        assert_eq!(sample.wind_direction, Some(WindDirection::SE));
        return Ok(());
    }

    #[tokio::test]
    async fn load_weather_data_requests_the_configured_path() -> Result<(), TestError> {
        // Same host, other path: a 404 proves the path part of the URL is used too.
        let addr: SocketAddr = serve_stub().await?;
        let config: WeatherConfig = config_for(addr, "/elsewhere")?;
        let result: Result<WeatherSample, FetchError> = load_weather_data(&config).await;
        assert!(matches!(result, Err(FetchError::Status(StatusCode::NOT_FOUND))), "got {result:?}");
        return Ok(());
    }

    #[tokio::test]
    async fn load_weather_data_reports_payload_too_large() -> Result<(), TestError> {
        let addr: SocketAddr = serve_stub().await?;
        let config: WeatherConfig = config_for(addr, TOO_LARGE_PATH)?;
        let result: Result<WeatherSample, FetchError> = load_weather_data(&config).await;
        assert!(matches!(result, Err(FetchError::Status(StatusCode::PAYLOAD_TOO_LARGE))), "got {result:?}");
        return Ok(());
    }

    #[tokio::test]
    async fn load_weather_data_stops_reading_past_the_body_cap() -> Result<(), TestError> {
        let addr: SocketAddr = serve_stub().await?;
        let config: WeatherConfig = config_for(addr, OVERSIZED_PATH)?;
        let result: Result<WeatherSample, FetchError> = load_weather_data(&config).await;
        assert!(
            matches!(result, Err(FetchError::BodyTooLarge { cap_bytes: MAX_RESPONSE_BODY_BYTES })),
            "one byte over the cap must be refused, got {result:?}"
        );
        return Ok(());
    }

    #[tokio::test]
    async fn load_weather_data_reads_a_body_exactly_at_the_cap() -> Result<(), TestError> {
        let addr: SocketAddr = serve_stub().await?;
        let config: WeatherConfig = config_for(addr, AT_CAP_PATH)?;
        let result: Result<WeatherSample, FetchError> = load_weather_data(&config).await;
        assert!(matches!(result, Err(FetchError::InvalidJson(_))), "a body at the cap must reach the decoder, got {result:?}");
        return Ok(());
    }

    #[tokio::test]
    async fn load_weather_data_reports_a_refused_connection() -> Result<(), TestError> {
        // Bind then drop a listener so the port is known to have nothing behind it.
        let closed: TcpListener = TcpListener::bind("127.0.0.1:0").await?;
        let addr: SocketAddr = closed.local_addr()?;
        drop(closed);
        let config: WeatherConfig = config_for(addr, STUB_PATH)?;
        let result: Result<WeatherSample, FetchError> = load_weather_data(&config).await;
        assert!(matches!(result, Err(FetchError::Request(_))), "got {result:?}");
        let message: ArrayString<MAX_MESSAGE_BYTES> = message_of(result);
        assert!(message.starts_with("Request error: "), "message must keep its former prefix, got {message}");
        return Ok(());
    }

    #[tokio::test]
    async fn load_weather_data_reports_a_cut_off_body() -> Result<(), TestError> {
        let addr: SocketAddr = serve_raw(CUT_OFF_REPLY, Duration::ZERO).await?;
        let config: WeatherConfig = config_for(addr, STUB_PATH)?;
        let result: Result<WeatherSample, FetchError> = load_weather_data(&config).await;
        assert!(matches!(result, Err(FetchError::BodyRead(_))), "got {result:?}");
        let message: ArrayString<MAX_MESSAGE_BYTES> = message_of(result);
        assert!(message.starts_with("Failed to read response body: "), "message must keep its former prefix, got {message}");
        return Ok(());
    }

    #[tokio::test]
    async fn load_weather_data_gives_up_on_a_silent_peer() -> Result<(), TestError> {
        let addr: SocketAddr = serve_raw(b"", SILENT_HOLD).await?;
        let mut config: WeatherConfig = config_for(addr, STUB_PATH)?;
        config.request_timeout_secs = SHORT_TIMEOUT;
        let result: Result<WeatherSample, FetchError> = load_weather_data(&config).await;
        assert!(
            matches!(result, Err(FetchError::Timeout(timeout)) if timeout == SHORT_TIMEOUT.as_duration()),
            "the request timeout must bound the whole exchange, got {result:?}"
        );
        return Ok(());
    }
}
