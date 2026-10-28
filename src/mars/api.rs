use std::net::SocketAddr;
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tracing::{error, info, warn};

use crate::mars::config::{Config, WeatherConfig};
use crate::mars::weather::{WeatherSample, load_weather_data};

// Single writer (the refresher task) publishes the latest sample; every handler
// holds a cheap Clone receiver and reads the current value without locking.
type WeatherReceiver = watch::Receiver<Option<WeatherSample>>;

// Routes and the 503 body are documented in openapi.json; tests below keep the two in sync.
const HEALTH_PATH: &str = "/health";
const WEATHER_PATH: &str = "/weather";
const NOT_READY_MESSAGE: &str = "Weather data not yet available";

pub async fn serve(config: Config) -> Result<(), String> {
    let bind_addr: SocketAddr = config.server.bind_addr();

    if bind_addr.ip().is_unspecified() {
        warn!(
            "Binding {} exposes the unauthenticated API on all network interfaces; \
             set server.host to 127.0.0.1 to restrict to loopback",
            bind_addr
        );
    }

    let refresh_interval: Duration = config.server.refresh_interval_secs.as_duration();
    let weather_config: WeatherConfig = config.weather;

    let (tx, rx) = watch::channel::<Option<WeatherSample>>(None);

    tokio::spawn(refresh_loop(tx, weather_config, refresh_interval));

    let app: Router = Router::new()
        .route(HEALTH_PATH, get(health))
        .route(WEATHER_PATH, get(weather))
        .with_state(rx);

    let listener: TcpListener = match TcpListener::bind(bind_addr).await {
        Ok(bound) => bound,
        Err(e) => return Err(format!("Failed to bind {}: {}", bind_addr, e)),
    };

    info!("Serving Mars weather API on {}", bind_addr);

    return match axum::serve(listener, app).await {
        Ok(()) => Ok(()),
        Err(e) => Err(format!("Server error: {}", e)),
    };
}

// Refreshes the published sample on a fixed interval; the first fetch runs
// immediately so the API has data shortly after startup.
async fn refresh_loop(tx: watch::Sender<Option<WeatherSample>>, config: WeatherConfig, interval: Duration) {
    loop {
        match load_weather_data(&config).await {
            Ok(sample) => {
                if tx.send(Some(sample)).is_err() {
                    return;
                }
            },
            Err(e) => error!("Failed to refresh weather data: {}", e),
        }

        tokio::time::sleep(interval).await;
    }
}

async fn health() -> StatusCode {
    return StatusCode::OK;
}

