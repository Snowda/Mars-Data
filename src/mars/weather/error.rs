// The ways a fetch or decode of the latest sol can fail.
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::time::Duration;

use hyper::StatusCode;
use hyper::http::uri::InvalidUri;
use hyper_util::client::legacy::Error as RequestError;

#[derive(Debug)]
pub enum FetchError {
    ClientBuild(rustls::Error),
    InvalidUri(InvalidUri),
    Request(RequestError),
    // Any status but 200 OK.
    Status(StatusCode),
    BodyRead(hyper::Error),
    BodyTooLarge { cap_bytes: usize },
    Timeout(Duration),
    InvalidJson(serde_json::Error),
    NoSolData,
}

impl Display for FetchError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        return match self {
            FetchError::ClientBuild(e) => write!(f, "Failed to build HTTP client: {}", e),
            FetchError::InvalidUri(e) => write!(f, "Invalid weather URL: {}", e),
            FetchError::Request(e) => write!(f, "Request error: {}", e),
            FetchError::Status(StatusCode::PAYLOAD_TOO_LARGE) => {
                write!(f, "Payload too large: Status code {}", StatusCode::PAYLOAD_TOO_LARGE)
            },
            FetchError::Status(status) => write!(f, "Unexpected status code: {}", status),
            FetchError::BodyRead(e) => write!(f, "Failed to read response body: {}", e),
            FetchError::BodyTooLarge { cap_bytes } => write!(f, "Response body exceeded the {}-byte cap", cap_bytes),
            FetchError::Timeout(timeout) => write!(f, "No complete response within {:?}", timeout),
            FetchError::InvalidJson(e) => write!(f, "Failed to parse JSON response: {}", e),
            FetchError::NoSolData => write!(f, "No sol data found in response"),
        };
    }
}

impl Error for FetchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        return match self {
            FetchError::ClientBuild(e) => Some(e),
            FetchError::InvalidUri(e) => Some(e),
            FetchError::Request(e) => Some(e),
            FetchError::BodyRead(e) => Some(e),
            FetchError::InvalidJson(e) => Some(e),
            FetchError::Status(_) | FetchError::BodyTooLarge { .. } | FetchError::Timeout(_) | FetchError::NoSolData => None,
        };
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::time::Duration;

    use arrayvec::ArrayString;
    use hyper::http::uri::InvalidUri;
    use hyper::{StatusCode, Uri};

    use super::FetchError;
    use crate::mars::test_support::{MAX_MESSAGE_BYTES, display};

    fn json_error() -> serde_json::Error {
        let Err(e) = serde_json::from_slice::<serde_json::Value>(b"{") else {
            panic!("an unclosed object must not parse");
        };
        return e;
    }

    fn uri_error() -> InvalidUri {
        let Err(e) = "not a url".parse::<Uri>() else {
            panic!("a URI with spaces must not parse");
        };
        return e;
    }

    fn tls_error() -> rustls::Error {
        return rustls::Error::General("no usable cipher suite".to_owned());
    }

    #[test]
    fn display_prefixes_the_wrapped_errors() {
        let cases: [(FetchError, ArrayString<MAX_MESSAGE_BYTES>); 2] = [
            (FetchError::ClientBuild(tls_error()), display(&format_args!("Failed to build HTTP client: {}", tls_error()))),
            (FetchError::InvalidUri(uri_error()), display(&format_args!("Invalid weather URL: {}", uri_error()))),
        ];
        cases.iter().for_each(|(error, expected): &(FetchError, ArrayString<MAX_MESSAGE_BYTES>)| {
            let text: ArrayString<MAX_MESSAGE_BYTES> = display(error);
            assert_eq!(text, *expected, "message must carry the cause after its prefix");
            assert!(error.source().is_some(), "the wrapped cause must stay reachable");
        });
    }

    // The String messages these variants replaced. Logs must read the same.
    #[test]
    fn display_keeps_the_former_string_messages() {
        let cases: [(FetchError, &str); 6] = [
            (FetchError::Status(StatusCode::NOT_FOUND), "Unexpected status code: 404 Not Found"),
            (FetchError::Status(StatusCode::PAYLOAD_TOO_LARGE), "Payload too large: Status code 413 Payload Too Large"),
            (FetchError::BodyTooLarge { cap_bytes: 8 }, "Response body exceeded the 8-byte cap"),
            (FetchError::Timeout(Duration::from_secs(5)), "No complete response within 5s"),
            (FetchError::NoSolData, "No sol data found in response"),
            (FetchError::InvalidJson(json_error()), "Failed to parse JSON response: EOF while parsing an object at line 1 column 1"),
        ];
        cases.iter().for_each(|(error, expected): &(FetchError, &str)| assert_eq!(error.to_string(), *expected));
    }

    #[test]
    fn source_chains_only_wrapped_errors() {
        assert!(FetchError::InvalidJson(json_error()).source().is_some(), "the serde_json cause must stay reachable");
        assert!(FetchError::Status(StatusCode::NOT_FOUND).source().is_none(), "a bare status has no cause");
        assert!(FetchError::Timeout(Duration::from_secs(1)).source().is_none(), "a timeout has no cause");
        assert!(FetchError::NoSolData.source().is_none(), "an empty feed has no cause");
    }
}
