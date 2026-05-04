/**
 * Hacker News Scrapper
 * 
 * Get news headlines, ranks, and urls from Hacker News
 * 
 * @SEE https://github.com/kxzk/scraping-with-rust
 * 
 * ChangeLog:
 * 
 * - Added the `PrettyTable` crate to display the results in a table
 * 
 * Run with:
 * cargo check --package crawler_playground --bin hackernews_scraper
 * 
 * clippy with:
 * cargo clippy --package crawler_playground --bin hackernews_scraper
 */
#[macro_use] extern crate prettytable; // for the table! macro
use select::document::Document;
use select::predicate::{Class, Name, Predicate};
use prettytable::Table;

fn main() {
    const HACKER_NEWS_URL: &str = "https://news.ycombinator.com";
    hacker_news(HACKER_NEWS_URL);
}

fn hacker_news(url: &str) {

    let resp = reqwest::blocking::get(url).unwrap();
    assert!(resp.status().is_success());

    let document = Document::from_read(resp).unwrap();

    let mut table = Table::new();

    // same as before
    for node in document.find(Class("athing")) {
        let rank = node.find(Class("rank")).next().unwrap();
        let story = node.find(Class("title").descendant(Name("a")))
            .next()
            .unwrap()
            .text();
        let url = node.find(Class("title").descendant(Name("a")))
            .next()
            .unwrap();
        let url_txt = url.attr("href").unwrap();
        // shorten strings to make table aesthetically appealing
        // otherwise table will look mangled by long URLs
        // let url_trim = url_txt.trim_left_matches('/');
        let url_trim = url_txt.trim_start_matches('/');
        let rank_story = format!(" | {} | {}", rank.text(), story);
        // [FdBybl->] specifies row formatting
        // F (foreground) d (black text)
        // B (background) y (yellow text) l (left-align)
        table.add_row(row![FdBybl->rank_story]);
        table.add_row(row![Fy->url_trim]);
    }
    // print table to stdout
    table.printstd();
}