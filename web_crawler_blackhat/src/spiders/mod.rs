use crate::error::Error;
use async_trait::async_trait;
use reqwest::{header, Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::time::Duration;

pub mod cvedetails;
pub mod github;
pub mod quotes;

/// GET `url` and return the response only if it is a 2xx.
///
/// `reqwest` treats a 404 or a 429 as a perfectly good response; without this
/// check a rate-limit page would be handed to the HTML or JSON parser and
/// show up as a confusing parse error, and pagination would stop silently.
pub(crate) async fn get_checked(client: &Client, url: &str) -> Result<Response, Error> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|source| Error::Fetch {
            url: url.to_string(),
            source,
        })?;
    check_status(url, response)
}

/// GET `url` and return its body as text.
pub(crate) async fn get_text(client: &Client, url: &str) -> Result<String, Error> {
    get_checked(client, url)
        .await?
        .text()
        .await
        .map_err(|source| Error::Fetch {
            url: url.to_string(),
            source,
        })
}

/// GET `url` and decode its body as JSON. A body that is not the expected
/// JSON is a `Parse` error, not a fetch error: the request worked, the page
/// is not what the spider thinks it is.
pub(crate) async fn get_json<T: DeserializeOwned>(client: &Client, url: &str) -> Result<T, Error> {
    let body = get_text(client, url).await?;
    serde_json::from_str(&body).map_err(|err| Error::parse(url, format!("invalid JSON: {err}")))
}

fn check_status(url: &str, response: Response) -> Result<Response, Error> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let header_str = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
    };
    let retry_after = header_str(header::RETRY_AFTER.as_str())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs);
    let quota_exhausted = header_str("x-ratelimit-remaining") == Some("0");

    // 429 is the standard signal. GitHub answers 403 with an exhausted quota
    // header instead, and a 503 that names a Retry-After is a polite
    // "come back later" rather than an outage.
    let rate_limited = status == StatusCode::TOO_MANY_REQUESTS
        || (status == StatusCode::FORBIDDEN && quota_exhausted)
        || (status == StatusCode::SERVICE_UNAVAILABLE && retry_after.is_some());

    if rate_limited {
        Err(Error::RateLimited {
            url: url.to_string(),
            status,
            retry_after,
        })
    } else {
        Err(Error::HttpStatus {
            url: url.to_string(),
            status,
        })
    }
}

#[async_trait]
pub trait Spider: Send + Sync {
    type Item;

    fn name(&self) -> String;
    fn start_urls(&self) -> Vec<String>;
    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Error>;
    async fn process(&self, item: Self::Item) -> Result<(), Error>;
}
