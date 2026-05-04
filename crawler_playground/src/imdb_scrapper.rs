/**
 * IMDB and Hacker News Scrapper
 *
 * Get the top 100 movies (by user rating) from IMDB
 *
 * @SEE https://www.scrapingbee.com/blog/web-scraping-rust/
 * @SEE https://github.com/kxzk/scraping-with-rust
 * 
 * Run with:
 * cargo run --package crawler_playground --bin imdb_web_scraper
 * 
 * check with:
 * cargo check --package crawler_playground --bin imdb_web_scraper
 * 
 * clippy with:
 * cargo clippy --package crawler_playground --bin imdb_web_scraper
 *
 */
use scraper::{Html, Selector};
use select::document::Document;
use select::predicate::{Class, Name, Predicate};

fn main() {
    const URL: &str =
        "https://www.imdb.com/search/title/?groups=top_100&sort=user_rating,desc&count=100";
    const HACKER_NEWS_URL: &str = "https://news.ycombinator.com";

    let response = reqwest::blocking::get(URL).unwrap().text().unwrap();

    let document = Html::parse_document(&response);

    // construct a CSS selector that will grab the title of each movie
    let title_selector = Selector::parse("div.ipc-title>a>h3").unwrap();

    // query the document with our selector, grab the text from each
    let titles = document
        .select(&title_selector)
        .map(|x| x.inner_html());

    // titles is an iterator that can be `zipped` and iterated over
    // to print out the rank and title of each movie
    // 10 titles are printed here
    titles
        .zip(1..11)
        .for_each(|(item, number)| println!("{}. {}", number, item));

    println!("\n");

    // test the get_list_of_text function
    get_list_of_text(HACKER_NEWS_URL, "span.titleline > a");

    println!("\n");

    // test the hacker_news function
    hacker_news(HACKER_NEWS_URL);
}

// A function that takes a `url` and a `selector` as parameters, and returns
// a list of text values matching the selector for the given url.
fn get_list_of_text(url: &str, selector: &str) {
    let response = reqwest::blocking::get(url).unwrap().text().unwrap();

    let document = Html::parse_document(&response);

    // query the document with CSS selectors
    let selector = Selector::parse(selector).unwrap();

    // Now apply the query to the parsed document
    let result = document.select(&selector).map(|x| x.inner_html());

    // print the first 10 matches
    result
        .zip(1..11)
        .for_each(|(item, number)| println!("{}. {}", number, item));
}

// Using the `select` crate for more complex queries
// We want to scrape the hacker news website and print out the rank,
// story headline and the url
fn hacker_news(url: &str) {

    let resp = reqwest::blocking::get(url).unwrap();
    // abort if the status is not a success
    assert!(resp.status().is_success());

    let document = Document::from_read(resp).unwrap();

    // finding all instances of our class of interest
    for node in document.find(Class("athing")) {
        // grabbing the story rank
        let rank = node.find(Class("rank")).next().unwrap();
        // finding class, then selecting article title
        let story = node
            .find(Class("title").descendant(Name("a")))
            .next()
            .unwrap()
            .text();
        // printing out | rank | story headline
        println!("\n | {} | {}\n", rank.text(), story);
        // same as above
        let url = node
            .find(Class("title").descendant(Name("a")))
            .next()
            .unwrap();
        // however, we don't grab text
        // instead find the "href" attribute, which gives us the url
        println!("{:?}\n", url.attr("href").unwrap());
    }
}
