/**
 * web_crawler_main
 *
 * This is the boilerplate code for the web crawler.
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
 * ISSUES:
 * 1. When run, the program does not terminate. Specifically, it got stuck
 * in an infinite loop. This is because the `UrlManager::get_next_url()` method
 * does not remove the visited URL from the set of URLs. This is a bug.
 * 
 * See version 2 for a fix (web_crawler_v2.rs)
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

    // Watch out for possible infinite loops
    fn get_next_url(&self) -> Option<String> {
        // lock the mutex and unwrap the result
        // Must lock the mutex before modifying the data
        let urls = self.urls.lock().unwrap(); 
        //urls.iter().cloned().next() // eager cloning
        urls.iter().next().cloned() // lazy cloning
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
    // vector of trait objects
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
trait Processor: Send + Sync {
    //fn process(&self, data: &Vec<String>) -> Result<(), Box<dyn Error>>;
    // instead of &Vec<String>, a slice &[String] could be used
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
    // Create a URL manager
    let start_urls = HashSet::from(["https://example.com".to_string()]);
    let url_manager = UrlManager::new(start_urls);

    // Create a channel to communicate between tasks
    // with a buffer size of 32 messages
    // mpsc: multiple producer, single consumer
    // tx: transmitter, rx: receiver
    let (tx, mut rx) = mpsc::channel(32);

    // let spider = Spider::new();
    let spider = Arc::new(Spider::new());
    let url_manager_clone = url_manager.clone();

    // Spawn a task to crawl each URL
    // task is a lightweight thread managed by tokio runtime
    task::spawn(async move {
        while let Some(url) = url_manager_clone.get_next_url() {
            let tx_clone = tx.clone();
            let spider_clone = Arc::clone(&spider);

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
