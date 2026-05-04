# Web Scrapping - IMDb and Hacker News

In this mini-project, we will be scrapping data from IMDb and Hacker News to
illustrate the core components and techniques for web scrapping.

## 1. Scrapping The Movie Database (IMDb)

[SEE](https://www.scrapingbee.com/blog/web-scraping-rust/)

Goal: get the top 100 movies (by user rating) from IMDb.

This program illustrates the core components and techniques for web scrapping.
It is relatively simple, but all the ingredients are there: 

1. getting the HTML
   We use an HTTP client to get the HTML document of the page we want to scrape.

2. Parsing the HTML document
   Using an HTML parser, we zone in on target elements in the HTML document.
3. extracting the data you need
    We extract the data we need from the HTML document.

The IMDb website is a good source of data for our web crawler. It is a
database of movies, TV shows, and celebrities. It is a good example of a web
scraping project because the data is not available in a convenient format. You
can’t download a CSV file with the top 100 movies, and neither is an API
available. The only way to get the data is to scrape it from the website.


## The Movie Database (IMDb)

The IMDb website is a database of movies, TV shows, and celebrities. It is a 
good source of data for our web crawler.

To scrape this website, we will first need to get the HTML document of the
page we want to scrape (using `reqwest`), then parse it (using `select` or 
`scaper` crate) to extract the data we want.

The `reqwest` crate is a high-level HTTP client for Rust, built on top of
`hyper`. It provides a convenient API for making HTTP requests and handling
responses. It also provides a convenient API for parsing JSON responses. In 
addition, it can do a lot of the things that a regular browser can do, such as 
open pages, log in, and store cookies.


## Getting the HTML document

```rust
use reqwest::blocking::Client;

fn main() {
    const URL: &str = "https://www.imdb.com/search/title/?groups=top_100&sort=user_rating,desc&count=100";
    let response = reqwest::blocking::get(URL,)
        .unwrap()
        .text()
        .unwrap();
}
```

## Parsing the HTML document

Parsing HTML documents is usually the hardest part of web scraping. The
`scraper` crate provides a convenient API for parsing HTML documents based on
CSS selectors.

```rust
use reqwest::blocking::Client;
use scraper::{Html, Selector};

fn main() {
    const URL: &str = "https://www.imdb.com/search/title/?groups=top_100&sort=user_rating,desc&count=100";
    let response = reqwest::blocking::get(URL,)
        .unwrap()
        .text()
        .unwrap();
    
    let document = Html::parse_document(&response);
}
```
Next, find and select the parts you need. To do that, you need to check the 
website’s code and find a collection of CSS selectors that uniquely identifies 
those items. 

**TIP***: Opening the website in a browser and using the developer tools can 
help you find the right CSS selectors.

Another way to find the right CSS selectors is to use the Chrome extension
`Selector Gadget`. This extension allows you to select elements on a page and
then suggests CSS selectors that uniquely identify those elements. Frankly, I
find this extension to be less useful than the developer tools, but it can be
useful for beginners.

In the case of IMDb, the element you need is the name of the movie. When you 
check the element, you’ll see that it’s wrapped in an `<a>` tag:

`<a href="/title/tt0111161/?ref_=adv_li_tt">The Shawshank Redemption</a>`

Unfortunately, this tag is not unique. Since there are a lot of `<a>` tags on 
the page, it wouldn’t be a smart idea to scrape them all, as most of them won’t 
be the items you need. Instead, find the tag unique to movie titles and then 
navigate to the `<a>` tag inside that tag.

In this case, you can pick the `lister-item-header`  CSS class. The HTML code
looks like this:

```html
<h3 class="lister-item-header">
    <span class="lister-item-index unbold text-primary">1.</span>
    <a href="/title/tt0111161/?ref_=adv_li_tt">The Shawshank Redemption</a>
    <span class="lister-item-year text-muted unbold">(1994)</span>
</h3>
```

Here is the code to select the movie titles:

```rust
use reqwest::blocking::Client;
use scraper::{Html, Selector};

fn main() {
    const URL: &str = "https://www.imdb.com/search/title/?groups=top_100&sort=user_rating,desc&count=100";
    let response = reqwest::blocking::get(URL,)
        .unwrap()
        .text()
        .unwrap();
    
    let document = Html::parse_document(&response);
    
    // construct a CSS selector that selects the title of each movie
    let title_selector = scraper::Selector::parse("h3.lister-item-header>a").unwrap();
    
    // query the document using the selector
    let titles = document
        .select(&title_selector)
        .map(|x| x.inner_html())
        .zip(1..101)
        .for_each(|(item, number)| println!("{}. {}", number, item));

}
```

## Problems with Scrapping

Top of the list is the fact that the structure of the website can change at any
time. If the structure changes, your scraper will break. This is why you should
always check the website’s terms of service before scraping it. Some websites
don’t allow scraping, and some allow it only for personal use.

For example, in the past, it is possible to scrape IMDb for titles and ratings
using the selector `h3.lister-item-header>a`, as shown in the above code 
listing (read: it no longer work!). But now, the website has changed and the 
selector no longer works. The new selector is `div.ipc-title>a>h3`

Another problem is that some websites have anti-scraping mechanisms in place.
These mechanisms can detect that you are scraping the website and block your
IP address. To avoid this, you can use a `proxy` server or a headless browser
(like `chromdriver` or `puppeteer`).

## 2. Scrapping Hacker News

[SEE](https://github.com/kxzk/scraping-with-rust)

Goal: we want to scrape the hacker news website and print out the |rank|,
|story headline| and the |url|.

We will be using the `select` crate for parsing because more complex queries
is possible with it. The `select` crate gives us an increased level of control
over specifying exactly what we want to scrape, using its `predicate` module.


```rust
use select::document::Document;
use select::predicate::{Class, Name, Predicate};

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

fn main() {
    hacker_news("https://news.ycombinator.com/");
}
```
