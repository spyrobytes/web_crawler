# # Web Crawler from the book "Black Hat Rust" by Sylvain Kerkour

[Trail](https://kerkour.com/rust-crawler-associated-types)
[Repo](https://github.com/skerkour/black-hat-rust/tree/main/ch_05)

## Notes on Design, Implementation, and Rust Language Features

We conceptualized the web crawler as consisting of distinct `spiders` that
are specialized for a specific task. In ASCII art, the design looks like this:

Web Crawler
    |
    |--- Spider 1
    |--- Spider 2
    |--- Spider 3
    |--- ...

    Spider
        |
        |--- Scraper
        |--- Processor

The `web crawler` is the main application. It is responsible for starting the
`spiders` and managing them. The `spiders` are responsible for scraping the
data and processing it. The heart of the crawler is the *Control Loop`. Its job
is to dispatch the data between the `spiders` and the `processors`, and queue
the URLs to be scraped.

### Spiders

The `spider` is a specialized part of the crawler that can be tuned for a specific
site/task. Functionally, it consists of two parts:

- The `scraper`: fetches the URLs, parses the data, turns it into structured data
- The `processor`: processes the structured data, e.g. saving it to database.

The `Spider` trait is defined as follows:

```rust
#[async_trait]
pub trait Spider: Send + Sync {
    type Item; // This determine the specialized type of the spider
               // and basically translates to the type of data the spider
                // will scrape and process: structs CveDetails, GitHubItem, 
                // QuotesItem

    fn name(&self) -> String;
    fn start_urls(&self) -> Vec<String>;
    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Error>;
    async fn process(&self, item: Self::Item) -> Result<(), Error>;
}
```

Any type that implements the `Spider` trait can be used as a spider. The 
application comes with three types that implemented the `Spider` trait:

1. `CveDetailsSpider`: scrapes the CVE details website.

This is a Common Vulnerabilities and Exposures (CVE) database of publicly 
disclosed information security issues. It implements the `Spider` trait as follows:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CveDetails {
    id: String,
    url: String,
    description: String,
    score: String,
    vulnerability_type: String,
    published_date: String,
    updated_date: String,
}

// struct CveDetailsSpider
struct CveDetailsSpider; // unit struct


#[async_trait]
impl Spider for CveDetailsSpider {
    type Item = CveDetails; // associated type

    fn name(&self) -> String {
        "cve_details".to_string()
    }

    fn start_urls(&self) -> Vec<String> {
        vec![
            "https://www.cvedetails.com/vulnerability-list.php?vendor_id=0&product_id=0&version_id=0&page=1&hasexp=0&opdos=0&opec=0&opov=0&opcsrf=0&opgpriv=0&opsqli=0&opxss=0&opdirt=0&opmemc=0&ophttprs=0&opbyp=0&opfileinc=0&opginf=0&cvssscoremin=0&cvssscoremax=0&year=0&month=0&cweid=0&order=1&trc=100&sha=0".to_string(),
        ]
    }

    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Error> {
        let mut items = vec![];
        let mut next_urls = vec![];

        let html = reqwest::get(&url).await?.text().await?;
        let document = Document::from(html.as_str());

        for node in document.find(Class("srrowns")).iter() {
            let cve_id = node.find(Class("num")).next().unwrap().text();
            let cve_url = format!("https://www.cvedetails.com/cve/{}", cve_id);
            let cve_description = node.find(Class("descript")).next().unwrap().text();
            let cve_score = node.find(Class("cvssbox")).next().unwrap().text();
            let cve_vulnerability_type = node.find(Class("cvelistinfo")).next().unwrap().text();
            let cve_published_date = node.find(Class("cvedata")).next().unwrap().text();
            let cve_updated_date = node.find(Class("cvssbox")).next().unwrap().text();

            let item = CveDetails {
                id: cve_id.to_string(),
                url: cve_url,
                description: cve_description.to_string(),
                score: cve_score.to_string(),
                vulnerability_type: cve_vulnerability_type.to_string(),
                published_date
                updated_date: cve_updated_date.to_string(),
            };

            items.push(item);
        }

        let next_page = document.find(Class("paging")).next().unwrap().find(Name("a")).next().unwrap().attr("href").unwrap();
        let next_url = format!("https://www.cvedetails.com{}", next_page);
        next_urls.push(next_url);

        Ok((items, next_urls))
    }

    async fn process(&self, item: Self::Item) -> Result<(), Error> {
        let mut client = Client::connect("mongodb://localhost:27017", None).await?;
        let db = client.database("blackhatrust");
        let collection = db.collection("cve_details");

        let document = bson::to_bson(&item)?;
        let document = document.as_document().unwrap().to_owned();

        collection.insert_one(document, None).await?;

        Ok(())
    }

}
```

2. `GitHubSpider`: scrapes the GitHub website.

This scrape all users of a GitHub organization. We're basically crawling a 
`JSON API`. As our crawler won't make tons of requests, we don't need to use 
a token to authenticate to Github's API, but we need to set up some headers. 
Otherwise, the server would block our requests. 

The user data we need is organized in a struct, `GitHubItem`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubItem {
    login: String,
    id: u64,
    node_id: String,
    html_url: String,
    avatar_url: String,
}
```

The `GitHubSpider` implements the `Spider` trait as follows:

```rust
#[async_trait]
impl Spider for GitHubSpider {
    type Item = GitHubItem;

    fn name(&self) -> String {
        "github".to_string()
    }

    fn start_urls(&self) -> Vec<String> {
        vec![
            "https://api.github.com/orgs/rust-lang/members".to_string(),
        ]
    }

    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Error> {
        let mut items = vec![];
        let mut next_urls = vec![];

        let client = reqwest::Client::new();
        let response = client.get(&url)
            .header("User-Agent", "Black Hat Rust")
            .send()
            .await?;

        let json = response.text().await?;
        let users: Vec<GitHubItem> = serde_json::from_str(&json)?;

        for user in users {
            let item = GitHubItem {
                login: user.login,
                id: user.id,
                node_id: user.node_id,
                html_url: user.html_url,
                avatar_url: user.avatar_url,
            };

            items.push(item);
        }

        Ok((items, next_urls))
    }

    async fn process(&self, item: Self::Item) -> Result<(), Error> {
        let mut client = Client::connect("mongodb://localhost:27017", None).await?;
        let db = client.database("blackhatrust");
        let collection = db.collection("github");

        let document = bson::to_bson(&item)?;
        let document = document.as_document().unwrap().to_owned();

        collection.insert_one(document, None).await?;

        Ok(())
    }
}
```

3. `QuotesSpider`: scrapes the Quotes to Scrape website.
This will scrape the quotes website - "https://quotes.toscrape.com/js".
It is a JavaScript SPA, so we need to use a headless browser to scrape it. 
We'll use the `headless_chrome` crate to scrape the website.

The spider will scrape the quotes and the author of the quotes:

```rust
#[derive(Debug, Clone)]
pub struct QuotesItem {
    quote: String,
    author: String,
}
```

The `QuotesSpider` implements the `Spider` trait as follows:

```rust
#[async_trait]
impl Spider for QuotesSpider {
    type Item = QuotesItem;

    fn name(&self) -> String {
        "quotes".to_string()
    }

    fn start_urls(&self) -> Vec<String> {
        vec![
            "https://quotes.toscrape.com/js".to_string(),
        ]
    }

    async fn scrape(&self, url: String) -> Result<(Vec<Self::Item>, Vec<String>), Error> {
        let mut items = vec![];
        let mut next_urls = vec![];

        let browser = HeadlessChrome::new(HeadlessConfig {
            executable_path: Some("/usr/bin/chromium".to_string()),
            ..Default::default()
        })
        .await?;

        let tab = browser.new_tab().await?;
        tab.navigate_to(&url).await?;

        let html = tab.wait_for_element("body").await?.inner_html().await?;
        let document = Document::from(html.as_str());

        for node in document.find(Class("quote")).iter() {
            let quote = node.find(Class("text")).next().unwrap().text();
            let author = node.find(Class("author")).next().unwrap().text();

            let item = QuotesItem {
                quote: quote.to_string(),
                author: author.to_string(),
            };

            items.push(item);
        }

        let next_page = document.find(Class("next")).next().unwrap().find(Name("a")).next().unwrap().attr("href").unwrap();
        let next_url = format!("https://quotes.toscrape.com{}", next_page);
        next_urls.push(next_url);

        Ok((items, next_urls))
    }

    async fn process(&self, item: Self::Item) -> Result<(), Error> {
        let mut client = Client::connect("mongodb://localhost:27017", None).await?;
        let db = client.database("blackhatrust");
        let collection = db.collection("quotes");

        let document = bson::to_bson(&item)?;
        let document = document.as_document().unwrap().to_owned();

        collection.insert_one(document, None).await?;

        Ok(())
    }
}


