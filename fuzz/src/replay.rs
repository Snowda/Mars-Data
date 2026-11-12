//! Stable replay: every seed and regression input runs through its harness on each plain test run. A unit-test
//! module, not tests/, so `--lib` coverage runs it and cargo never builds the fuzz_targets bins for it.

use std::env;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::fs::{DirEntry, File};
#[cfg(windows)]
use std::fs::OpenOptions;
use std::io;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process;

use arrayvec::ArrayVec;

use super::{config, weather_body};

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
// libFuzzer's default -max_len is 4096 bytes; the cap leaves room for hand-written seeds.
const MAX_INPUT_BYTES: usize = 64 * 1024;

// Why a replay could not run; `target` borrows the caller's name, so nothing is copied.
#[derive(Debug)]
enum ReplayError<'a> {
	Io { path: PathBuf, source: io::Error },
	TooLarge { path: PathBuf },
	NoSeeds { target: &'a str },
}

impl Display for ReplayError<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		return match self {
			ReplayError::Io { path, source } => write!(f, "{}: {source}", path.display()),
			ReplayError::TooLarge { path } => write!(f, "{} is over the {MAX_INPUT_BYTES}-byte input cap", path.display()),
			ReplayError::NoSeeds { target } => write!(f, "fuzz/seeds/{target}/ holds no seed inputs"),
		};
	}
}

// Run `harness` over every non-hidden file in <root>/seeds/<target>/ and <root>/regressions/<target>/; returns how many ran.
fn replay<'a>(root: &Path, target: &'a str, harness: fn(&[u8])) -> Result<usize, ReplayError<'a>> {
	return ["seeds", "regressions"].iter().try_fold(0, |ran: usize, dir: &&str| -> Result<usize, ReplayError<'a>> {
		let Ok(entries) = fs::read_dir(root.join(dir).join(target)) else {
			return Ok(ran);
		};
		return entries.flatten().filter(is_visible_file).try_fold(ran, |ran: usize, entry: DirEntry| -> Result<usize, ReplayError<'a>> {
			let bytes: ArrayVec<u8, MAX_INPUT_BYTES> = read_input(&entry.path())?;
			harness(&bytes);
			return Ok(ran + 1);
		});
	});
}

// Reads one input into a stack buffer; ArrayVec's io::Write stops when full, so io::copy fails with WriteZero.
fn read_input(path: &Path) -> Result<ArrayVec<u8, MAX_INPUT_BYTES>, ReplayError<'static>> {
	let io_error = |source: io::Error| ReplayError::Io { path: path.to_path_buf(), source };
	let mut file: File = File::open(path).map_err(io_error)?;
	let mut bytes: ArrayVec<u8, MAX_INPUT_BYTES> = ArrayVec::new();
	return match io::copy(&mut file, &mut bytes) {
		Ok(_) => Ok(bytes),
		Err(e) if e.kind() == io::ErrorKind::WriteZero => Err(ReplayError::TooLarge { path: path.to_path_buf() }),
		Err(e) => Err(io_error(e)),
	};
}

// Dot-files (.gitkeep) and subdirectories are not inputs.
fn is_visible_file(entry: &DirEntry) -> bool {
	let is_hidden: bool = entry.file_name().to_string_lossy().starts_with('.');
	return !is_hidden && entry.path().is_file();
}

// Replay one target and fail if its seed directory held nothing to run.
fn replay_nonempty<'a>(root: &Path, target: &'a str, harness: fn(&[u8])) -> Result<(), ReplayError<'a>> {
	return match replay(root, target, harness) {
		Ok(0) => Err(ReplayError::NoSeeds { target }),
		Ok(_) => Ok(()),
		Err(e) => Err(e),
	};
}

#[test]
fn fuzz_replay_weather_body() -> Result<(), ReplayError<'static>> {
	return replay_nonempty(Path::new(MANIFEST_DIR), "weather_body", weather_body);
}

#[test]
fn fuzz_replay_config() -> Result<(), ReplayError<'static>> {
	return replay_nonempty(Path::new(MANIFEST_DIR), "config", config);
}

const TARGET: &str = "t";
const HIDDEN_BYTES: &[u8] = b"hidden";
#[cfg(windows)]
const EXCLUSIVE_SHARE_MODE: u32 = 0;

// A seeds/regressions tree under the OS temp dir, unique per test and removed on drop.
struct ScratchRoot {
	path: PathBuf,
}

impl ScratchRoot {
	fn new(name: &str) -> Result<Self, ReplayError<'static>> {
		let path: PathBuf = env::temp_dir().join(format!("mars-data-fuzz-replay-{}-{name}", process::id()));
		// A killed earlier run with the same pid can leave files behind.
		let _ = fs::remove_dir_all(&path);
		if let Err(source) = fs::create_dir_all(&path) {
			return Err(ReplayError::Io { path, source });
		}
		return Ok(Self { path });
	}

	fn mkdir(&self, rel: &str) -> Result<PathBuf, ReplayError<'static>> {
		let dir: PathBuf = self.path.join(rel);
		if let Err(source) = fs::create_dir_all(&dir) {
			return Err(ReplayError::Io { path: dir, source });
		}
		return Ok(dir);
	}

	fn write(&self, rel_dir: &str, name: &str, bytes: &[u8]) -> Result<PathBuf, ReplayError<'static>> {
		let file: PathBuf = self.mkdir(rel_dir)?.join(name);
		if let Err(source) = fs::write(&file, bytes) {
			return Err(ReplayError::Io { path: file, source });
		}
		return Ok(file);
	}
}

