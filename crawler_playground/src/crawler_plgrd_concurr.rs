/**
 * Concurrent web crawler
 * This is a simple crawler that will crawl a website and save all the pages
 *
 * @SEE https://rolisz.ro/2020/03/01/web-crawler-in-rust/
 * 
 * ChangeLog:
 *
 * 2023-12-7 : Added concurrency with Rayon
 *
 * Run with:
 * - Debug
 * `cargo run --package crawler_playground --bin crawler_playground_concurrent`
 * - Release
 * `cargo run --release --package crawler_playground --bin crawler_playground_concurrent`
 *
 *
 */
use rayon::prelude::*;
use reqwest::Url;
use select::document::Document;
use select::predicate::{Name, Predicate};
use std::collections::HashSet;
use std::fs;
use std::io::Error as IoErr;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

#[allow(dead_code)]
#[derive(Debug)]
enum Error {
    Write { url: String, e: IoErr },          // IO error
    Fetch { url: String, e: reqwest::Error }, // Reqwest error
}

type Result<T> = std::result::Result<T, Error>;

// Implement the `From` trait for the `Error` enum
// so that we can easily convert from `std::io::Error`
// to our own `Error` type
// The `S: AsRef<str>` is a trait bound that says that
// the first argument needs to be convertible to a `&str`
impl<S: AsRef<str>> From<(S, IoErr)> for Error {
    fn from((url, e): (S, IoErr)) -> Self {
        Error::Write {
            url: url.as_ref().to_string(),
            e,
        }
    }
}

// Implement the `From` trait for the `Error` enum
// so that we can easily convert from `reqwest::Error`
// to our own `Error` type
// Note the alternative syntax for the trait bound
impl<S> From<(S, reqwest::Error)> for Error
where
    S: AsRef<str>,
{
    fn from((url, e): (S, reqwest::Error)) -> Self {
        Error::Fetch {
            url: url.as_ref().to_string(),
            e,
        }
    }
}

fn get_links_from_html(html: &str) -> HashSet<String> {
    Document::from(html)
        .find(Name("a").or(Name("link")))
        .filter_map(|n| n.attr("href"))
        .filter(has_extension)
        .filter_map(normalize_url)
        .collect::<HashSet<String>>()
}

fn normalize_url(url: &str) -> Option<String> {
    // Parse the URL
    // To understand the remainder of the code, you need to know that
    // `Url::parse` returns a `Result<Url, ParseError>`:
    // Ok(Url) if the URL is valid
    // Err(ParseError) if the URL is invalid
    let new_url = Url::parse(url); // returns a Result<Url, ParseError>
    
    match new_url {
        Ok(new_url) => { // if the URL is valid
            if let Some("rolisz.ro") = new_url.host_str() {
                Some(url.to_string())
            } else {
                None
            }
        }
        Err(_e) => { // if the URL is invalid
            // Relative urls are not parsed by Reqwest
            if url.starts_with('/') {
                Some(format!("https://rolisz.ro{}", url))
            } else {
                None
            }
        }
    }
}

fn fetch_url(client: &reqwest::blocking::Client, url: &str) -> Result<String> {
    let mut res = client.get(url).send().map_err(|e| (url, e))?;
    println!("Status for {}: {}", url, res.status());

    let mut body = String::new();
    res.read_to_string(&mut body).map_err(|e| (url, e))?; // propagate the error
    Ok(body)
}

fn has_extension(url: &&str) -> bool {
    Path::new(&url).extension().is_none()
}

// Write a file to disk in the `static` folder of the root directory
fn write_file(path: &str, content: &str) -> Result<()> {
    let dir = format!("static{}", path);
    fs::create_dir_all(format!("static{}", path)).map_err(|e| (&dir, e))?;
    let index = format!("static{}/index.html", path);
    fs::write(&index, content).map_err(|e| (&index, e))?; // propagate the error

    Ok(())
}

fn main() -> Result<()> {
    let now = Instant::now();

    let client = reqwest::blocking::Client::new();
    let origin_url = "https://rolisz.ro/";

    let body = fetch_url(&client, origin_url)?;

    write_file("", &body)?;
    let mut visited = HashSet::new();
    visited.insert(origin_url.to_string());
    let found_urls = get_links_from_html(&body);
    let mut new_urls = found_urls
        .difference(&visited)
        .map(|x| x.to_string())
        .collect::<HashSet<String>>();

    while !new_urls.is_empty() {
        let (found_urls, errors): (Vec<Result<HashSet<String>>>, Vec<_>) = new_urls
            .par_iter()
            .map(|url| -> Result<HashSet<String>> {
                let body = fetch_url(&client, url)?;
                write_file(&url[origin_url.len() - 1..], &body)?;

                let links = get_links_from_html(&body);
                println!("Visited: {} found {} links", url, links.len());
                Ok(links)
            })
            .partition(Result::is_ok);

        visited.extend(new_urls);
        new_urls = found_urls
            .into_par_iter()
            .map(Result::unwrap)
            .reduce(HashSet::new, |mut acc, x| {
                acc.extend(x);
                acc
            })
            .difference(&visited)
            .map(|x| x.to_string())
            .collect::<HashSet<String>>();
        println!("New urls: {}", new_urls.len());
        println!(
            "Errors: {:#?}",
            errors
                .into_iter()
                .map(Result::unwrap_err)
                .collect::<Vec<Error>>()
        )
    }
    println!("Elapsed time: {}", now.elapsed().as_secs());
    Ok(())
}
