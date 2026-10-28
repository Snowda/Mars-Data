use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::NonZeroU64;
use std::time::Duration;

use serde::Deserialize;
use url::Url;

const CONFIG_PATH: &str = "config.toml";
// Loopback by default so the API is not exposed on all interfaces without the
// operator explicitly opting in (set server.host = "0.0.0.0" for public serving).
const DEFAULT_HOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const DEFAULT_PORT: u16 = 3000;
const DEFAULT_REFRESH_INTERVAL_SECS: Seconds = Seconds::new(3600).expect("non-zero literal");
const DEFAULT_REQUEST_TIMEOUT_SECS: Seconds = Seconds::new(5).expect("non-zero literal");
const DEFAULT_CONNECT_TIMEOUT_SECS: Seconds = Seconds::new(3).expect("non-zero literal");
const DEFAULT_WEATHER_URL: &str = "https://mars.nasa.gov/rss/api/?feed=weather&category=msl&feedtype=json";

// Whole seconds, never zero: a zero refresh hammers the upstream and a zero timeout fails every fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct Seconds(NonZeroU64);

impl Seconds {
    #[must_use]
    pub const fn new(secs: u64) -> Option<Self> {
        return match NonZeroU64::new(secs) {
            Some(non_zero) => Some(Seconds(non_zero)),
            None => None,
        };
    }

    #[must_use]
    pub const fn as_duration(self) -> Duration {
        return Duration::from_secs(self.0.get());
    }
}

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
    pub refresh_interval_secs: Seconds,
}

