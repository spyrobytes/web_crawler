/**
 * Snippets for testing web crawler components
 * 
 * A possible code organization for a simple web crawler:
 * 
 * Crawler -> Fetcher -> Parser
 * 
 * Run with:
 * cargo run --package crawler_playground --bin crawler_snippets
 * 
 * check with:
 * cargo check --package crawler_playground --bin crawler_snippets
 * 
 * clippy with:
 * cargo clippy --package crawler_playground --bin crawler_snippets
 * 
 */
use select::document::Document;
use select::predicate::Name;


fn main() {
    const URL: &str = "https://www.rust-lang.org/";

    // Snippet for testing the fetcher
    let fetcher = Fetcher::new();
    // let url = "https://www.rust-lang.org/";
    let /*mut*/ response = fetcher.fetch(URL).unwrap();
    println!("Response: {:?}", response);
    println!("Response body: {:?}", response.body);

    // Snippet for testing the parser
    let parser = Parser::new();
    // let url = "https://www.rust-lang.org/";
    let mut response = fetcher.fetch(URL).unwrap();
    let parsed = parser.parse(&mut response);
    println!("Parsed: {:?}", parsed);

    // Snippet for testing the crawler
    let crawler = Crawler::new();
    // let url = "https://www.rust-lang.org/";
    let mut response = fetcher.fetch(URL).unwrap();
    let parsed = parser.parse(&mut response);
    let links = crawler.crawl(&parsed);
    println!("Links: {:?}", links);
}

// implement the fetcher
struct Fetcher {}

impl Fetcher {
    fn new() -> Fetcher {
        Fetcher {}
    }

    fn fetch(&self, url: &str) -> Result<Response, Error> {
        //let mut response = reqwest::get(url)?;
        //let body = response.text()?;
        let body = reqwest::blocking::get(url)?.text()?;

        Ok(Response { body })
    }
}

// implement the parser
struct Parser {}

impl Parser {
    fn new() -> Parser {
        Parser {}
    }

    fn parse(&self, response: &mut Response) -> Parsed {
        let mut links = Vec::new();
        let document = Document::from(response.body.as_str());
        for node in document.find(Name("a")) {
            if let Some(link) = node.attr("href") {
                links.push(link.to_string());
            }
        }
        Parsed { links }
    }
}

// implement the crawler
struct Crawler {}

impl Crawler {
    fn new() -> Crawler {
        Crawler {}
    }

    fn crawl(&self, parsed: &Parsed) -> Vec<String> {
        parsed.links.clone()
    }
}

// define the response
#[derive(Debug)]
struct Response {
    body: String,
}

// define the parsed
#[derive(Debug)]
struct Parsed {
    links: Vec<String>,
}

// define the error
#[derive(Debug)]
enum Error {
    FetchError(reqwest::Error),
    ParseError,
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Error::FetchError(error)
    }
}

// custom selectError
#[derive(Debug)]
struct SelectError {}

// implement the from for selectError
// for error conversion: convert selectError to Error
impl From<SelectError> for Error {
    fn from(_error: SelectError) -> Self {
        Error::ParseError
    }
}

