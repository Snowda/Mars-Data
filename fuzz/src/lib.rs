//! Fuzz harnesses for Mars-Data's untrusted input boundaries. Each takes raw bytes and asserts the boundary's
//! invariant; the libFuzzer targets (fuzz_targets/) and the stable replay test (src/replay.rs) both call them.

#[cfg(test)]
mod replay;

use core::str::from_utf8;
use std::net::SocketAddr;

use mars_data::mars::config::Config;
use mars_data::mars::weather::{WeatherSample, parse_weather_body};

/// NASA MSL feed body: total, and an accepted sample serves as JSON that decodes and re-encodes byte-exact.
pub fn weather_body(data: &[u8]) {
	let Ok(sample) = parse_weather_body(data) else {
		return;
	};
	let wire: Result<Vec<u8>, serde_json::Error> = serde_json::to_vec(&sample);
	assert!(wire.is_ok(), "an accepted sample must serialize for the /weather response");
	let Ok(wire) = wire else {
		return;
	};
	let decoded: Result<WeatherSample, serde_json::Error> = serde_json::from_slice(&wire);
	assert!(decoded.is_ok(), "the served JSON must decode back into a WeatherSample");
	let Ok(decoded) = decoded else {
		return;
	};
	let again: Result<Vec<u8>, serde_json::Error> = serde_json::to_vec(&decoded);
	assert!(again.is_ok(), "a decoded sample must re-serialize");
	let Ok(again) = again else {
		return;
	};
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
