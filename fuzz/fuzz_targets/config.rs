// Startup config.toml: total, and an accepted config binds the host and port it names; a panic is the bug.
// Without `--cfg fuzzing` this is an empty main, so the workspace builds and tests on stable.
#![cfg_attr(fuzzing, no_main)]

#[cfg(fuzzing)]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
	mars_data_fuzz::config(data);
});

#[cfg(not(fuzzing))]
fn main() {}