impl Drop for ScratchRoot {
	fn drop(&mut self) {
		let _ = fs::remove_dir_all(&self.path);
	}
}

fn accept_any(_data: &[u8]) {}

fn reject_hidden(data: &[u8]) {
	assert_ne!(data, HIDDEN_BYTES, "replay must skip dot-files");
}

#[test]
fn replay_runs_seeds_and_regressions() -> Result<(), ReplayError<'static>> {
	let scratch: ScratchRoot = ScratchRoot::new("both_dirs")?;
	scratch.write("seeds/t", "a", b"a")?;
	scratch.write("regressions/t", "b", b"b")?;

	let ran: usize = replay(&scratch.path, TARGET, accept_any)?;

	assert_eq!(ran, 2, "one file in each of seeds/ and regressions/ must both run");
	return Ok(());
}

#[test]
fn replay_skips_hidden_files_and_subdirs() -> Result<(), ReplayError<'static>> {
	let scratch: ScratchRoot = ScratchRoot::new("skips")?;
	scratch.write("seeds/t", ".hidden", HIDDEN_BYTES)?;
	scratch.mkdir("seeds/t/subdir")?;
	scratch.write("seeds/t", "visible", b"visible")?;

	let ran: usize = replay(&scratch.path, TARGET, reject_hidden)?;

	assert_eq!(ran, 1, "only the visible file counts; the dot-file and the subdirectory are skipped");
	return Ok(());
}

#[test]
fn replay_missing_target_runs_nothing() -> Result<(), ReplayError<'static>> {
	let scratch: ScratchRoot = ScratchRoot::new("missing")?;

	let ran: usize = replay(&scratch.path, TARGET, accept_any)?;

	assert_eq!(ran, 0, "absent seeds/ and regressions/ dirs are skipped, not errors");
	return Ok(());
}

#[test]
fn replay_nonempty_errs_when_only_hidden_seeds() -> Result<(), ReplayError<'static>> {
	let scratch: ScratchRoot = ScratchRoot::new("empty")?;
	scratch.write("seeds/t", ".gitkeep", b"")?;

	let result: Result<(), ReplayError<'_>> = replay_nonempty(&scratch.path, TARGET, accept_any);

	assert!(matches!(result, Err(ReplayError::NoSeeds { target: TARGET })), "an empty corpus must fail loudly, got {result:?}");
	return Ok(());
}

#[test]
fn no_seeds_displays_the_seed_dir() {
	assert_eq!(ReplayError::NoSeeds { target: TARGET }.to_string(), "fuzz/seeds/t/ holds no seed inputs", "the String-era text stays");
}

#[test]
fn replay_refuses_an_input_over_the_cap_and_takes_one_at_the_cap() -> Result<(), ReplayError<'static>> {
	let scratch: ScratchRoot = ScratchRoot::new("cap")?;
	let at_cap: [u8; MAX_INPUT_BYTES] = [0; MAX_INPUT_BYTES];
	scratch.write("seeds/t", "at_cap", &at_cap)?;
	assert_eq!(replay(&scratch.path, TARGET, accept_any)?, 1, "an input of exactly MAX_INPUT_BYTES must run");

	let over: [u8; MAX_INPUT_BYTES + 1] = [0; MAX_INPUT_BYTES + 1];
	let over_path: PathBuf = scratch.write("seeds/t", "over", &over)?;
	let result: Result<usize, ReplayError<'_>> = replay(&scratch.path, TARGET, accept_any);
	assert!(
		matches!(&result, Err(ReplayError::TooLarge { path }) if *path == over_path),
		"one byte over the cap must be refused, not cut, got {result:?}"
	);
	return Ok(());
}

#[cfg(windows)]
#[test]
fn replay_nonempty_propagates_read_error() -> Result<(), ReplayError<'static>> {
	let scratch: ScratchRoot = ScratchRoot::new("read_error")?;
	let locked: PathBuf = scratch.write("seeds/t", "locked", b"locked")?;
	// Share mode 0 denies every other open, so the replay's open fails while is_file() still sees a file.
	let _lock: File = match OpenOptions::new().read(true).share_mode(EXCLUSIVE_SHARE_MODE).open(&locked) {
		Ok(file) => file,
		Err(source) => return Err(ReplayError::Io { path: locked, source }),
	};

	// replay names the file by its read_dir path, which joins one component at a time.
	let entry_path: PathBuf = scratch.path.join("seeds").join(TARGET).join("locked");

	let result: Result<(), ReplayError<'_>> = replay_nonempty(&scratch.path, TARGET, accept_any);

	let is_read_error: bool = matches!(&result, Err(ReplayError::Io { path, .. }) if *path == entry_path);
	assert!(is_read_error, "an unreadable seed must surface as Err naming the file, got {result:?}");
	return Ok(());
}
