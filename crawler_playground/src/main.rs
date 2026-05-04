/**
 * web crawler playground - sequential version
 * 
 * Pieces of code that may make it into the final crawler
 * 
 * - HTTP request with `reqwest` to grab an HTML page
 * - HTML parsing with `select` to extract links
 * 
 * @SEE https://rolisz.ro/2020/03/01/web-crawler-in-rust/
 * 
 * Run with:
 * - Debug
 * `cargo run --package crawler_playground --bin crawler_playground`
 * - Release
 * `cargo run --release --package crawler_playground --bin crawler_playground`
 * 
 */
use std::io::Read;
use std::collections::HashSet;
use std::path::Path;
use std::fs;
use std::time::Instant;
use select::document::Document;
use select::predicate::{Name, Predicate};
use reqwest::Url;

// Let's modularize the code into functions
// 1. get all links from an HTML page with no duplicates
fn get_links_from_html(html: &str) -> HashSet<String> {
    Document::from(html)
        .find(Name("a").or(Name("link")))
        .filter_map(|n| n.attr("href"))
        .filter(has_extension) // filter out links with extensions ie. images, videos, etc
        .filter_map(normalize_url)
        .collect::<HashSet<String>>()
}

// 2. normalize URLs so that they can properly be
// parsed by `reqwest::Url`
fn normalize_url(url: &str) -> Option<String> {
    let new_url = Url::parse(url);
    match new_url {
        Ok(new_url) => {
            // only keep URLs from the same domain; don't follow external links
            if new_url.has_host() && new_url.host_str().unwrap() == "ghost.rolisz.ro" {
                Some(url.to_string())
            } else {
                None
            }
        },
        Err(_e) => {
            // Relative urls are not parsed by Reqwest, so we need to
            // fully qualify them
            if url.starts_with('/') {
                Some(format!("https://rolisz.ro{}", url))
            } else { // ignore other URLs
                None
            }
        }
    }
}

// Fetch a URL and return the body as a string
fn fetch_url(client: &reqwest::blocking::Client, url: &str) -> String {
    let mut res = client.get(url).send().unwrap();
    println!("Status for {}: {}", url, res.status());

    let mut body  = String::new();
    res.read_to_string(&mut body).unwrap();
    body
}

// Check if a URL has an extension
// We don't want to follow links to images, videos, etc
// So we only follow links that don't have an extension e.g. `rolisz.ro/blog`
// but not `rolisz.ro/image.png`
// This is not a perfect solution, but it's good enough for now
// because some websites may have URLs with html extension
fn has_extension(url: &&str) -> bool {
    Path::new(url).extension().is_none()
}

// write to disk
// first create the folder "static" if it doesn't exist in the root directory
// then write the `content` to disk
// `create_dir_all` works like `mkdir -p` in `bash`. This way we can create
// nested directories
fn write_file(path: &str, content: &str) {
    fs::create_dir_all(format!("static{}", path)).unwrap();
    // fs::write returns a Result, but we don't care about it
    let _ = fs::write(format!("static{}/index.html", path), content);
}


fn main() {
    const URL: &str = "https://rolisz.ro/";
    // const URL2: &str = "https://www.rust-lang.org/";

    let now = Instant::now();

    let client = reqwest::blocking::Client::new();
    let origin_url = URL;

    let body = fetch_url(&client, origin_url);

    let mut visited = HashSet::new();
    visited.insert(origin_url.to_string()); // mark starting URL as visited
    let found_urls = get_links_from_html(&body);
    let mut new_urls = found_urls
        .difference(&visited)
        .map(|x| x.to_string())
        .collect::<HashSet<String>>();

    // Iterating over all links
    // we do a breadth first search
    // follow each link and get all the links from that page
    while !new_urls.is_empty() {
        let found_urls: HashSet<String> = new_urls
        	.iter()
            .map(|url| {
                let body = fetch_url(&client, url);
                write_file(&url[origin_url.len() - 1..], &body);
                let links = get_links_from_html(&body);
                println!("Visited: {} found {} links", url, links.len());
                links
        })
        .fold(HashSet::new(), |mut acc, x| {
                acc.extend(x);
                acc
        });

        visited.extend(new_urls);
        // remove the ones we already visited: set difference
        new_urls = found_urls
            .difference(&visited)
            .map(|x| x.to_string())
            .collect::<HashSet<String>>();
        println!("New urls: {}", new_urls.len())
    }
    println!("URLs: {:#?}", found_urls);
    println!("{}", now.elapsed().as_secs());

}
