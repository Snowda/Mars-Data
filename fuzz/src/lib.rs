//! Fuzz harnesses for Mars-Data's untrusted input boundaries. Each takes raw bytes and asserts the boundary's
//! invariant; the libFuzzer targets (fuzz_targets/) and the stable replay test (src/replay.rs) both call them.

#[cfg(test)]
mod replay;

use core::str::from_utf8;
use std::net::SocketAddr;

use arrayvec::ArrayVec;
use mars_data::mars::config::Config;
use mars_data::mars::weather::{MAX_SAMPLE_JSON_BYTES, WeatherSample, parse_weather_body};

/// NASA MSL feed body: total, and an accepted sample serves as JSON that decodes and re-encodes byte-exact.
pub fn weather_body(data: &[u8]) {
	let Ok(sample) = parse_weather_body(data) else {
		return;
	};
	// A stack buffer of the published bound: if a sample's JSON outgrows it, the fuzzer reports a crash.
	let mut wire: ArrayVec<u8, MAX_SAMPLE_JSON_BYTES> = ArrayVec::new();
	let written: Result<(), serde_json::Error> = serde_json::to_writer(&mut wire, &sample);
	assert!(written.is_ok(), "an accepted sample must serialize within MAX_SAMPLE_JSON_BYTES for the /weather response");
	let decoded: Result<WeatherSample, serde_json::Error> = serde_json::from_slice(&wire);
	assert!(decoded.is_ok(), "the served JSON must decode back into a WeatherSample");
	let Ok(decoded) = decoded else {
		return;
	};
	let mut again: ArrayVec<u8, MAX_SAMPLE_JSON_BYTES> = ArrayVec::new();
	let rewritten: Result<(), serde_json::Error> = serde_json::to_writer(&mut again, &decoded);
	assert!(rewritten.is_ok(), "a decoded sample must re-serialize");
	assert_eq!(again, wire, "serialize -> deserialize -> serialize must be byte-exact");
}

/// Startup config.toml: total, and an accepted config binds exactly the host and port it names.
pub fn config(data: &[u8]) {
	let Ok(text) = from_utf8(data) else {
		return;
	};
	let Ok(config) = toml::from_str::<Config>(text) else {
		return;
	};
	let expected: SocketAddr = SocketAddr::new(config.server.host, config.server.port);
	assert_eq!(config.server.bind_addr(), expected, "bind_addr must use the configured host and port");
}

// Line 26 (the `else { return; }` arm after `assert!(decoded.is_ok())`) is unreachable: the assert fires first.
#[cfg(test)]
mod tests {
	use core::str::from_utf8;

	use super::{Config, config, parse_weather_body, weather_body};

	// 0xFF and 0xFE never occur in UTF-8 (rustc's invalid_from_utf8 lint proves it), so these bytes stop before TOML.
	const INVALID_UTF8: &[u8] = &[0xFF, 0xFE, 0xFD];
	const UNCLOSED_TABLE_TOML: &[u8] = b"[server\nport = ";
	const NOT_JSON: &[u8] = b"not json";
	const REPO_CONFIG: &[u8] = include_bytes!("../seeds/config/00_repo_config_toml");
	const MSL_FEED: &[u8] = include_bytes!("../seeds/weather_body/00_msl_feed_two_sols");

	#[test]
	fn config_ignores_non_utf8_bytes() {
		config(INVALID_UTF8);
	}

	#[test]
	fn config_ignores_unparseable_toml() {
		let text: &str = from_utf8(UNCLOSED_TABLE_TOML).unwrap_or_default();
		assert!(!text.is_empty(), "fixture is ASCII, so it must reach the TOML parser");
		assert!(toml::from_str::<Config>(text).is_err(), "fixture must hit the TOML-reject early return");
		config(UNCLOSED_TABLE_TOML);
	}

	#[test]
	fn config_checks_accepted_repo_config() {
		let text: &str = from_utf8(REPO_CONFIG).unwrap_or_default();
		assert!(toml::from_str::<Config>(text).is_ok(), "the repo config seed must reach the bind_addr assert");
		config(REPO_CONFIG);
	}

	#[test]
	fn weather_body_ignores_rejected_body() {
		assert!(parse_weather_body(NOT_JSON).is_err(), "fixture must hit the reject early return");
		weather_body(NOT_JSON);
	}

	#[test]
	fn weather_body_round_trips_accepted_feed() {
		assert!(parse_weather_body(MSL_FEED).is_ok(), "the MSL feed seed must reach the round-trip asserts");
		weather_body(MSL_FEED);
	}
}
