/**
 * Web Crawler v2
 * 
 * ChangeLog:
 * 
 * 1. Fix the bug in `UrlManager::get_next_url` by cloning the `urls` set
 * 
 * TODO: The next milestone is to implement the `Scraper` struct.
 * 1. Complete the implementation of the `Scraper` struct:
 *   - Implement the `CrawlerComponent` trait for `Scraper`
 *   - Implement the `run` method for `Scraper`
 *
 * 
 * 
 */
use std::collections::HashSet;
use std::error::Error;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task;

// Define a trait for a generic web crawler component
trait CrawlerComponent {
    type Output;
    fn new() -> Self;
    fn run(&self) -> Self::Output;
}

// Define a struct for URLs management
struct UrlManager {
    urls: Arc<Mutex<HashSet<String>>>,
}

impl UrlManager {
    fn new(start_urls: HashSet<String>) -> Self {
        Self {
            urls: Arc::new(Mutex::new(start_urls)),
        }
    }

    // Watch out for possible infinite loops if url is not removed from the set
    // after being crawled
    fn get_next_url(&self) -> Option<String> {

        // Must acquire the lock before accessing the data
        let mut urls = self.urls.lock().unwrap();

        // if there is a url, remove it from the set and return it
        // otherwise return None
        if let Some(url) = urls.iter().next().cloned() {
            urls.remove(&url);
            Some(url)
        } else {
            None
        }

    }
}

impl Clone for UrlManager {
    fn clone(&self) -> Self {
        UrlManager {
            urls: Arc::clone(&self.urls),
        }
    }
}

// Define a struct and impl for Scraper
struct Scraper; // unit struct

impl CrawlerComponent for Scraper {
    type Output = Vec<String>; // List of URLs

    fn new() -> Self {
        Scraper
    }

    fn run(&self) -> Self::Output {
        // Placeholder for scraping logic
        vec![]
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
            processors: vec![],
        }
    }

    fn add_processor<P: Processor + 'static>(&mut self, processor: P) {
        self.processors.push(Box::new(processor));
    }

    async fn crawl(&self, _url: String) -> Result<(), Box<dyn Error>> {
        let scraped_urls = self.scraper.run();

        for processor in &self.processors {
            processor.process(&scraped_urls)?;
        }

        Ok(())
    }
}

// Define a trait and a struct for Processors
// Processor is a trait that can be implemented by any type
// that implements Send and Sync. This is because the type
// will be sent across threads and shared between threads.
trait Processor: Send + Sync {

    fn process(&self, data: &[String]) -> Result<(), Box<dyn Error>>;
}

struct DataProcessor; // unit struct

impl Processor for DataProcessor {
    fn process(&self, _data: &[String]) -> Result<(), Box<dyn Error>> {
        // Placeholder for processing logic
        Ok(())
    }
}

#[tokio::main] // so that we can use async/await
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a URL manager with some start URLs
    let start_urls = HashSet::from([
        "https://example.com".to_string(), 
        "https://rust-lang.org".to_string(),
        "https://news.ycombinator.com".to_string(),
        "https://www.imdb.com".to_string(),
        "https://www.cvedetails.com/".to_string(),
        ]);
    let url_manager = UrlManager::new(start_urls);

    // Create a channel to communicate between tasks
    // with a buffer size of 32 messages
    // mpsc: multiple producer, single consumer
    // tx: transmitter, rx: receiver
    let (tx, mut rx) = mpsc::channel(32); // unbounded channel (non-blocking)

    // `Arc` because we need to share ownership between tasks
    let spider = Arc::new(Spider::new());
    let url_manager_clone = url_manager.clone();

    // Spawn a task to crawl each URL
    // task is a lightweight, non-blocking thread managed by tokio runtime
    task::spawn(async move {
        while let Some(url) = url_manager_clone.get_next_url() {
            let tx_clone = tx.clone();
            let spider_clone = Arc::clone(&spider);

            // async block: returns a future instead of blocking the current thread
            task::spawn(async move {
                match spider_clone.crawl(url).await {
                    Ok(_) => println!("Crawled successfully"),
                    Err(e) => eprintln!("Error crawling: {:?}", e),
                }
                // Signal completion
                tx_clone.send(()).await.unwrap();
            });
        }
    });

    // Await all spawned tasks to complete
    while rx.recv().await.is_some() {}

    Ok(())
}