impl ServerConfig {
    pub const fn bind_addr(&self) -> SocketAddr {
        return SocketAddr::new(self.host, self.port);
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct WeatherConfig {
    // None reads the NASA MSL feed. A Url, not a String, so a malformed override fails at load.
    pub url: Option<Url>,
    pub request_timeout_secs: Seconds,
    pub connect_timeout_secs: Seconds,
}

impl WeatherConfig {
    #[must_use]
    pub fn url(&self) -> &str {
        return self.url.as_ref().map_or(DEFAULT_WEATHER_URL, Url::as_str);
    }
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
            url: None,
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
    use std::time::Duration;

    use url::Url;

    use super::Config;
    use super::ConfigError;
    use super::DEFAULT_CONNECT_TIMEOUT_SECS;
    use super::DEFAULT_REFRESH_INTERVAL_SECS;
    use super::DEFAULT_REQUEST_TIMEOUT_SECS;
    use super::DEFAULT_WEATHER_URL;
    use super::Seconds;

    #[test]
    fn parses_all_fields() -> Result<(), ConfigError> {
        let toml: &str = "[server]\nhost = \"127.0.0.1\"\nport = 8080\nrefresh_interval_secs = 60\n\n[weather]\nrequest_timeout_secs = 9\nconnect_timeout_secs = 4";
        let config: Config = Config::from_toml_str(toml)?;
        let expected_addr: SocketAddr = SocketAddr::from(([127, 0, 0, 1], 8080));
        assert_eq!(config.server.bind_addr(), expected_addr);
        assert_eq!(config.server.refresh_interval_secs.as_duration(), Duration::from_secs(60));
        assert_eq!(config.weather.request_timeout_secs.as_duration(), Duration::from_secs(9));
        assert_eq!(config.weather.connect_timeout_secs.as_duration(), Duration::from_secs(4));
        return Ok(());
    }

    #[test]
    fn seconds_rejects_zero_and_converts_to_duration() {
        assert_eq!(Seconds::new(0), None, "zero is not a usable interval or timeout");
        assert_eq!(Seconds::new(7).map(Seconds::as_duration), Some(Duration::from_secs(7)));
    }

    #[test]
    fn weather_url_defaults_to_the_nasa_msl_feed() -> Result<(), url::ParseError> {
        let config: Config = Config::default();
        assert_eq!(config.weather.url(), DEFAULT_WEATHER_URL);
        // The default is only a &str in code, so prove it parses the way an override would.
        assert_eq!(Url::parse(DEFAULT_WEATHER_URL)?.as_str(), DEFAULT_WEATHER_URL);
        return Ok(());
    }

    #[test]
    fn weather_url_override_replaces_the_default() -> Result<(), ConfigError> {
        let config: Config = Config::from_toml_str("[weather]\nurl = \"http://127.0.0.1:8080/feed\"")?;
        assert_eq!(config.weather.url(), "http://127.0.0.1:8080/feed");
        assert_eq!(config.weather.request_timeout_secs, DEFAULT_REQUEST_TIMEOUT_SECS, "other fields keep defaults");
        return Ok(());
    }

    #[test]
    fn default_binds_loopback_on_port_3000() {
        let config: Config = Config::default();
        let expected: SocketAddr = SocketAddr::from(([127, 0, 0, 1], 3000));
        assert_eq!(config.server.bind_addr(), expected);
    }

    #[test]
    fn loopback_default_is_not_unspecified_but_explicit_zero_host_is() -> Result<(), ConfigError> {
        // serve() warns when the bind address is unspecified (all interfaces);
        // this locks both directions of that guard's condition.
        assert!(!Config::default().server.bind_addr().ip().is_unspecified());
        let exposed: Config = Config::from_toml_str("[server]\nhost = \"0.0.0.0\"")?;
        assert!(exposed.server.bind_addr().ip().is_unspecified());
        assert_eq!(exposed.server.bind_addr().port(), 3000);
        return Ok(());
    }

    #[test]
    fn empty_config_uses_all_defaults() -> Result<(), ConfigError> {
        let config: Config = Config::from_toml_str("")?;
        assert_eq!(config.server.refresh_interval_secs, DEFAULT_REFRESH_INTERVAL_SECS);
        assert_eq!(config.weather.request_timeout_secs, DEFAULT_REQUEST_TIMEOUT_SECS);
        assert_eq!(config.weather.connect_timeout_secs, DEFAULT_CONNECT_TIMEOUT_SECS);
        return Ok(());
    }

    #[test]
    fn partial_config_overrides_only_named_fields() -> Result<(), ConfigError> {
        let config: Config = Config::from_toml_str("[server]\nport = 5000")?;
        let expected: SocketAddr = SocketAddr::from(([127, 0, 0, 1], 5000));
        assert_eq!(config.server.bind_addr(), expected);
        assert_eq!(config.server.refresh_interval_secs, DEFAULT_REFRESH_INTERVAL_SECS);
        assert_eq!(config.weather.request_timeout_secs, DEFAULT_REQUEST_TIMEOUT_SECS);
        return Ok(());
    }

    #[test]
    fn rejects_malformed_host() {
        let result: Result<Config, _> = Config::from_toml_str("[server]\nhost = \"not-an-address\"");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_zero_refresh_interval_and_timeouts() {
        // Zero refresh hammers the upstream; a zero timeout fails every fetch.
        ["[server]\nrefresh_interval_secs = 0", "[weather]\nrequest_timeout_secs = 0", "[weather]\nconnect_timeout_secs = 0"]
            .iter()
            .for_each(|toml: &&str| assert!(Config::from_toml_str(toml).is_err(), "zero must be rejected at load: {toml}"));
    }

    #[test]
    fn rejects_malformed_weather_url() {
        let result: Result<Config, ConfigError> = Config::from_toml_str("[weather]\nurl = \"not a url\"");
        assert!(result.is_err(), "a bad upstream URL must fail at load, not on the first fetch");
    }

    #[test]
    fn new_loads_repository_config_file() {
        // The repo ships a config.toml at the crate root, which is the cwd during
        // `cargo test`, so this exercises the real file-reading path.
        let result: Result<Config, ConfigError> = Config::new();
        assert!(result.is_ok());
    }

    #[test]
    fn missing_file_falls_back_to_defaults() -> Result<(), ConfigError> {
        let config: Config = Config::from_path("definitely-not-a-real-config.toml")?;
        assert_eq!(config.server.bind_addr(), Config::default().server.bind_addr());
        return Ok(());
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
        let Err(error) = Config::from_toml_str("= not valid toml") else {
            panic!("malformed toml should fail to parse");
        };
        assert!(format!("{}", error).contains("failed to parse config file"));
    }
}
