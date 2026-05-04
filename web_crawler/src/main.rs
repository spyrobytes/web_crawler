/**
 * web_crawler_main
 *
 * This is the main test-drive version of the web crawler.
 *
 * Run with:
 * cargo run --package web_crawler --bin web_crawler_main
 *
 * check with:
 * cargo check --package web_crawler --bin web_crawler_main
 *
 * with clippy:
 * cargo clippy --package web_crawler --bin web_crawler_main
 *
 */
use reqwest::Client;
use select::document::Document;
use select::predicate::{Name, Predicate};
use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task;
use url::Url;

const MAX_PAGES: usize = 20;

// Define a trait for a generic web crawler component
trait CrawlerComponent {
    type Output;
    fn new() -> Self;
    async fn run(&self, url: &str) -> Result<Self::Output, Box<dyn Error + Send + Sync>>;
}

// Define a struct for URLs management
struct UrlManager {
    pending_urls: Arc<Mutex<VecDeque<String>>>,
    visited_urls: Arc<Mutex<HashSet<String>>>,
    max_pages: usize,
}

impl UrlManager {
    fn new(start_urls: Vec<String>, max_pages: usize) -> Self {
        Self {
            pending_urls: Arc::new(Mutex::new(VecDeque::from(start_urls))),
            visited_urls: Arc::new(Mutex::new(HashSet::new())),
            max_pages,
        }
    }

    // Watch out for possible infinite loops if url is not removed from the set
    // after being crawled
    fn get_next_url(&self) -> Option<String> {
        let mut pending_urls = self.pending_urls.lock().ok()?;
        let mut visited_urls = self.visited_urls.lock().ok()?;

        if visited_urls.len() >= self.max_pages {
            return None;
        }

        while let Some(url) = pending_urls.pop_front() {
            if visited_urls.insert(url.clone()) {
                return Some(url);
            }
        }

        None
    }

    fn add_urls(&self, urls: Vec<String>) {
        let Ok(mut pending_urls) = self.pending_urls.lock() else {
            return;
        };
        let Ok(visited_urls) = self.visited_urls.lock() else {
            return;
        };

        for url in urls {
            if !visited_urls.contains(&url) && !pending_urls.contains(&url) {
                pending_urls.push_back(url);
            }
        }
    }

    fn pending_count(&self) -> usize {
        self.pending_urls
            .lock()
            .map(|pending_urls| pending_urls.len())
            .unwrap_or(0)
    }

    fn visited_count(&self) -> usize {
        self.visited_urls
            .lock()
            .map(|visited_urls| visited_urls.len())
            .unwrap_or(0)
    }
}

impl Clone for UrlManager {
    fn clone(&self) -> Self {
        UrlManager {
            pending_urls: Arc::clone(&self.pending_urls),
            visited_urls: Arc::clone(&self.visited_urls),
            max_pages: self.max_pages,
        }
    }
}

// Define a struct and impl for Scraper
struct Scraper {
    http_client: Client,
}

impl CrawlerComponent for Scraper {
    type Output = Vec<String>; // List of URLs

    fn new() -> Self {
        Scraper {
            http_client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Building HTTP client"),
        }
    }

    async fn run(&self, url: &str) -> Result<Self::Output, Box<dyn Error + Send + Sync>> {
        let html = self.http_client.get(url).send().await?.text().await?;
        Ok(get_links_from_html(url, &html))
    }
}

// Define a struct and impl for Spider
struct Spider {
    scraper: Scraper,
    processors: Vec<Box<dyn Processor>>,
}

#[allow(dead_code)]
impl Spider {
    fn new() -> Self {
        Self {
            scraper: Scraper::new(),
            processors: vec![Box::new(DataProcessor)],
        }
    }

    fn add_processor<P: Processor + 'static>(&mut self, processor: P) {
        self.processors.push(Box::new(processor));
    }

    async fn crawl(&self, url: String) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let scraped_urls = self.scraper.run(&url).await?;

        for processor in &self.processors {
            processor.process(&scraped_urls)?;
        }

        Ok(scraped_urls)
    }
}

// Define a trait and a struct for Processors
trait Processor: Send + Sync {
    //fn process(&self, data: &Vec<String>) -> Result<(), Box<dyn Error>>;
    // instead of &Vec<String>, a slice &[String] could be used
    fn process(&self, data: &[String]) -> Result<(), Box<dyn Error + Send + Sync>>;
}

struct DataProcessor; // unit struct

impl Processor for DataProcessor {
    fn process(&self, data: &[String]) -> Result<(), Box<dyn Error + Send + Sync>> {
        println!("Discovered {} links", data.len());
        Ok(())
    }
}

// Extract links from HTML, normalize them against the page URL, remove
// duplicates, and ignore obvious non-page assets.
fn get_links_from_html(base_url: &str, html: &str) -> Vec<String> {
    let mut links = Document::from(html)
        .find(Name("a").or(Name("link")))
        .filter_map(|node| node.attr("href"))
        .filter_map(|href| normalize_url(base_url, href))
        .filter(|url| is_crawlable_url(url))
        .collect::<HashSet<String>>()
        .into_iter()
        .collect::<Vec<String>>();

    links.sort();
    links
}

