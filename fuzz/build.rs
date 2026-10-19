// Declares the `fuzzing` cfg that only the libFuzzer container build sets, so a plain build does not warn.
use std::io::{self, Write};

fn main() -> io::Result<()> {
	return writeln!(io::stdout(), "cargo::rustc-check-cfg=cfg(fuzzing)");
}
