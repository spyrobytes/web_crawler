/**
 * web crawler for wikipedia
 * 
 * This is a simple web crawler that crawls wikipedia pages concurrently.
 * 
 * Goals: using threads to speed up a parallel fetch of pages on the web.
 * 
 * It is a simple web crawler that:
 * - Uses wikipedia crate to fetch pages
 * - Processes page content
 * - Collect timing metrics
 * - Concurrent page processing with rayon
 * - Demonstrates crate usage and concurrency in Rust
 */
// Import crates
use rayon::prelude::*;
use wikipedia::http::default::Client;
use wikipedia::{Page, Wikipedia};

struct ProcessedPage {
   title: String,
   data: String,
}

// Basketball stars
const PAGES: [&str; 9] = [
    "Giannis Antetokounmpo",
    "James Harden",
    "Russell Westbrook",
    "Stephen Curry",
    "Kevin Durant",
    "LeBron James",
    "Kobi Bryant",
    "Michael Jordan",
    "Shaquille O'Neal",
];

// Define a function to process a page
fn process_page(page: &Page<Client>) -> ProcessedPage {
    let title = page.get_title().unwrap();
    let content = page.get_content().unwrap();
    ProcessedPage {
        title, // title: title,
        data: content,
    }
}

// In main we time how long it takes to process the pages and 
// total time
fn main() {
    // start timer
    let start = std::time::Instant::now();
    let wikipedia = Wikipedia::<Client>::default();
    let pages: Vec<_> = PAGES
        .par_iter() // parallel iterator (without thread just use .iter())
        .map(|&p| wikipedia.page_from_title(p.to_string()))
        .collect();

    // process the pages
    let processed_pages: Vec<ProcessedPage> = pages.par_iter().map(process_page).collect();
    for page in processed_pages {
        // time how long it takes to process each page
        let start_page = std::time::Instant::now();

        println!("Title: {}", page.title.as_str());
        // grab the first sentence of the page
        let first_sentence = page.data.split('.').next().unwrap();
        println!("First sentence: {}", first_sentence);
        // count the number of words in the page
        let word_count = page.data.split_whitespace().count();
        println!("Word count: {}", word_count);
        // print time it took to process the page
        println!("Page time: {:?}", start_page.elapsed());
    }

    // descriptive statistics of: total time, average time per page, and 
    // total number of pages
    println!("Total time: {:?}", start.elapsed());
    println!(
        "Average time per page: {:?}",
        start.elapsed() / PAGES.len() as u32
    );
    println!("Total number of pages: {}", PAGES.len());
    println!("Number of threads: {}", rayon::current_num_threads());

}