fn normalize_url(base_url: &str, href: &str) -> Option<String> {
    let href = href.trim();

    if href.is_empty()
        || href.starts_with('#')
        || href.starts_with("mailto:")
        || href.starts_with("tel:")
        || href.starts_with("javascript:")
    {
        return None;
    }

    let base = Url::parse(base_url).ok()?;
    let mut url = base.join(href).ok()?;
    url.set_fragment(None);

    Some(url.to_string())
}

fn is_crawlable_url(url: &str) -> bool {
    let Ok(parsed_url) = Url::parse(url) else {
        return false;
    };

    if !matches!(parsed_url.scheme(), "http" | "https") {
        return false;
    }

    Path::new(parsed_url.path()).extension().is_none()
}

#[tokio::main] // so that we can use async/await
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a URL manager
    let start_urls = vec![
        "https://example.com".to_string(),
        "https://rust-lang.org".to_string(),
        "https://news.ycombinator.com".to_string(),
        "https://www.imdb.com".to_string(),
        "https://www.cvedetails.com/".to_string(),
    ];
    let url_manager = UrlManager::new(start_urls, MAX_PAGES);

    // Create a channel to communicate between tasks
    // with a buffer size of 32 messages
    // mpsc: multiple producer, single consumer
    // tx: transmitter, rx: receiver
    let (tx, mut rx) = mpsc::channel(32); // bounded channel

    // `Arc` because we need to share ownership between tasks
    let spider = Arc::new(Spider::new());
    while let Some(url) = url_manager.get_next_url() {
        let tx_clone = tx.clone();
        let spider_clone = Arc::clone(&spider);

        task::spawn(async move {
            let crawled_url = url.clone();
            match spider_clone.crawl(url).await {
                Ok(discovered_urls) => {
                    println!("Crawled successfully: {}", crawled_url);
                    if tx_clone.send(discovered_urls).await.is_err() {
                        eprintln!("Receiver dropped before crawl result could be sent");
                    }
                }
                Err(e) => {
                    eprintln!("Error crawling {}: {:?}", crawled_url, e);
                    if tx_clone.send(Vec::new()).await.is_err() {
                        eprintln!("Receiver dropped before crawl error could be sent");
                    }
                }
            }
        });

        let Some(discovered_urls) = rx.recv().await else {
            break;
        };

        url_manager.add_urls(discovered_urls);

        if url_manager.visited_count() >= MAX_PAGES || url_manager.pending_count() == 0 {
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_manager_returns_each_url_once() {
        let url_manager = UrlManager::new(
            vec![
                "https://example.com".to_string(),
                "https://example.com".to_string(),
            ],
            10,
        );

        assert_eq!(
            url_manager.get_next_url(),
            Some("https://example.com".to_string())
        );
        assert_eq!(url_manager.get_next_url(), None);
    }

    #[test]
    fn url_manager_adds_only_unvisited_urls() {
        let url_manager = UrlManager::new(vec!["https://example.com".to_string()], 10);

        assert_eq!(
            url_manager.get_next_url(),
            Some("https://example.com".to_string())
        );

        url_manager.add_urls(vec![
            "https://example.com".to_string(),
            "https://example.com/about".to_string(),
        ]);

        assert_eq!(
            url_manager.get_next_url(),
            Some("https://example.com/about".to_string())
        );
        assert_eq!(url_manager.get_next_url(), None);
    }

    #[test]
    fn normalize_url_resolves_relative_links() {
        assert_eq!(
            normalize_url("https://example.com/docs/index.html", "/about"),
            Some("https://example.com/about".to_string())
        );
        assert_eq!(
            normalize_url("https://example.com/docs/index.html", "guide"),
            Some("https://example.com/docs/guide".to_string())
        );
    }

    #[test]
    fn normalize_url_ignores_non_page_links() {
        assert_eq!(normalize_url("https://example.com", "#top"), None);
        assert_eq!(
            normalize_url("https://example.com", "mailto:test@example.com"),
            None
        );
        assert_eq!(
            normalize_url("https://example.com", "javascript:void(0)"),
            None
        );
    }

    #[test]
    fn get_links_from_html_extracts_unique_crawlable_links() {
        let html = r#"
            <html>
                <body>
                    <a href="/about">About</a>
                    <a href="/about#team">About duplicate</a>
                    <a href="https://example.com/image.png">Image</a>
                    <a href="mailto:test@example.com">Email</a>
                    <link href="/feed">
                </body>
            </html>
        "#;

        assert_eq!(
            get_links_from_html("https://example.com", html),
            vec![
                "https://example.com/about".to_string(),
                "https://example.com/feed".to_string(),
            ]
        );
    }
}
