//! Stable replay: every seed and regression input runs through its harness on each plain test run. A unit-test
//! module, not tests/, so `--lib` coverage runs it and cargo never builds the fuzz_targets bins for it.

use std::fs;
use std::path::{Path, PathBuf};

use super::{config, weather_body};

// Run `harness` over every non-hidden file in seeds/<target>/ and regressions/<target>/; returns how many ran.
fn replay(target: &str, harness: fn(&[u8])) -> Result<usize, String> {
	let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
	let mut ran: usize = 0;
	for dir in ["seeds", "regressions"] {
		let Ok(entries) = fs::read_dir(root.join(dir).join(target)) else {
			continue;
		};
		for entry in entries.flatten() {
			let is_hidden: bool = entry.file_name().to_string_lossy().starts_with('.');
			if is_hidden || !entry.path().is_file() {
				continue;
			}
			let bytes: Vec<u8> = match fs::read(entry.path()) {
				Ok(bytes) => bytes,
				Err(e) => return Err(format!("read {}: {e}", entry.path().display())),
			};
			harness(&bytes);
			ran += 1;
		}
	}
	return Ok(ran);
}

// Replay one target and fail if its seed directory held nothing to run.
fn replay_nonempty(target: &str, harness: fn(&[u8])) -> Result<(), String> {
	return match replay(target, harness) {
		Ok(0) => Err(format!("fuzz/seeds/{target}/ holds no seed inputs")),
		Ok(_) => Ok(()),
		Err(e) => Err(e),
	};
}

#[test]
fn fuzz_replay_weather_body() -> Result<(), String> {
	return replay_nonempty("weather_body", weather_body);
}

#[test]
fn fuzz_replay_config() -> Result<(), String> {
	return replay_nonempty("config", config);
}
