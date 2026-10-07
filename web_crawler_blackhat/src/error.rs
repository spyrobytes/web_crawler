use reqwest::StatusCode;
use std::time::Duration;
use thiserror::Error;

/// Everything that can go wrong in a crawl, grouped by what a caller could do
/// about it. The point of the variants is `is_retryable`: a rate limit and a
/// changed page layout both end a scrape, but only one is worth another try.
#[derive(Error, Debug)]
pub enum Error {
    /// A bug or an invariant we broke ourselves.
    #[error("Internal: {0}")]
    Internal(String),

    #[error("Spider is not valid: {0}")]
    InvalidSpider(String),

    /// The request never produced a usable response: DNS, connect, TLS,
    /// timeout, or a body that could not be read.
    #[error("request to {url} failed: {source}")]
    Fetch {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    /// The server answered, but not with a 2xx.
    #[error("{url} answered HTTP {status}")]
    HttpStatus { url: String, status: StatusCode },

    /// The server told us to slow down: HTTP 429, GitHub's 403 with an
    /// exhausted quota, or a 503 that names a `Retry-After`.
    #[error("{url} rate-limited us (HTTP {status}){}", retry_after_hint(.retry_after))]
    RateLimited {
        url: String,
        status: StatusCode,
        retry_after: Option<Duration>,
    },

    /// The response arrived but did not look like the page the spider
    /// expects: a missing table, a malformed row, a non-JSON body.
    #[error("{url}: unexpected page content: {reason}")]
    Parse { url: String, reason: String },

    #[error("WebDriver: {0}")]
    WebDriver(String),
}

fn retry_after_hint(retry_after: &Option<Duration>) -> String {
    match retry_after {
        Some(delay) => format!(", retry after {}s", delay.as_secs()),
        None => String::new(),
    }
}

impl Error {
    /// Whether fetching the same URL again later could reasonably succeed.
    /// Nothing acts on this yet; it is the hook for backoff and retries.
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::RateLimited { .. } => true,
            Error::HttpStatus { status, .. } => {
                status.is_server_error() || *status == StatusCode::REQUEST_TIMEOUT
            }
            Error::Fetch { source, .. } => {
                source.is_timeout() || source.is_connect() || source.is_body()
            }
            Error::Parse { .. }
            | Error::Internal(_)
            | Error::InvalidSpider(_)
            | Error::WebDriver(_) => false,
        }
    }

    pub fn parse(url: &str, reason: impl Into<String>) -> Self {
        Error::Parse {
            url: url.to_string(),
            reason: reason.into(),
        }
    }
}

// `?` on a reqwest call still works; the URL comes from the error itself
// when reqwest recorded one.
impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        let url = err
            .url()
            .map(ToString::to_string)
            .unwrap_or_else(|| String::from("<unknown url>"));
        Error::Fetch { url, source: err }
    }
}

impl From<fantoccini::error::CmdError> for Error {
    fn from(err: fantoccini::error::CmdError) -> Self {
        Error::WebDriver(err.to_string())
    }
}

impl From<fantoccini::error::NewSessionError> for Error {
    fn from(err: fantoccini::error::NewSessionError) -> Self {
        Error::WebDriver(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryability_follows_the_variant() {
        let rate_limited = Error::RateLimited {
            url: "u".into(),
            status: StatusCode::TOO_MANY_REQUESTS,
            retry_after: Some(Duration::from_secs(7)),
        };
        assert!(rate_limited.is_retryable());
        assert!(rate_limited.to_string().ends_with("retry after 7s"));

        let server_error = Error::HttpStatus {
            url: "u".into(),
            status: StatusCode::BAD_GATEWAY,
        };
        assert!(server_error.is_retryable());

        let not_found = Error::HttpStatus {
            url: "u".into(),
            status: StatusCode::NOT_FOUND,
        };
        assert!(!not_found.is_retryable());

        assert!(!Error::parse("u", "no table").is_retryable());
        assert!(!Error::Internal("bug".into()).is_retryable());
    }
}
