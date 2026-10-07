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
    pub fn new() -> Self {
        let http_timeout = Duration::from_secs(6);
        let http_client = Client::builder()
            .timeout(http_timeout)
            .build()
            .expect("spiders/cvedetails: Building HTTP client");

        CveDetailsSpider { http_client }
    }

    fn normalize_url(&self, url: &str) -> String {
        let url = url.trim();

        if url.starts_with("//www.cvedetails.com") {
            return format!("https:{}", url);
        } else if url.starts_with('/') {
            return format!("https://www.cvedetails.com{}", url);
        }

        url.to_string()
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
        let (cve_name, cve_href) = link_in(cve_cell)
            .ok_or_else(|| Error::Internal(format!("{url}: CVE column has no link")))?;
        let cve_url = self.normalize_url(cve_href);

        // The CWE column is legitimately empty for some entries.
        let cwe = link_in(column(&columns, 2, "CWE", url)?)
            .map(|(id, href)| (id, self.normalize_url(href)));

        let vulnerability_type = column_text(&columns, 4, "vulnerability type", url)?;
        let publish_date = column_text(&columns, 5, "publish date", url)?;
        let update_date = column_text(&columns, 6, "update date", url)?;

        let score_text = column_text(&columns, 7, "score", url)?;
        let score: f32 = score_text.parse().map_err(|_| {
            Error::Internal(format!(
                "{url}: {cve_name} has an unparseable score {score_text:?}"
            ))
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
        Error::Internal(format!(
            "{url}: CVE row has no {name} column (expected at index {index}, found {} columns)",
            columns.len()
        ))
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
        String::from("cvedetails")
    }

    fn start_urls(&self) -> Vec<String> {
        vec!["https://www.cvedetails.com/vulnerability-list/vulnerabilities.html".to_string()]
    }

    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Error> {
        log::info!("visiting: {}", url);

        let http_res = self.http_client.get(&url).send().await?.text().await?;
        let mut items = Vec::new();

        let document = Document::from(http_res.as_str());

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
            .map(|url| self.normalize_url(url))
            .collect::<Vec<String>>();

        Ok((items, next_pages_links))
    }

    // For now we just print the CVEs, but we could save them to a database
    async fn process(&self, item: Self::Item) -> Result<(), Error> {
        println!("{:?}", item);

        Ok(())
    }
}

// Moved to impl block of CveDetailsSpider
// impl CveDetailsSpider {
//     fn normalize_url(&self, url: &str) -> String {
//         let url = url.trim();

//         if url.starts_with("//www.cvedetails.com") {
//             return format!("https:{}", url);
//         } else if url.starts_with('/') {
//             return format!("https://www.cvedetails.com{}", url);
//         }

//         url.to_string()
//     }
// }

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
        assert_eq!(CveDetailsSpider::new().name(), "cvedetails");
    }
}
