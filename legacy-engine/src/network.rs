//! Initial URL validation and HTTP/HTTPS response fetching.
//!
//! This is a first networking milestone, not a complete browser networking
//! stack. `ureq` supplies HTTP/TLS protocol machinery; browser-specific
//! navigation policy, limits, and response handling live in this module.
//! TLS certificate verification remains enabled by default.

use std::io::{Read, Take};
use std::time::Duration;
use url::Url;

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
/// Images have a separate cap; this is a response-byte limit, not decoded-memory size.
pub const MAX_IMAGE_RESPONSE_BYTES: usize = 5 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug)]
pub struct PageResponse {
    pub final_url: String,
    pub status: u16,
    pub status_text: String,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub enum FetchError {
    InvalidUrl(String),
    UnsupportedScheme(String),
    Request(String),
    Read(String),
    ResponseTooLarge { limit: usize },
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl(message) => write!(f, "invalid URL: {message}"),
            Self::UnsupportedScheme(scheme) => {
                write!(f, "unsupported URL scheme '{scheme}'; use http or https")
            }
            Self::Request(message) => write!(f, "request error: {message}"),
            Self::Read(message) => write!(f, "response read error: {message}"),
            Self::ResponseTooLarge { limit } => {
                write!(f, "response body exceeds the {limit}-byte safety limit")
            }
        }
    }
}

impl std::error::Error for FetchError {}

/// Parse a URL and permit only HTTP or HTTPS destinations.
pub fn parse_http_url(input: &str) -> Result<Url, FetchError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(FetchError::InvalidUrl("empty input".to_owned()));
    }

    let url = Url::parse(trimmed)
        .map_err(|error| FetchError::InvalidUrl(error.to_string()))?;

    match url.scheme() {
        "http" | "https" => {}
        other => return Err(FetchError::UnsupportedScheme(other.to_owned())),
    }

    if url.host_str().is_none() {
        return Err(FetchError::InvalidUrl("URL has no host".to_owned()));
    }
    Ok(url)
}

/// Fetch a URL with redirects and bounded body reading.
///
/// Redirects are handled by the HTTP agent, with a finite redirect limit.
/// The response body is streamed and capped to avoid unbounded allocation.
pub fn fetch_page(input: &str) -> Result<PageResponse, FetchError> {
    fetch_with_limit(input, MAX_RESPONSE_BYTES)
}

/// Fetch a same-policy HTTP(S) resource while enforcing the caller's byte budget.
pub fn fetch_resource(input: &str, max_bytes: usize) -> Result<PageResponse, FetchError> {
    fetch_with_limit(input, max_bytes.min(MAX_IMAGE_RESPONSE_BYTES))
}

fn fetch_with_limit(input: &str, max_bytes: usize) -> Result<PageResponse, FetchError> {
    let parsed = parse_http_url(input)?;
    let original_url = parsed.to_string();

    let agent = ureq::AgentBuilder::new()
        .redirects(10)
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout_read(READ_TIMEOUT)
        .build();

    let response = match agent
        .get(original_url.as_str())
        .set("User-Agent", "Skylark/0.3 (custom engine; +no-javascript)")
        .set("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")
        .call()
    {
        Ok(response) => response,
        // ureq represents non-2xx/3xx HTTP responses as Status errors.
        // These still contain useful response bodies and status metadata.
        Err(ureq::Error::Status(_, response)) => response,
        Err(ureq::Error::Transport(error)) => {
            return Err(FetchError::Request(error.to_string()));
        }
    };

    let final_url = response.get_url().to_owned();
    let status = response.status();
    let status_text = response.status_text().to_owned();
    let content_type = response.header("Content-Type").map(str::to_owned);

    let reader = response.into_reader();
    let mut bounded: Take<Box<dyn Read>> =
        Box::new(reader).take((max_bytes as u64) + 1);
    let mut body = Vec::new();
    bounded
        .read_to_end(&mut body)
        .map_err(|error| FetchError::Read(error.to_string()))?;

    if body.len() > max_bytes {
        return Err(FetchError::ResponseTooLarge {
            limit: max_bytes,
        });
    }

    Ok(PageResponse {
        final_url,
        status,
        status_text,
        content_type,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_response_budget_is_bounded() {
        assert_eq!(MAX_IMAGE_RESPONSE_BYTES, 5 * 1024 * 1024);
    }

    #[test]
    fn accepts_http_and_https() {
        assert_eq!(parse_http_url("https://example.com").unwrap().scheme(), "https");
        assert_eq!(parse_http_url("http://example.com/path").unwrap().scheme(), "http");
    }

    #[test]
    fn rejects_empty_and_invalid_urls() {
        assert!(parse_http_url("").is_err());
        assert!(parse_http_url("not a URL").is_err());
    }

    #[test]
    fn rejects_non_web_schemes() {
        assert!(matches!(
            parse_http_url("file:///etc/passwd"),
            Err(FetchError::UnsupportedScheme(_))
        ));
    }

    #[test]
    fn normalizes_url_using_url_parser() {
        let url = parse_http_url("https://EXAMPLE.com:443/a/../b").unwrap();
        assert_eq!(url.as_str(), "https://example.com/b");
    }
}
