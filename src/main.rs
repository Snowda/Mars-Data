mod mars;

use mars::calculations::{mars_comms_delay, mars_comms_return_delay};
use mars::weather::load_weather_data;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    info!("Mars communications delay: {} seconds", mars_comms_delay(None));
    info!("Mars round-trip communications delay: {} seconds", mars_comms_return_delay(None));

    match load_weather_data().await {
        Ok(weather_data) => {
            let _ = weather_data.print_report();
        },
        Err(e) => {
            error!("Error loading weather data: {}", e);
            return;
        }
    };
}
