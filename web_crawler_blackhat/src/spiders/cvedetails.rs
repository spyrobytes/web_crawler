/**
 * CVE Details Spider
 *
 * CVE - Common Vulnerabilities and Exposures database of publicly
 * disclosed information security issues.
 *
 * This spider crawls the CVE Details website and extracts information about
 * vulnerabilities.
 */
use crate::error::Error;
use crate::links;
use async_trait::async_trait;
use reqwest::Client;
use select::{
    document::Document,
    node::Node,
    predicate::{Attr, Class, Name, Predicate},
};
use std::time::Duration;

pub struct CveDetailsSpider {
    http_client: Client,
    base_url: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Cve {
    name: String,
    url: String,
    cwe_id: Option<String>,
    cwe_url: Option<String>,
    vulnerability_type: String,
    publish_date: String,
    update_date: String,
    score: f32,
    access: String,
    complexity: String,
    authentication: String,
    confidentiality: String,
    integrity: String,
    availability: String,
}

impl CveDetailsSpider {
    pub const NAME: &'static str = "cvedetails";

    pub fn new() -> Self {
        Self::with_base_url("https://www.cvedetails.com")
    }

    /// Point the spider at another host. Tests use this to crawl a local
    /// mock server instead of the real site.
    pub fn with_base_url(base_url: &str) -> Self {
        let http_timeout = Duration::from_secs(6);
        let http_client = Client::builder()
            .timeout(http_timeout)
            .build()
            .expect("spiders/cvedetails: Building HTTP client");

        CveDetailsSpider {
            http_client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    // Parse one row of the vulnerability table.
    //
    // Every `?` below used to be an `unwrap()`. A panic here does not crash
    // the program: tokio catches it at the task boundary, which kills the
    // scraper task and leaves the crawler's control loop waiting forever.
    // Returning an error instead lets the crawler log it and move on.
    fn parse_row(&self, url: &str, row: Node<'_>) -> Result<Cve, Error> {
        let columns: Vec<Node<'_>> = row.find(Name("td")).collect();

        // Column layout, by position. Columns 0 (row number), 3 (number of
        // exploits) and 8 (gained access level) are not used.
        let cve_cell = column(&columns, 1, "CVE", url)?;
        let (cve_name, cve_href) =
            link_in(cve_cell).ok_or_else(|| Error::parse(url, "CVE column has no link"))?;
        let cve_url = links::resolve(url, cve_href)
            .ok_or_else(|| Error::parse(url, format!("CVE link {cve_href:?} is unusable")))?;

        // The CWE column is legitimately empty for some entries, and a CWE
        // link we cannot resolve is not worth losing the row over.
        let cwe = link_in(column(&columns, 2, "CWE", url)?)
            .and_then(|(id, href)| links::resolve(url, href).map(|href| (id, href)));

        let vulnerability_type = column_text(&columns, 4, "vulnerability type", url)?;
        let publish_date = column_text(&columns, 5, "publish date", url)?;
        let update_date = column_text(&columns, 6, "update date", url)?;

        let score_text = column_text(&columns, 7, "score", url)?;
        let score: f32 = score_text.parse().map_err(|_| {
            Error::parse(
                url,
                format!("{cve_name} has an unparseable score {score_text:?}"),
            )
        })?;

        let access = column_text(&columns, 9, "access", url)?;
        let complexity = column_text(&columns, 10, "complexity", url)?;
        let authentication = column_text(&columns, 11, "authentication", url)?;
        let confidentiality = column_text(&columns, 12, "confidentiality", url)?;
        let integrity = column_text(&columns, 13, "integrity", url)?;
        let availability = column_text(&columns, 14, "availability", url)?;

        Ok(Cve {
            name: cve_name,
            url: cve_url,
            cwe_id: cwe.as_ref().map(|cwe| cwe.0.clone()),
            cwe_url: cwe.as_ref().map(|cwe| cwe.1.clone()),
            vulnerability_type,
            publish_date,
            update_date,
            score,
            access,
            complexity,
            authentication,
            confidentiality,
            integrity,
            availability,
        })
    }
}

// The column at `index`, naming the column in the error so that a layout
// change on the site is easy to diagnose from the logs.
fn column<'a>(
    columns: &[Node<'a>],
    index: usize,
    name: &str,
    url: &str,
) -> Result<Node<'a>, Error> {
    columns.get(index).copied().ok_or_else(|| {
        Error::parse(
            url,
            format!(
                "CVE row has no {name} column (expected at index {index}, found {} columns)",
                columns.len()
            ),
        )
    })
}

// The trimmed text of the column at `index`.
fn column_text(columns: &[Node<'_>], index: usize, name: &str, url: &str) -> Result<String, Error> {
    column(columns, index, name, url).map(|cell| cell.text().trim().to_string())
}

// The first link inside `cell`, as (link text, raw href).
fn link_in(cell: Node<'_>) -> Option<(String, &str)> {
    let link = cell.find(Name("a")).next()?;
    let href = link.attr("href")?;
    Some((link.text().trim().to_string(), href))
}

#[async_trait]
impl super::Spider for CveDetailsSpider {
    type Item = Cve;

    fn name(&self) -> String {
        String::from(Self::NAME)
    }

    fn start_urls(&self) -> Vec<String> {
        vec![format!(
            "{}/vulnerability-list/vulnerabilities.html",
            self.base_url
        )]
    }

    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Error> {
        log::info!("visiting: {}", url);

        let http_res = super::get_text(&self.http_client, &url).await?;
        let mut items = Vec::new();

        let document = Document::from(http_res.as_str());

        // A list page without the table is not "no vulnerabilities today";
        // it is a page we do not understand. Saying so beats reporting a
        // successful crawl of zero items.
        if document.find(Attr("id", "vulnslisttable")).next().is_none() {
            return Err(Error::parse(
                &url,
                "no #vulnslisttable found; the site may have changed its markup",
            ));
        }

        let rows = document.find(Attr("id", "vulnslisttable").descendant(Class("srrowns")));
        for row in rows {
            // One malformed row should not cost us the rest of the page or
            // its pagination links, so we log it and keep going.
            match self.parse_row(&url, row) {
                Ok(cve) => items.push(cve),
                Err(err) => log::warn!("{}", err),
            }
        }

        let next_pages_links = document
            .find(Attr("id", "pagingb").descendant(Name("a")))
            .filter_map(|n| n.attr("href"))
            .filter_map(|href| links::resolve(&url, href))
            .collect::<Vec<String>>();

        Ok((items, next_pages_links))
    }

    // For now we just print the CVEs, but we could save them to a database
    async fn process(&self, item: Self::Item) -> Result<(), Error> {
        println!("{:?}", item);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spiders::Spider;

    const URL: &str = "https://www.cvedetails.com/vulnerability-list/vulnerabilities.html";

    fn table(row_cells: &str) -> String {
        format!(r#"<table id="vulnslisttable"><tr class="srrowns">{row_cells}</tr></table>"#)
    }

    const FULL_ROW: &str = r#"
        <td>1</td>
        <td><a href="/cve/CVE-2024-0001/">CVE-2024-0001</a></td>
        <td><a href="//www.cvedetails.com/cwe-details/79/">79</a></td>
        <td></td>
        <td>XSS</td>
        <td>2024-01-01</td>
        <td>2024-01-02</td>
        <td>7.5</td>
        <td>None</td>
        <td>Remote</td>
        <td>Low</td>
        <td>Not required</td>
        <td>Partial</td>
        <td>Partial</td>
        <td>Partial</td>
    "#;

    fn parse(cells: &str) -> Result<Cve, Error> {
        let html = table(cells);
        let document = Document::from(html.as_str());
        let row = document
            .find(Attr("id", "vulnslisttable").descendant(Class("srrowns")))
            .next()
            .expect("test html has one row");
        CveDetailsSpider::new().parse_row(URL, row)
    }

    #[test]
    fn parses_a_well_formed_row() {
        let cve = parse(FULL_ROW).expect("row parses");

        assert_eq!(cve.name, "CVE-2024-0001");
        assert_eq!(cve.url, "https://www.cvedetails.com/cve/CVE-2024-0001/");
        assert_eq!(cve.cwe_id.as_deref(), Some("79"));
        assert_eq!(
            cve.cwe_url.as_deref(),
            Some("https://www.cvedetails.com/cwe-details/79/")
        );
        assert_eq!(cve.vulnerability_type, "XSS");
        assert_eq!(cve.score, 7.5);
        assert_eq!(cve.availability, "Partial");
    }

    #[test]
    fn missing_cwe_link_is_not_an_error() {
        let cells = FULL_ROW.replace(
            r#"<td><a href="//www.cvedetails.com/cwe-details/79/">79</a></td>"#,
            "<td></td>",
        );
        let cve = parse(&cells).expect("row parses");

        assert_eq!(cve.cwe_id, None);
        assert_eq!(cve.cwe_url, None);
    }

    #[test]
    fn short_row_is_an_error_naming_the_column() {
        // Only the first eight cells: everything from "access" onwards is gone.
        let cells: String = FULL_ROW
            .split("</td>")
            .take(8)
            .map(|cell| format!("{cell}</td>"))
            .collect();

        let err = parse(&cells).expect_err("short row is rejected");
        let message = err.to_string();

        assert!(message.contains("access"), "{message}");
        assert!(message.contains(URL), "{message}");
    }

    #[test]
    fn unparseable_score_is_an_error() {
        let cells = FULL_ROW.replace("<td>7.5</td>", "<td>n/a</td>");

        let err = parse(&cells).expect_err("bad score is rejected");
        assert!(err.to_string().contains("score"), "{err}");
    }

    #[test]
    fn row_without_cve_link_is_an_error() {
        let cells = FULL_ROW.replace(
            r#"<td><a href="/cve/CVE-2024-0001/">CVE-2024-0001</a></td>"#,
            "<td>CVE-2024-0001</td>",
        );

        let err = parse(&cells).expect_err("row without a link is rejected");
        assert!(err.to_string().contains("no link"), "{err}");
    }

    #[test]
    fn spider_name_matches_cli_name() {
        assert_eq!(CveDetailsSpider::new().name(), CveDetailsSpider::NAME);
    }

    // ---- the fetch path, against a local mock server ----

    use reqwest::StatusCode;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    async fn list_page_server(status: u16, body: &str, headers: &[(&str, &str)]) -> MockServer {
        let server = MockServer::start().await;
        let mut response = ResponseTemplate::new(status).set_body_string(body);
        for (name, value) in headers {
            response = response.insert_header(*name, *value);
        }
        Mock::given(method("GET"))
            .and(path("/vulnerability-list/vulnerabilities.html"))
            .respond_with(response)
            .mount(&server)
            .await;
        server
    }

    async fn scrape_start_page(server: &MockServer) -> Result<(Vec<Cve>, Vec<String>), Error> {
        let spider = CveDetailsSpider::with_base_url(&server.uri());
        let start = spider.start_urls().remove(0);
        spider.scrape(start).await
    }

    #[tokio::test]
    async fn scrape_parses_rows_and_pagination_from_an_http_response() {
        let page = format!(
            r#"{}<div id="pagingb"><a href="/vulnerability-list.php?page=2">2</a></div>"#,
            table(FULL_ROW)
        );
        let server = list_page_server(200, &page, &[]).await;

        let (items, next) = scrape_start_page(&server).await.expect("page parses");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "CVE-2024-0001");
        assert_eq!(items[0].url, format!("{}/cve/CVE-2024-0001/", server.uri()));
        assert_eq!(
            next,
            vec![format!("{}/vulnerability-list.php?page=2", server.uri())]
        );
    }

    #[tokio::test]
    async fn server_error_is_a_retryable_http_status() {
        let server = list_page_server(503, "down", &[]).await;

        let err = scrape_start_page(&server)
            .await
            .expect_err("503 is an error");

        assert!(
            matches!(
                err,
                Error::HttpStatus {
                    status: StatusCode::SERVICE_UNAVAILABLE,
                    ..
                }
            ),
            "{err:?}"
        );
        assert!(err.is_retryable());
    }

    #[tokio::test]
    async fn rate_limit_carries_retry_after() {
        let server = list_page_server(429, "slow down", &[("Retry-After", "7")]).await;

        let err = scrape_start_page(&server)
            .await
            .expect_err("429 is an error");

        match err {
            Error::RateLimited {
                status,
                retry_after,
                ..
            } => {
                assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
                assert_eq!(retry_after, Some(Duration::from_secs(7)));
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn page_without_the_table_is_a_parse_error_not_a_success() {
        let server = list_page_server(200, "<html><body>redesigned</body></html>", &[]).await;

        let err = scrape_start_page(&server)
            .await
            .expect_err("missing table is an error");

        assert!(matches!(err, Error::Parse { .. }), "{err:?}");
        assert!(!err.is_retryable());
        assert!(err.to_string().contains("vulnslisttable"), "{err}");
    }
}
