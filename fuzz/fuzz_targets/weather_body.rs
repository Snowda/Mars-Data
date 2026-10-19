// NASA MSL feed body: total, and an accepted sample's served JSON round-trips byte-exact; a panic is the bug.
// Without `--cfg fuzzing` this is an empty main, so the workspace builds and tests on stable.
#![cfg_attr(fuzzing, no_main)]

#[cfg(fuzzing)]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
	mars_data_fuzz::weather_body(data);
});

#[cfg(not(fuzzing))]
fn main() {}
