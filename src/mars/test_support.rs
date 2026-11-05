// Test-only helpers: Display text in a fixed buffer, and one error type that test bodies return with `?`.

use std::fmt::{self, Display, Formatter, Write};
use std::io;

use arrayvec::ArrayString;

use crate::mars::weather::FetchError;

// An error's text is a line or two; tests that assert on a message use this capacity.
pub const MAX_MESSAGE_BYTES: usize = 512;

// Panics when the text is longer than N, so a too-small buffer fails the test and does not cut the text.
#[must_use]
pub fn display<const N: usize>(value: &impl Display) -> ArrayString<N> {
    let mut text: ArrayString<N> = ArrayString::new();
    if write!(text, "{value}").is_err() {
        panic!("the Display text is longer than the {N}-byte test buffer");
    }
    return text;
}

// Each failure a test body can hit before its asserts.
#[derive(Debug)]
pub enum TestError {
    Io(io::Error),
    Url(url::ParseError),
    Fetch(FetchError),
    Json(serde_json::Error),
    Body(axum::Error),
}

impl Display for TestError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return match self {
            TestError::Io(e) => write!(f, "io: {e}"),
            TestError::Url(e) => write!(f, "url: {e}"),
            TestError::Fetch(e) => write!(f, "fetch: {e}"),
            TestError::Json(e) => write!(f, "json: {e}"),
            TestError::Body(e) => write!(f, "body: {e}"),
        };
    }
}

impl From<io::Error> for TestError {
    fn from(e: io::Error) -> Self {
        return TestError::Io(e);
    }
}

impl From<url::ParseError> for TestError {
    fn from(e: url::ParseError) -> Self {
        return TestError::Url(e);
    }
}

impl From<FetchError> for TestError {
    fn from(e: FetchError) -> Self {
        return TestError::Fetch(e);
    }
}

impl From<serde_json::Error> for TestError {
    fn from(e: serde_json::Error) -> Self {
        return TestError::Json(e);
    }
}

impl From<axum::Error> for TestError {
    fn from(e: axum::Error) -> Self {
        return TestError::Body(e);
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use arrayvec::ArrayString;
    use url::Url;

    use super::{MAX_MESSAGE_BYTES, TestError, display};
    use crate::mars::weather::FetchError;

    const FIVE_DIGITS: u32 = 12_345;

    #[test]
    fn display_writes_the_whole_text() {
        let text: ArrayString<8> = display(&FIVE_DIGITS);
        assert_eq!(text.as_str(), "12345", "a short value must be copied whole");
    }

    #[test]
    #[should_panic(expected = "longer than the 4-byte test buffer")]
    fn display_panics_instead_of_truncating() {
        let _text: ArrayString<4> = display(&FIVE_DIGITS);
    }

    #[test]
    fn test_error_names_the_step_that_failed() {
        let Err(url) = Url::parse("not a url") else {
            panic!("a URL with no scheme must not parse");
        };
        let Err(json) = serde_json::from_str::<u8>("x") else {
            panic!("a bare letter is not JSON");
        };
        let cases: [(TestError, &str); 5] = [
            (TestError::from(io::Error::other("disk")), "io: disk"),
            (TestError::from(url), "url: relative URL without a base"),
            (TestError::from(FetchError::NoSolData), "fetch: No sol data found in response"),
            (TestError::from(json), "json: expected value at line 1 column 1"),
            (TestError::from(axum::Error::new(io::Error::other("cut"))), "body: cut"),
        ];
        cases.iter().for_each(|(error, expected): &(TestError, &str)| {
            let text: ArrayString<MAX_MESSAGE_BYTES> = display(error);
            assert_eq!(text.as_str(), *expected, "the prefix names the failed step, then the cause");
        });
    }
}
