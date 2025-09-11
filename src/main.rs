mod mars;

use mars::calculations::{mars_comms_delay, mars_comms_return_delay};
use mars::config::load_config;
use mars::weather::load_weather_data;

#[tokio::main]
async fn main() {
    println!("Mars communications delay: {} seconds", mars_comms_delay(None));
    println!("Mars round-trip communications delay: {} seconds", mars_comms_return_delay(None));
    
    match load_config("config.json") {
        Ok(api_key) => {
            match load_weather_data(&api_key).await {
                Ok(weather_data) => {
                    let _ = weather_data.print_report();
                },
                Err(e) => {
                    eprintln!("Error loading weather data: {}", e);
                    return;
                }
            };
        },
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            return;
        }
    };
}
