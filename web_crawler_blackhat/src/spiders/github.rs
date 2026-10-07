use crate::error::Error;
use async_trait::async_trait;
use regex::Regex;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct GitHubSpider {
    http_client: Client,
    base_url: String,
    page_regex: Regex,
    expected_number_of_results: usize,
}

impl GitHubSpider {
    pub const NAME: &'static str = "github";

    pub fn new() -> Self {
        Self::with_base_url("https://api.github.com")
    }

    /// Point the spider at another host. Tests use this to crawl a local
    /// mock server instead of the real API.
    pub fn with_base_url(base_url: &str) -> Self {
        let http_timeout = Duration::from_secs(6);
        let mut headers = header::HeaderMap::new();
        headers.insert(
            "Accept",
            header::HeaderValue::from_static("application/vnd.github.v3+json"),
        );

        let http_client = Client::builder()
            .timeout(http_timeout)
            .default_headers(headers)
            .user_agent(
                "Mozilla/5.0 (Windows NT 6.1; Win64; x64; rv:47.0) Gecko/20100101 Firefox/47.0",
            )
            .build()
            .expect("spiders/github: Building HTTP client");

        let page_regex =
            Regex::new(".*page=([0-9]*).*").expect("spiders/github: Compiling page regex");

        GitHubSpider {
            http_client,
            base_url: base_url.trim_end_matches('/').to_string(),
            page_regex,
            expected_number_of_results: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubItem {
    login: String,
    id: u64,
    node_id: String,
    html_url: String,
    avatar_url: String,
}

#[async_trait]
impl super::Spider for GitHubSpider {
    type Item = GitHubItem;

    fn name(&self) -> String {
        String::from(Self::NAME)
    }

    fn start_urls(&self) -> Vec<String> {
        vec![format!(
            "{}/orgs/google/public_members?per_page=100&page=1",
            self.base_url
        )]
    }

    async fn scrape(&self, url: String) -> Result<(Vec<GitHubItem>, Vec<String>), Error> {
        let items: Vec<GitHubItem> = super::get_json(&self.http_client, &url).await?;

        let next_pages_links = if items.len() == self.expected_number_of_results {
            // We built this URL ourselves, so a missing page parameter is a
            // bug. Surface it as an error rather than a panic: a panic would
            // kill the scraper task and hang the crawler.
            let missing_page =
                || Error::Internal(format!("spider/github: no page parameter in {url}"));
            let captures = self.page_regex.captures(&url).ok_or_else(missing_page)?;
            let old_page_number = captures
                .get(1)
                .ok_or_else(missing_page)?
                .as_str()
                .to_string();
            let mut new_page_number = old_page_number
                .parse::<usize>()
                .map_err(|_| Error::Internal("spider/github: parsing page number".to_string()))?;
            new_page_number += 1;

            let next_url = url.replace(
                format!("&page={}", old_page_number).as_str(),
                format!("&page={}", new_page_number).as_str(),
            );
            vec![next_url]
        } else {
            Vec::new()
        };

        Ok((items, next_pages_links))
    }

    async fn process(&self, item: Self::Item) -> Result<(), Error> {
        println!("{}, {}, {}", item.login, item.html_url, item.avatar_url);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spiders::Spider;
    use reqwest::StatusCode;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    fn members_json(count: usize) -> String {
        let members: Vec<serde_json::Value> = (0..count)
            .map(|i| {
                serde_json::json!({
                    "login": format!("user{i}"),
                    "id": i,
                    "node_id": format!("node{i}"),
                    "html_url": format!("https://github.com/user{i}"),
                    "avatar_url": format!("https://avatars.example/{i}"),
                })
            })
            .collect();
        serde_json::Value::Array(members).to_string()
    }

    async fn members_server(status: u16, body: &str, headers: &[(&str, &str)]) -> MockServer {
        let server = MockServer::start().await;
        let mut response = ResponseTemplate::new(status).set_body_string(body);
        for (name, value) in headers {
            response = response.insert_header(*name, *value);
        }
        Mock::given(method("GET"))
            .and(path("/orgs/google/public_members"))
            .respond_with(response)
            .mount(&server)
            .await;
        server
    }

    async fn scrape_first_page(
        server: &MockServer,
    ) -> Result<(Vec<GitHubItem>, Vec<String>), Error> {
        let spider = GitHubSpider::with_base_url(&server.uri());
        let start = spider.start_urls().remove(0);
        spider.scrape(start).await
    }

    #[test]
    fn spider_name_matches_cli_name() {
        assert_eq!(GitHubSpider::new().name(), GitHubSpider::NAME);
    }

    #[tokio::test]
    async fn a_full_page_links_to_the_next_one() {
        let server = members_server(200, &members_json(100), &[]).await;

        let (items, next) = scrape_first_page(&server).await.expect("json parses");

        assert_eq!(items.len(), 100);
        assert_eq!(items[0].login, "user0");
        assert_eq!(
            next,
            vec![format!(
                "{}/orgs/google/public_members?per_page=100&page=2",
                server.uri()
            )]
        );
    }

    #[tokio::test]
    async fn a_short_page_ends_pagination() {
        let server = members_server(200, &members_json(3), &[]).await;

        let (items, next) = scrape_first_page(&server).await.expect("json parses");

        assert_eq!(items.len(), 3);
        assert!(next.is_empty());
    }

    // GitHub signals an exhausted quota with 403 and a header, not 429.
    #[tokio::test]
    async fn an_exhausted_quota_is_a_rate_limit() {
        let server = members_server(
            403,
            r#"{"message":"API rate limit exceeded"}"#,
            &[("x-ratelimit-remaining", "0")],
        )
        .await;

        let err = scrape_first_page(&server)
            .await
            .expect_err("403 is an error");

        assert!(
            matches!(
                err,
                Error::RateLimited {
                    status: StatusCode::FORBIDDEN,
                    ..
                }
            ),
            "{err:?}"
        );
        assert!(err.is_retryable());
    }

    // Before the status check, this body reached `.json()` and surfaced as a
    // confusing decode error; now it is a plain non-2xx.
    #[tokio::test]
    async fn a_plain_forbidden_is_not_a_rate_limit() {
        let server = members_server(403, "nope", &[]).await;

        let err = scrape_first_page(&server)
            .await
            .expect_err("403 is an error");

        assert!(
            matches!(
                err,
                Error::HttpStatus {
                    status: StatusCode::FORBIDDEN,
                    ..
                }
            ),
            "{err:?}"
        );
        assert!(!err.is_retryable());
    }

    #[tokio::test]
    async fn a_non_json_body_is_a_parse_error() {
        let server = members_server(200, "<html>maintenance</html>", &[]).await;

        let err = scrape_first_page(&server)
            .await
            .expect_err("html is not json");

        assert!(matches!(err, Error::Parse { .. }), "{err:?}");
    }
}
