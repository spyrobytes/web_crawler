# Web Crawlers, Scrapers, and Spiders

[SEE](https://kerkour.com/rust-crawler-associated-types) <|Trails|>
[SEE](https://scrape-it.cloud/blog/web-scraping-with-rust)
[SEE](https://rolisz.ro/2020/03/01/web-crawler-in-rust/)
[Cookbook](https://rust-lang-nursery.github.io/rust-cookbook/web/scraping.html)

Test sites for scrapping:

[SscrapeMe](https://scrapeme.live/shop/)
[Books to Scrape](https://books.toscrape.com/)
[Demo OpenCart](https://demo.opencart.com/)

Programs that crawl through web pages and extract information from them.

A use case might be when you want to compare prices listed on different 
e-commerce websites, but these websites _don’t provide APIs_. In this case,
a crawler can, for example, crawl through their web pages and extract the 
price information from the HTML.

## Building The Web Crawler

## 1. Use Frameworks

There are a few frameworks for building web crawlers, for example `maman`, 
`spider`, and `url-crawler`. `url-crawler` is now depreciated, replaced with
`crusty`.


## Use HTTP client Library + HTML Parser

If you want more fine-grained control over the crawling and parsing process, 
you can use the `reqwest` HTTP client library to download the HTML page, and 
use an HTML parsing/querying library to parse the page and extract data. Some 
popular HTML parsing/querying library include `html5ever`, `scraper`, and 
`select`.


## `Spider` Web Crawler

[SEE](https://crates.io/crates/spider)