async fn weather(State(rx): State<WeatherReceiver>) -> Response {
    return match rx.borrow().clone() {
        Some(sample) => Json(sample).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, NOT_READY_MESSAGE).into_response(),
    };
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::net::IpAddr;

    use axum::body::{self, Bytes};
    use axum::extract::State;
    use axum::http::StatusCode;
    use serde_json::{Map, Value, json};
    use tokio::net::TcpListener;
    use tokio::sync::watch;

    use super::{HEALTH_PATH, NOT_READY_MESSAGE, WEATHER_PATH, health, serve, weather};
    use crate::mars::config::{Config, ServerConfig, WeatherConfig};
    use crate::mars::openapi_contract::{self, assert_conforms, assert_enum_matches, json_type};
    use crate::mars::weather::WeatherSample;

    const MAX_TEST_BODY_BYTES: usize = 1024;

    fn is_documented(spec: &Value, path: &str, status: StatusCode) -> bool {
        return spec["paths"][path]["get"]["responses"].get(status.as_str()).is_some();
    }

    #[test]
    fn spec_documents_exactly_the_served_routes() {
        let spec: Value = openapi_contract::spec();
        let mut paths: Vec<&str> =
            spec["paths"].as_object().map(|p| p.keys().map(String::as_str).collect()).unwrap_or_default();
        paths.sort_unstable();
        assert_eq!(paths, [HEALTH_PATH, WEATHER_PATH]);
    }

    #[tokio::test]
    async fn health_status_is_documented() {
        assert!(is_documented(&openapi_contract::spec(), HEALTH_PATH, health().await));
    }

    #[tokio::test]
    async fn weather_not_ready_status_and_body_are_documented() -> Result<(), axum::Error> {
        let spec: Value = openapi_contract::spec();
        let (_tx, rx) = watch::channel::<Option<WeatherSample>>(None);
        let response = weather(State(rx)).await;
        assert!(is_documented(&spec, WEATHER_PATH, response.status()));

        let body: Bytes = body::to_bytes(response.into_body(), MAX_TEST_BODY_BYTES).await?;
        let documented: &Value = &spec["paths"][WEATHER_PATH]["get"]["responses"]["503"]["content"]["text/plain"]["schema"]["const"];
        assert_eq!(documented.as_str(), Some(NOT_READY_MESSAGE));
        assert_eq!(body.as_ref(), NOT_READY_MESSAGE.as_bytes());
        return Ok(());
    }

    #[tokio::test]
    async fn weather_ok_status_is_documented() -> Result<(), serde_json::Error> {
        let (_tx, rx) = watch::channel::<Option<WeatherSample>>(Some(sample()?));
        let response = weather(State(rx)).await;
        assert!(is_documented(&openapi_contract::spec(), WEATHER_PATH, response.status()));
        return Ok(());
    }

    #[test]
    fn undocumented_status_is_detected() {
        assert!(!is_documented(&openapi_contract::spec(), WEATHER_PATH, StatusCode::INTERNAL_SERVER_ERROR));
        assert!(!is_documented(&openapi_contract::spec(), "/missing", StatusCode::OK));
    }

    // Hand-written so these tests exercise the contract checks without the serializer.
    fn contract_instance() -> Value {
        return json!({
            "terrestrial_date": "2025-09-10", "sol": 100, "min_temp": -80.0, "max_temp": null,
            "min_ground_temp": null, "max_ground_temp": null, "pressure": 750.0,
            "pressure_change_direction": "Rising", "mars_season": 5, "abs_humidity": null, "wind_speed": null,
            "wind_direction": "NW", "atmo_opacity": "Sunny", "uv_index": "Moderate", "season": "Month 5",
            "sunrise": "06:00:00", "sunset": "18:00:00"
        });
    }

    #[test]
    fn contract_accepts_a_conforming_instance() {
        assert_conforms(&openapi_contract::spec(), "WeatherSample", &contract_instance());
    }

    #[test]
    #[should_panic(expected = "serialized keys differ")]
    fn contract_rejects_a_payload_missing_a_field() {
        let mut payload: Value = contract_instance();
        if let Some(object) = payload.as_object_mut() {
            object.remove("sunset");
        }
        assert_conforms(&openapi_contract::spec(), "WeatherSample", &payload);
    }

    #[test]
    #[should_panic(expected = "serializer emits string")]
    fn contract_rejects_a_number_emitted_as_a_string() {
        let mut payload: Value = contract_instance();
        payload["sol"] = json!("100");
        assert_conforms(&openapi_contract::spec(), "WeatherSample", &payload);
    }

    #[test]
    #[should_panic(expected = "is not in the spec enum")]
    fn contract_rejects_an_undocumented_enum_value() {
        let mut payload: Value = contract_instance();
        payload["uv_index"] = json!("Very_High");
        assert_conforms(&openapi_contract::spec(), "WeatherSample", &payload);
    }

    #[test]
    #[should_panic(expected = "spec enum differs")]
    fn contract_rejects_an_enum_missing_a_variant() {
        assert_enum_matches(&openapi_contract::spec(), "PressureDirection", &["Rising"]);
    }

    #[test]
    fn contract_json_type_distinguishes_integers_from_floats() {
        let object: Map<String, Value> = Map::new();
        assert_eq!(json_type(&json!(1)), "integer");
        assert_eq!(json_type(&json!(1.5)), "number");
        assert_eq!(json_type(&json!(true)), "boolean");
        assert_eq!(json_type(&json!([])), "array");
        assert_eq!(json_type(&Value::Object(object)), "object");
    }

    fn sample() -> Result<WeatherSample, serde_json::Error> {
        let value = json!({
            "terrestrial_date": "2025-09-10",
            "sol": 100,
            "min_temp": -80.0,
            "max_temp": -10.0,
            "min_ground_temp": -96.0,
            "max_ground_temp": 10.0,
            "pressure": 750.0,
            "pressure_change_direction": "Rising",
            "mars_season": 5,
            "abs_humidity": null,
            "wind_speed": null,
            "wind_direction": "NW",
            "atmo_opacity": "Sunny",
            "uv_index": "Moderate",
            "season": "month 5",
            "sunrise": "06:00:00",
            "sunset": "18:00:00"
        });
        return serde_json::from_value(value);
    }

    #[tokio::test]
    async fn health_returns_ok() {
        assert_eq!(health().await, StatusCode::OK);
    }

    #[tokio::test]
    async fn weather_returns_service_unavailable_without_data() {
        let (_tx, rx) = watch::channel::<Option<WeatherSample>>(None);
        let response = weather(State(rx)).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn weather_returns_ok_with_data() -> Result<(), serde_json::Error> {
        let (_tx, rx) = watch::channel::<Option<WeatherSample>>(Some(sample()?));
        let response = weather(State(rx)).await;
        assert_eq!(response.status(), StatusCode::OK);
        return Ok(());
    }

    #[tokio::test]
    async fn serve_errors_when_address_in_use() -> Result<(), io::Error> {
        // Occupy a port, then ask the server to bind the same one so the bind
        // fails deterministically without depending on an external network.
        let occupied: TcpListener = TcpListener::bind("127.0.0.1:0").await?;
        let port: u16 = occupied.local_addr()?.port();

        let config: Config = Config {
            server: ServerConfig { host: IpAddr::from([127, 0, 0, 1]), port, ..ServerConfig::default() },
            weather: WeatherConfig::default(),
        };

        let result: Result<(), String> = serve(config).await;
        assert!(result.is_err());
        return Ok(());
    }
}
