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
use tracing::{error, info};

use crate::mars::config::{Config, WeatherConfig};
use crate::mars::weather::{WeatherSample, load_weather_data};

// Single writer (the refresher task) publishes the latest sample; every handler
// holds a cheap Clone receiver and reads the current value without locking.
type WeatherReceiver = watch::Receiver<Option<WeatherSample>>;

pub async fn serve(config: Config) -> Result<(), String> {
    let bind_addr: SocketAddr = config.server.bind_addr();
    let refresh_interval_secs: u64 = config.server.refresh_interval_secs;
    let weather_config: WeatherConfig = config.weather;

    let (tx, rx) = watch::channel::<Option<WeatherSample>>(None);

    tokio::spawn(refresh_loop(tx, weather_config, refresh_interval_secs));

    let app: Router = Router::new()
        .route("/health", get(health))
        .route("/weather", get(weather))
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
async fn refresh_loop(tx: watch::Sender<Option<WeatherSample>>, config: WeatherConfig, interval_secs: u64) {
    loop {
        match load_weather_data(&config).await {
            Ok(sample) => {
                if tx.send(Some(sample)).is_err() {
                    return;
                }
            },
            Err(e) => error!("Failed to refresh weather data: {}", e),
        }

        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
    }
}

async fn health() -> StatusCode {
    return StatusCode::OK;
}

async fn weather(State(rx): State<WeatherReceiver>) -> Response {
    return match rx.borrow().clone() {
        Some(sample) => Json(sample).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "Weather data not yet available").into_response(),
    };
}

#[cfg(test)]
mod tests {
    use axum::extract::State;
    use axum::http::StatusCode;
    use serde_json::json;
    use tokio::net::TcpListener;
    use tokio::sync::watch;

    use super::{health, serve, weather};
    use crate::mars::config::{Config, ServerConfig, WeatherConfig};
    use crate::mars::weather::WeatherSample;

    fn sample() -> WeatherSample {
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
        return serde_json::from_value(value).unwrap();
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
    async fn weather_returns_ok_with_data() {
        let (_tx, rx) = watch::channel::<Option<WeatherSample>>(Some(sample()));
        let response = weather(State(rx)).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn serve_errors_when_address_in_use() {
        // Occupy a port, then ask the server to bind the same one so the bind
        // fails deterministically without depending on an external network.
        let occupied: TcpListener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port: u16 = occupied.local_addr().unwrap().port();

        let config: Config = Config {
            server: ServerConfig {
                host: "127.0.0.1".parse().unwrap(),
                port,
                refresh_interval_secs: 3600,
            },
            weather: WeatherConfig { request_timeout_secs: 1, connect_timeout_secs: 1 },
        };

        let result: Result<(), String> = serve(config).await;
        assert!(result.is_err());
    }
}
