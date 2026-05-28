use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde::Deserialize;

const CONFIG_PATH: &str = "config.toml";
const DEFAULT_HOST: IpAddr = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
const DEFAULT_PORT: u16 = 3000;
const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 3600;
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 5;
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 3;

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Parse(toml::de::Error),
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return match self {
            ConfigError::Io(e) => write!(f, "failed to read config file: {}", e),
            ConfigError::Parse(e) => write!(f, "failed to parse config file: {}", e),
        };
    }
}

impl std::error::Error for ConfigError {}

// Every field carries a serde default, so a partial config.toml overrides only
// the values it names and leaves the rest at the built-in defaults.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub weather: WeatherConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    // IpAddr deserializes from a string, so an invalid host surfaces as a parse
    // error rather than a runtime bind failure.
    pub host: IpAddr,
    pub port: u16,
    pub refresh_interval_secs: u64,
}

impl ServerConfig {
    pub fn bind_addr(&self) -> SocketAddr {
        return SocketAddr::new(self.host, self.port);
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct WeatherConfig {
    pub request_timeout_secs: u64,
    pub connect_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        return ServerConfig {
            host: DEFAULT_HOST,
            port: DEFAULT_PORT,
            refresh_interval_secs: DEFAULT_REFRESH_INTERVAL_SECS,
        };
    }
}

impl Default for WeatherConfig {
    fn default() -> Self {
        return WeatherConfig {
            request_timeout_secs: DEFAULT_REQUEST_TIMEOUT_SECS,
            connect_timeout_secs: DEFAULT_CONNECT_TIMEOUT_SECS,
        };
    }
}

impl Default for Config {
    fn default() -> Self {
        return Config { server: ServerConfig::default(), weather: WeatherConfig::default() };
    }
}

impl Config {
    pub fn new() -> Result<Self, ConfigError> {
        return Self::from_path(CONFIG_PATH);
    }

    // Loads config from the given path. A missing file is not an error: the
    // binary should run out of the box, so we fall back to the built-in defaults.
    fn from_path(path: &str) -> Result<Self, ConfigError> {
        let contents: String = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(ConfigError::Io(e)),
        };

        return Self::from_toml_str(&contents);
    }

    fn from_toml_str(contents: &str) -> Result<Self, ConfigError> {
        return toml::from_str(contents).map_err(ConfigError::Parse);
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::net::SocketAddr;

    use super::Config;
    use super::ConfigError;
    use super::DEFAULT_CONNECT_TIMEOUT_SECS;
    use super::DEFAULT_REFRESH_INTERVAL_SECS;
    use super::DEFAULT_REQUEST_TIMEOUT_SECS;

    #[test]
    fn parses_all_fields() {
        let toml: &str = "[server]\nhost = \"127.0.0.1\"\nport = 8080\nrefresh_interval_secs = 60\n\n[weather]\nrequest_timeout_secs = 9\nconnect_timeout_secs = 4";
        let config: Config = Config::from_toml_str(toml).unwrap();
        let expected_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        assert_eq!(config.server.bind_addr(), expected_addr);
        assert_eq!(config.server.refresh_interval_secs, 60);
        assert_eq!(config.weather.request_timeout_secs, 9);
        assert_eq!(config.weather.connect_timeout_secs, 4);
    }

    #[test]
    fn default_binds_all_interfaces_on_port_3000() {
        let config: Config = Config::default();
        let expected: SocketAddr = "0.0.0.0:3000".parse().unwrap();
        assert_eq!(config.server.bind_addr(), expected);
    }

    #[test]
    fn empty_config_uses_all_defaults() {
        let config: Config = Config::from_toml_str("").unwrap();
        assert_eq!(config.server.refresh_interval_secs, DEFAULT_REFRESH_INTERVAL_SECS);
        assert_eq!(config.weather.request_timeout_secs, DEFAULT_REQUEST_TIMEOUT_SECS);
        assert_eq!(config.weather.connect_timeout_secs, DEFAULT_CONNECT_TIMEOUT_SECS);
    }

    #[test]
    fn partial_config_overrides_only_named_fields() {
        let config: Config = Config::from_toml_str("[server]\nport = 5000").unwrap();
        let expected: SocketAddr = "0.0.0.0:5000".parse().unwrap();
        assert_eq!(config.server.bind_addr(), expected);
        assert_eq!(config.server.refresh_interval_secs, DEFAULT_REFRESH_INTERVAL_SECS);
        assert_eq!(config.weather.request_timeout_secs, DEFAULT_REQUEST_TIMEOUT_SECS);
    }

    #[test]
    fn rejects_malformed_host() {
        let result: Result<Config, _> = Config::from_toml_str("[server]\nhost = \"not-an-address\"");
        assert!(result.is_err());
    }

    #[test]
    fn new_loads_repository_config_file() {
        // The repo ships a config.toml at the crate root, which is the cwd during
        // `cargo test`, so this exercises the real file-reading path.
        let result: Result<Config, ConfigError> = Config::new();
        assert!(result.is_ok());
    }

    #[test]
    fn missing_file_falls_back_to_defaults() {
        let config: Config = Config::from_path("definitely-not-a-real-config.toml").unwrap();
        assert_eq!(config.server.bind_addr(), Config::default().server.bind_addr());
    }

    #[test]
    fn unreadable_path_yields_io_error() {
        // Reading a directory as a file fails with a non-NotFound IO error.
        let result: Result<Config, ConfigError> = Config::from_path(".");
        assert!(matches!(result, Err(ConfigError::Io(_))));
    }

    #[test]
    fn io_error_displays_read_message() {
        let error: ConfigError = ConfigError::Io(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));
        assert!(format!("{}", error).contains("failed to read config file"));
    }

    #[test]
    fn parse_error_displays_parse_message() {
        let error: ConfigError = Config::from_toml_str("= not valid toml").unwrap_err();
        assert!(format!("{}", error).contains("failed to parse config file"));
    }
}
