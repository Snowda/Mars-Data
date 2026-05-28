mod mars;

use std::env;

use mars::api::serve;
use mars::calculations::{mars_comms_delay, mars_comms_return_delay};
use mars::config::{Config, WeatherConfig};
use mars::weather::load_weather_data;
use tracing::{error, info};

const SERVE_COMMAND: &str = "serve";
const JSON_FLAG: &str = "--json";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config: Config = match Config::new() {
        Ok(loaded) => loaded,
        Err(e) => {
            error!("Failed to load config: {}", e);
            return;
        }
    };

    let command: Option<String> = env::args().nth(1);

    match command.as_deref() {
        Some(SERVE_COMMAND) => run_server(config).await,
        Some(JSON_FLAG) => print_weather_json(&config.weather).await,
        _ => print_report(&config.weather).await,
    };
}

async fn run_server(config: Config) {
    if let Err(e) = serve(config).await {
        error!("Server failed: {}", e);
    }
}

async fn print_weather_json(config: &WeatherConfig) {
    match load_weather_data(config).await {
        Ok(weather_data) => match serde_json::to_string_pretty(&weather_data) {
            Ok(json) => info!("{}", json),
            Err(e) => error!("Failed to serialize weather data: {}", e),
        },
        Err(e) => error!("Error loading weather data: {}", e),
    };
}

async fn print_report(config: &WeatherConfig) {
    info!("Mars communications delay: {} seconds", mars_comms_delay(None));
    info!("Mars round-trip communications delay: {} seconds", mars_comms_return_delay(None));

    match load_weather_data(config).await {
        Ok(weather_data) => {
            let _ = weather_data.print_report();
        },
        Err(e) => error!("Error loading weather data: {}", e),
    };
}
