use std::fs;
use std::error::Error;
use serde_json::Value;

pub fn load_config(file_path: &str) -> Result<String, Box<dyn Error>> {
    let config_content = fs::read_to_string(file_path)?; // TODO replace ?s with proper error handling
    let config: Value = serde_json::from_str(&config_content)?; // TODO replace ?s with proper error handling

    match config.get("DEMO_KEY") { // Extract the DEMO_KEY from the config
        Some(key) => {
            match key.as_str() {
                Some(key_str) => Ok(key_str.to_string()),
                None => Err("DEMO_KEY is not a string in config file".into())
            }
        },
        None => Err("DEMO_KEY not found in config file".into())
    }
}
