# Building a Web Crawler / Spider with Rust

References:

- [Trail](https://kerkour.com/rust-crawler-associated-types)
- [Code Repo](https://github.com/skerkour/black-hat-rust/tree/main/ch_05/crawler)

## Designing a Web Crawler

The terms *crawler*, *spider*, and *scraper* are often used interchangeably.
However, there are some differences between them. A *crawler* is a program that
traverses the web by following links from one page to another. A *scraper* is a
program that extracts data from a web page. A *spider* is a program that does
both crawling and scraping.

We will keep to these broad definitions in designing our web crawler. The figure 
crawler_architecture.png shows the architecture of a web crawler. Our goal is 
to turn unstructured web data into structured data.

The web crawler has three main components: a *downloader*, a *spider*, and a
control loop.

1. The Downloader (`Fetcher`): get_pages(), using `reqwest` crate 

- Given a list of *seed* URLs, get/download the page(s) to start the crawl. 
  This could be the root/index page of the target websites.
- May be a breadth-first search, if getting all links on the target page
  desired.
  
2. The Spider: scraper + processor

The `spider` is a specialized part of the crawler that can be tuned for a specific
site/task. Functionally, it consists of two parts:
- The scraper: fetches the URLs, parses the data, turns it into structured data
- The processor: processes the structured data, e.g. saving it to database.
- Specifications - We can have three different spiders:
	1. a spider for an HTML-only website (static)
	2. a spider for a JSON API
	3. and a spider for a website using JavaScript to render elements so we 
	are going to need to use a headless browser

3. A control loop

This is the generic part of a crawler. Its job is to *dispatch* data between the 
`scrapers` and the `processors` and queue URLs.


## Spider Specialization

1. A spider for dynamic websites (using JavaScript to render elements)

We'll need a *headless browser* in order to scrape dynamically generated web 
sites. For example, websites that generate elements of pages on client-site
using JavaScript.

Headless browser is a browser that can be operated remotely and programmatically.
The `headless_chrome` crate would connect to the `chromedriver` from Rust code. It
is even possible to automate actions on page - form filling, clicks, etc.
The `headless_chrome` library has similar functionality to `Puppeteer` for NodeJS 
and `Selenium` for Python.

Install the open source Google `chromium` browser and the `chromedriver`:

`dnf info chromium`
`dnf info chromedriver`

You'll also need the `headless_chrome` crate for programmatic control. An alternative
crate is `thirtyfour` which is a Selenium client for Rust.

[headless_chrome](https://docs.rs/headless_chrome/latest/headless_chrome/)
[thirtyfour](https://docs.rs/thirtyfour/latest/thirtyfour/)
