use crate::error::Error;
use async_trait::async_trait;
use fantoccini::{Client, ClientBuilder};
use select::{
    document::Document,
    node::Node,
    predicate::{Class, Name, Predicate},
};
use tokio::sync::Mutex;

pub struct QuotesSpider {
    webdriver_client: Mutex<Client>,
}

impl QuotesSpider {
    pub async fn new() -> Result<Self, Error> {
        let mut caps = serde_json::map::Map::new();
        let chrome_opts = serde_json::json!({ "args": ["--headless", "--disable-gpu"] });
        caps.insert("goog:chromeOptions".to_string(), chrome_opts);
        // fantoccini 0.20+ returns a Result here because building the
        // rustls connector can fail.
        let mut builder =
            ClientBuilder::rustls().map_err(|err| Error::WebDriver(err.to_string()))?;
        let webdriver_client = builder
            .capabilities(caps)
            .connect("http://localhost:4444")
            .await?;

        Ok(QuotesSpider {
            webdriver_client: Mutex::new(webdriver_client),
        })
    }
}

#[derive(Debug, Clone)]
pub struct QuotesItem {
    quote: String,
    author: String,
}

#[async_trait]
impl super::Spider for QuotesSpider {
    type Item = QuotesItem;

    fn name(&self) -> String {
        String::from("quotes")
    }

    fn start_urls(&self) -> Vec<String> {
        vec!["https://quotes.toscrape.com/js".to_string()]
    }

    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Error> {
        let mut items = Vec::new();
        let html = {
            let webdriver = self.webdriver_client.lock().await;
            webdriver.goto(&url).await?;
            webdriver.source().await?
        };

        let document = Document::from(html.as_str());

        for quote in document.find(Class("quote")) {
            // A single odd block should not cost us the rest of the page, so
            // we log it and keep going.
            match parse_quote(&url, quote) {
                Ok(item) => items.push(item),
                Err(err) => log::warn!("{}", err),
            }
        }

        let next_pages_link = document
            .find(
                Class("pager")
                    .descendant(Class("next"))
                    .descendant(Name("a")),
            )
            .filter_map(|n| n.attr("href"))
            .map(|url| self.normalize_url(url))
            .collect::<Vec<String>>();

        Ok((items, next_pages_link))
    }

    async fn process(&self, item: Self::Item) -> Result<(), Error> {
        println!("{}", item.quote);
        println!("by {}\n", item.author);
        Ok(())
    }
}

// Parse one `.quote` block.
//
// These used to be `unwrap()`s. A panic inside a spider does not crash the
// program: tokio catches it at the task boundary, which kills the scraper
// task and leaves the crawler's control loop waiting forever.
fn parse_quote(url: &str, quote: Node<'_>) -> Result<QuotesItem, Error> {
    let mut spans = quote.find(Name("span"));

    let quote_str = spans
        .next()
        .map(|span| span.text().trim().to_string())
        .ok_or_else(|| Error::Internal(format!("{url}: quote block has no text span")))?;

    let author = spans
        .next()
        .and_then(|span| span.find(Class("author")).next())
        .map(|node| node.text().trim().to_string())
        .ok_or_else(|| Error::Internal(format!("{url}: quote block has no author")))?;

    Ok(QuotesItem {
        quote: quote_str,
        author,
    })
}

impl QuotesSpider {
    fn normalize_url(&self, url: &str) -> String {
        let url = url.trim();

        if url.starts_with('/') {
            return format!("https://quotes.toscrape.com{}", url);
        }

        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://quotes.toscrape.com/js";

    fn parse(block: &str) -> Result<QuotesItem, Error> {
        let document = Document::from(block);
        let quote = document
            .find(Class("quote"))
            .next()
            .expect("test html has one quote block");
        parse_quote(URL, quote)
    }

    #[test]
    fn parses_a_well_formed_quote() {
        let item = parse(
            r#"<div class="quote">
                 <span class="text">“Be yourself.”</span>
                 <span>by <small class="author">Oscar Wilde</small></span>
               </div>"#,
        )
        .expect("quote parses");

        assert_eq!(item.quote, "“Be yourself.”");
        assert_eq!(item.author, "Oscar Wilde");
    }

    #[test]
    fn quote_without_author_is_an_error() {
        let err = parse(r#"<div class="quote"><span class="text">“Be yourself.”</span></div>"#)
            .expect_err("missing author is rejected");

        assert!(err.to_string().contains("no author"), "{err}");
    }

    #[test]
    fn empty_quote_block_is_an_error() {
        let err = parse(r#"<div class="quote"></div>"#).expect_err("empty block is rejected");

        assert!(err.to_string().contains("no text span"), "{err}");
    }
}
