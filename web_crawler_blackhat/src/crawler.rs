use crate::error::Error;
use crate::spiders::Spider;
use futures::stream::StreamExt;
use std::{
    collections::{HashSet, VecDeque},
    fmt,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    sync::mpsc,
    task::{JoinError, JoinHandle},
    time::sleep,
};
use tokio_util::sync::CancellationToken;

/// What a finished crawl did, and what went wrong along the way.
///
/// `run` returns this on success. An `Err` from `run` is reserved for the
/// crawler's own machinery failing (a task died); a page that could not be
/// scraped or an item that could not be processed is counted here instead,
/// so that nothing fails silently and the caller decides how strict to be.
/// Why a crawl stopped before its frontier was empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The cancellation token was triggered (for example by Ctrl-C).
    Cancelled,
    /// The configured `max_pages` was reached with URLs still waiting.
    PageLimit,
}

impl fmt::Display for StopReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StopReason::Cancelled => write!(f, "cancelled"),
            StopReason::PageLimit => write!(f, "page limit reached"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrawlStats {
    /// Set when the crawl stopped early; `None` means the frontier ran dry.
    pub stop_reason: Option<StopReason>,
    /// URLs that were discovered but never handed out (non-zero only when
    /// the crawl stopped early).
    pub frontier_remaining: usize,
    /// Pages whose `scrape` returned `Ok`.
    pub pages_scraped: usize,
    /// Pages whose `scrape` returned `Err` (logged at the time).
    pub scrape_errors: usize,
    /// Items whose `process` returned `Ok`.
    pub items_processed: usize,
    /// Items whose `process` returned `Err` (logged at the time).
    pub process_errors: usize,
}

impl CrawlStats {
    pub fn has_failures(&self) -> bool {
        self.scrape_errors > 0 || self.process_errors > 0
    }
}

impl fmt::Display for CrawlStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pages scraped: {}, scrape errors: {}, items processed: {}, processing errors: {}",
            self.pages_scraped, self.scrape_errors, self.items_processed, self.process_errors
        )?;
        if let Some(reason) = self.stop_reason {
            write!(
                f,
                " (stopped early: {reason}, {} urls left in the frontier)",
                self.frontier_remaining
            )?;
        }
        Ok(())
    }
}

// Tallies kept inside each task and handed back through its `JoinHandle`.
// They are atomics only because `for_each_concurrent` runs several futures
// against the same counters; nothing outside the task reads them until the
// task has finished.
#[derive(Default)]
struct ScraperTally {
    pages_scraped: AtomicUsize,
    scrape_errors: AtomicUsize,
}

#[derive(Default)]
struct ProcessorTally {
    items_processed: AtomicUsize,
    process_errors: AtomicUsize,
}

pub struct Crawler {
    delay: Duration,
    crawling_concurrency: usize,
    processing_concurrency: usize,
    max_pages: Option<usize>,
    cancellation: CancellationToken,
}

impl Crawler {
    pub fn new(
        delay: Duration,
        crawling_concurrency: usize,
        processing_concurrency: usize,
    ) -> Self {
        Crawler {
            delay,
            crawling_concurrency,
            processing_concurrency,
            max_pages: None,
            // A token nobody else holds can never be cancelled, so the
            // default is "run until the frontier is empty".
            cancellation: CancellationToken::new(),
        }
    }

    /// Stop handing out URLs once this many have been dispatched. In-flight
    /// pages still finish and their items are still processed.
    pub fn with_max_pages(mut self, max_pages: Option<usize>) -> Self {
        self.max_pages = max_pages;
        self
    }

    /// Cancelling this token asks the crawl to stop: nothing new is handed
    /// out, in-flight pages finish, and `run` returns with
    /// `StopReason::Cancelled`.
    pub fn with_cancellation(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = cancellation;
        self
    }

    pub async fn run<T: Send + 'static>(
        &self,
        spider: Arc<dyn Spider<Item = T>>,
    ) -> Result<CrawlStats, Error> {
        let crawling_concurrency = self.crawling_concurrency;
        let crawling_queue_capacity = crawling_concurrency * 400;
        let processing_concurrency = self.processing_concurrency;
        let processing_queue_capacity = processing_concurrency * 10;

        // Three bounded channels connect the tasks:
        //   control loop --urls_to_visit--> scrapers --items--> processors
        //   control loop <--new_urls------- scrapers
        // The first and last form a cycle, which is why the control loop
        // below must never block on a send (see the loop comment).
        let (urls_to_visit_tx, urls_to_visit_rx) = mpsc::channel(crawling_queue_capacity);
        let (items_tx, items_rx) = mpsc::channel(processing_queue_capacity);
        let (new_urls_tx, mut new_urls_rx) = mpsc::channel(crawling_queue_capacity);

        // The control loop owns all crawl state: what has been seen, what is
        // waiting to be fetched, and how many URLs are currently in the
        // scrapers' hands. None of it is shared, so none of it needs a lock.
        let mut visited_urls = HashSet::<String>::new();
        let mut frontier = VecDeque::<String>::new();
        let mut outstanding: usize = 0;
        let mut dispatched: usize = 0;
        let mut stop_reason: Option<StopReason> = None;

        for url in spider.start_urls() {
            if visited_urls.insert(url.clone()) {
                frontier.push_back(url);
            }
        }

        let processors = self.launch_processors(processing_concurrency, spider.clone(), items_rx);

        // `new_urls_tx` is moved, not cloned: the scraper task must own the
        // only sender, so that `new_urls_rx.recv()` returning `None` means
        // "the scraper task is gone" and nothing else.
        let scrapers = self.launch_scrapers(
            crawling_concurrency,
            spider.clone(),
            urls_to_visit_rx,
            new_urls_tx,
            items_tx,
            self.delay,
        );

        // The Control Loop
        //
        // Each turn waits for whichever happens first: a scraper reports back,
        // or there is room to hand the scrapers another URL. Waiting for room
        // with `reserve()` instead of calling `send().await` is the whole
        // point. If the loop blocked on a send into a full `urls_to_visit`,
        // it would stop draining `new_urls`; once that filled too, the
        // scrapers would block on their reports and nothing would move. Two
        // bounded channels in a cycle deadlock unless one side never blocks.
        //
        // `outstanding` counts URLs from the moment they are handed over until
        // their report arrives, so "frontier empty and nothing outstanding"
        // is exactly "no work anywhere", with no gap for a URL to hide in
        // between leaving the channel and being picked up by a scraper.
        //
        // Stopping early, whether by page budget or by cancellation, is just
        // "behave as if the frontier were empty": stop handing URLs out, keep
        // taking reports until nothing is outstanding, then shut down the
        // normal way. In-flight pages finish and their items are processed.
        loop {
            let page_limit_hit = self.max_pages.is_some_and(|max| dispatched >= max);
            if stop_reason.is_none() && page_limit_hit && !frontier.is_empty() {
                log::info!(
                    "crawler: page limit of {dispatched} reached, {} urls left in the frontier",
                    frontier.len()
                );
                stop_reason = Some(StopReason::PageLimit);
            }

            if (frontier.is_empty() || stop_reason.is_some()) && outstanding == 0 {
                break;
            }

            tokio::select! {
                report = new_urls_rx.recv() => {
                    let Some((visited_url, new_urls)) = report else {
                        // The only sender lived in the scraper task, so this
                        // means that task has ended early (it panicked). The
                        // join handle below will say why.
                        log::error!(
                            "crawler: scrapers stopped with {outstanding} urls outstanding"
                        );
                        break;
                    };

                    outstanding -= 1;
                    log::debug!("visited: {visited_url}");

                    for url in new_urls {
                        if visited_urls.insert(url.clone()) {
                            log::debug!("queueing: {url}");
                            frontier.push_back(url);
                        }
                    }
                }

                // Only ask for room when there is something to send, otherwise
                // this branch would hold a permit for nothing.
                // The `if stop_reason.is_none()` guard is what makes "stopping"
                // mean "stop dispatching".
                permit = urls_to_visit_tx.reserve(), if !frontier.is_empty() && stop_reason.is_none() => {
                    let Ok(permit) = permit else {
                        // The receiver is gone: same situation as above.
                        log::error!(
                            "crawler: scrapers stopped with {outstanding} urls outstanding"
                        );
                        break;
                    };

                    let url = frontier.pop_front().expect("branch is guarded by !is_empty");
                    outstanding += 1;
                    dispatched += 1;
                    permit.send(url);
                }

                // Once a token is cancelled, `cancelled()` is ready forever.
                // Without the guard this branch would win every turn and the
                // loop would spin at full CPU while waiting for the last
                // reports. A permanently-ready future inside a `select!` loop
                // always needs a guard like this.
                _ = self.cancellation.cancelled(), if stop_reason.is_none() => {
                    log::warn!(
                        "crawler: stop requested, waiting for {outstanding} in-flight pages"
                    );
                    stop_reason = Some(StopReason::Cancelled);
                }
            }
        }

        log::info!("crawler: control loop exited");

        // Dropping the transmitter closes the stream of URLs, which ends the
        // scraper task; the scraper task drops `items_tx` on its way out,
        // which ends the processor task. That chain is the shutdown order.
        drop(urls_to_visit_tx);

        // Wait for the tasks by awaiting their join handles rather than a
        // barrier. A barrier is a rendezvous: it only works if every party
        // arrives, and a task that panicked never does. A `JoinHandle`
        // resolves either way, and tells us when the task died.
        //
        // Both handles are awaited before returning, even if the first one
        // failed, so processing always drains before `run` returns.
        let scrapers = scrapers.await.map_err(|err| task_failure("scraper", err));
        let processors = processors
            .await
            .map_err(|err| task_failure("processor", err));
        let (scraper_tally, processor_tally) = (scrapers?, processors?);

        let stats = CrawlStats {
            stop_reason,
            frontier_remaining: frontier.len(),
            pages_scraped: scraper_tally.pages_scraped.into_inner(),
            scrape_errors: scraper_tally.scrape_errors.into_inner(),
            items_processed: processor_tally.items_processed.into_inner(),
            process_errors: processor_tally.process_errors.into_inner(),
        };
        log::info!("crawler: {stats}");

        Ok(stats)
    }

    // Launching the processors is a matter of spawning a new task with a
    // stream and for_each_concurrent. The task ends when the stream ends, and
    // the caller gets its tally through the returned join handle.
    fn launch_processors<T: Send + 'static>(
        &self,
        concurrency: usize,
        spider: Arc<dyn Spider<Item = T>>,
        items: mpsc::Receiver<T>,
    ) -> JoinHandle<ProcessorTally> {
        tokio::spawn(async move {
            let tally = ProcessorTally::default();
            let spider_name = spider.name();

            tokio_stream::wrappers::ReceiverStream::new(items)
                .for_each_concurrent(concurrency, |item| async {
                    // `process` is where the real work happens (today a
                    // print, soon a database write), so a failure here is a
                    // scraped item lost. It must be logged and counted, not
                    // discarded with `let _ =`. The crawler cannot show the
                    // item itself (it knows nothing about `T`), so the
                    // spider's error has to carry whatever identifies it.
                    match spider.process(item).await {
                        Ok(()) => {
                            tally.items_processed.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(err) => {
                            log::error!("{spider_name}: processing an item failed: {err}");
                            tally.process_errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                })
                .await;

            tally
        })
    }

    // Launch the scrapers, much the same way as launching the processors.
    // Every URL taken from the stream produces exactly one report on
    // `new_urls_tx`, whether the scrape succeeded or not; the control loop
    // relies on that to know when the URL is no longer in flight. The sleep
    // between requests is there not to flood the server (to avoid being
    // banned).
    fn launch_scrapers<T: Send + 'static>(
        &self,
        concurrency: usize,
        spider: Arc<dyn Spider<Item = T>>,
        urls_to_visit: mpsc::Receiver<String>,
        new_urls_tx: mpsc::Sender<(String, Vec<String>)>,
        items_tx: mpsc::Sender<T>,
        delay: Duration,
    ) -> JoinHandle<ScraperTally> {
        tokio::spawn(async move {
            let tally = ScraperTally::default();
            let spider_name = spider.name();

            tokio_stream::wrappers::ReceiverStream::new(urls_to_visit)
                .for_each_concurrent(concurrency, |queued_url| async {
                    let mut urls = Vec::new();

                    match spider.scrape(queued_url.clone()).await {
                        Ok((items, new_urls)) => {
                            tally.pages_scraped.fetch_add(1, Ordering::Relaxed);
                            for item in items {
                                // A failed send means the processor task is
                                // gone, which the join handle reports; there
                                // is nothing useful to do with the item here.
                                let _ = items_tx.send(item).await;
                            }
                            urls = new_urls;
                        }
                        Err(err) => {
                            log::error!("{spider_name}: scraping {queued_url} failed: {err}");
                            tally.scrape_errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    // Same reasoning: the only way this fails is if the
                    // control loop has already stopped listening.
                    let _ = new_urls_tx.send((queued_url, urls)).await;
                    sleep(delay).await;
                })
                .await;

            drop(items_tx);
            tally
        })
    }
}

// Turn a task that panicked (or was cancelled) into a crawler error, logging
// it at the point where we learn about it.
fn task_failure(task: &str, err: JoinError) -> Error {
    log::error!("crawler: {task} task failed: {err}");
    Error::Internal(format!("crawler: {task} task failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // A spider that behaves like one with an `unwrap()` on bad markup.
    struct PanickingSpider;

    #[async_trait]
    impl Spider for PanickingSpider {
        type Item = ();

        fn name(&self) -> String {
            String::from("panicking")
        }

        fn start_urls(&self) -> Vec<String> {
            vec![String::from("first"), String::from("second")]
        }

        async fn scrape(&self, url: String) -> Result<(Vec<()>, Vec<String>), Error> {
            if url == "second" {
                panic!("simulated unwrap on bad markup");
            }
            Ok((Vec::new(), Vec::new()))
        }

        async fn process(&self, _item: ()) -> Result<(), Error> {
            Ok(())
        }
    }

    // The scraper task owns the only `new_urls` sender, so its death closes
    // the channel and the control loop exits; awaiting the join handle
    // (instead of a barrier the dead task never reaches) lets `run` return
    // and report it.
    #[tokio::test]
    async fn run_returns_an_error_after_a_spider_panics() {
        let crawler = Crawler::new(Duration::from_millis(0), 2, 1);
        let spider: Arc<dyn Spider<Item = ()>> = Arc::new(PanickingSpider);

        let result = tokio::time::timeout(Duration::from_secs(2), crawler.run(spider))
            .await
            .expect("crawler should return after a spider panics");

        let err = result.expect_err("a dead scraper task should be reported");
        assert!(err.to_string().contains("scraper task failed"), "{err}");
    }

    // A well-behaved spider: two pages, five items, and counters we can read
    // back after the crawl.
    struct CountingSpider {
        scraped: AtomicUsize,
        processed: AtomicUsize,
    }

    #[async_trait]
    impl Spider for CountingSpider {
        type Item = u32;

        fn name(&self) -> String {
            String::from("counting")
        }

        fn start_urls(&self) -> Vec<String> {
            vec![String::from("page-1")]
        }

        async fn scrape(&self, url: String) -> Result<(Vec<u32>, Vec<String>), Error> {
            self.scraped.fetch_add(1, Ordering::SeqCst);
            match url.as_str() {
                "page-1" => Ok((vec![1, 2, 3], vec![String::from("page-2")])),
                // links back to page-1 to check the visited-set dedup
                "page-2" => Ok((vec![4, 5], vec![String::from("page-1")])),
                other => Err(Error::Internal(format!("unexpected url {other}"))),
            }
        }

        async fn process(&self, _item: u32) -> Result<(), Error> {
            self.processed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    // Every item is processed before `run` returns.
    #[tokio::test]
    async fn run_processes_every_item_before_returning() {
        let spider = Arc::new(CountingSpider {
            scraped: AtomicUsize::new(0),
            processed: AtomicUsize::new(0),
        });
        let dyn_spider: Arc<dyn Spider<Item = u32>> = spider.clone();
        let crawler = Crawler::new(Duration::from_millis(0), 2, 2);

        let stats = tokio::time::timeout(Duration::from_secs(2), crawler.run(dyn_spider))
            .await
            .expect("crawler should finish")
            .expect("a well-behaved spider should not produce an error");

        assert_eq!(spider.scraped.load(Ordering::SeqCst), 2);
        assert_eq!(spider.processed.load(Ordering::SeqCst), 5);
        assert_eq!(
            stats,
            CrawlStats {
                pages_scraped: 2,
                scrape_errors: 0,
                items_processed: 5,
                process_errors: 0,
                ..Default::default()
            }
        );
        assert!(!stats.has_failures());
    }

    // Two pages; the second cannot be scraped and the even items cannot be
    // processed. None of that is the crawler's fault, so `run` succeeds and
    // the stats say what was lost.
    struct FlakySpider {
        attempted: AtomicUsize,
    }

    #[async_trait]
    impl Spider for FlakySpider {
        type Item = u32;

        fn name(&self) -> String {
            String::from("flaky")
        }

        fn start_urls(&self) -> Vec<String> {
            vec![String::from("good"), String::from("bad")]
        }

        async fn scrape(&self, url: String) -> Result<(Vec<u32>, Vec<String>), Error> {
            match url.as_str() {
                "good" => Ok((vec![1, 2, 3, 4, 5], Vec::new())),
                other => Err(Error::Internal(format!("{other}: simulated 503"))),
            }
        }

        async fn process(&self, item: u32) -> Result<(), Error> {
            self.attempted.fetch_add(1, Ordering::SeqCst);
            if item.is_multiple_of(2) {
                Err(Error::Internal(format!(
                    "item {item}: simulated write failure"
                )))
            } else {
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn run_counts_scrape_and_process_failures_instead_of_hiding_them() {
        let spider = Arc::new(FlakySpider {
            attempted: AtomicUsize::new(0),
        });
        let dyn_spider: Arc<dyn Spider<Item = u32>> = spider.clone();
        let crawler = Crawler::new(Duration::from_millis(0), 2, 2);

        let stats = tokio::time::timeout(Duration::from_secs(2), crawler.run(dyn_spider))
            .await
            .expect("crawler should finish")
            .expect("spider failures are not crawler failures");

        // every item still reached `process` before `run` returned
        assert_eq!(spider.attempted.load(Ordering::SeqCst), 5);
        assert_eq!(
            stats,
            CrawlStats {
                pages_scraped: 1,
                scrape_errors: 1,
                items_processed: 3,
                process_errors: 2,
                ..Default::default()
            }
        );
        assert!(stats.has_failures());
    }

    #[tokio::test]
    async fn run_with_no_start_urls_returns_immediately() {
        struct EmptySpider;

        #[async_trait]
        impl Spider for EmptySpider {
            type Item = ();
            fn name(&self) -> String {
                String::from("empty")
            }
            fn start_urls(&self) -> Vec<String> {
                Vec::new()
            }
            async fn scrape(&self, _url: String) -> Result<(Vec<()>, Vec<String>), Error> {
                unreachable!("nothing should be scraped")
            }
            async fn process(&self, _item: ()) -> Result<(), Error> {
                Ok(())
            }
        }

        let crawler = Crawler::new(Duration::from_millis(0), 2, 1);
        let spider: Arc<dyn Spider<Item = ()>> = Arc::new(EmptySpider);

        tokio::time::timeout(Duration::from_secs(2), crawler.run(spider))
            .await
            .expect("crawler should finish")
            .expect("nothing can fail here");
    }

    // One root page that links to more pages than both channels can hold.
    struct LeafySpider {
        leaves: usize,
        scraped: AtomicUsize,
    }

    #[async_trait]
    impl Spider for LeafySpider {
        type Item = ();

        fn name(&self) -> String {
            String::from("leafy")
        }

        fn start_urls(&self) -> Vec<String> {
            vec![String::from("root")]
        }

        async fn scrape(&self, url: String) -> Result<(Vec<()>, Vec<String>), Error> {
            self.scraped.fetch_add(1, Ordering::SeqCst);
            if url == "root" {
                let leaves = (0..self.leaves).map(|i| format!("leaf-{i}")).collect();
                Ok((Vec::new(), leaves))
            } else {
                Ok((Vec::new(), Vec::new()))
            }
        }

        async fn process(&self, _item: ()) -> Result<(), Error> {
            Ok(())
        }
    }

    // With concurrency 2 both channels hold 800. The control loop used to
    // block on `send` into a full `urls_to_visit` while never draining
    // `new_urls`; once 800 reports piled up, the scrapers blocked too and
    // nothing moved. One page with more than 1,600 unseen links was enough.
    #[tokio::test]
    async fn run_survives_a_page_with_more_links_than_the_channels_hold() {
        let spider = Arc::new(LeafySpider {
            leaves: 1_700,
            scraped: AtomicUsize::new(0),
        });
        let dyn_spider: Arc<dyn Spider<Item = ()>> = spider.clone();
        let crawler = Crawler::new(Duration::from_millis(0), 2, 1);

        tokio::time::timeout(Duration::from_secs(5), crawler.run(dyn_spider))
            .await
            .expect("crawler should finish instead of deadlocking")
            .expect("a well-behaved spider should not produce an error");

        assert_eq!(spider.scraped.load(Ordering::SeqCst), 1_701);
    }

    // A frontier that never runs dry: every page links to the next one and
    // yields one item.
    struct EndlessSpider;

    #[async_trait]
    impl Spider for EndlessSpider {
        type Item = usize;

        fn name(&self) -> String {
            String::from("endless")
        }

        fn start_urls(&self) -> Vec<String> {
            vec![String::from("page-0")]
        }

        async fn scrape(&self, url: String) -> Result<(Vec<usize>, Vec<String>), Error> {
            let n: usize = url
                .trim_start_matches("page-")
                .parse()
                .map_err(|_| Error::Internal(format!("bad test url {url}")))?;
            Ok((vec![n], vec![format!("page-{}", n + 1)]))
        }

        async fn process(&self, _item: usize) -> Result<(), Error> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn run_stops_at_the_page_limit() {
        let spider: Arc<dyn Spider<Item = usize>> = Arc::new(EndlessSpider);
        let crawler = Crawler::new(Duration::from_millis(0), 2, 1).with_max_pages(Some(10));

        let stats = tokio::time::timeout(Duration::from_secs(2), crawler.run(spider))
            .await
            .expect("the page limit should end an endless crawl")
            .expect("a page limit is not a failure");

        assert_eq!(stats.stop_reason, Some(StopReason::PageLimit));
        assert_eq!(stats.pages_scraped, 10);
        assert_eq!(stats.items_processed, 10);
        assert_eq!(stats.frontier_remaining, 1);
        assert!(!stats.has_failures());
    }

    // The edge that would otherwise hang: a limit of zero must stop before
    // the first dispatch, not wait for a report that never comes.
    #[tokio::test]
    async fn run_with_a_zero_page_limit_returns_immediately() {
        let spider: Arc<dyn Spider<Item = usize>> = Arc::new(EndlessSpider);
        let crawler = Crawler::new(Duration::from_millis(0), 2, 1).with_max_pages(Some(0));

        let stats = tokio::time::timeout(Duration::from_secs(2), crawler.run(spider))
            .await
            .expect("crawler should finish")
            .expect("a page limit is not a failure");

        assert_eq!(stats.stop_reason, Some(StopReason::PageLimit));
        assert_eq!(stats.pages_scraped, 0);
        assert_eq!(stats.frontier_remaining, 1);
    }

    // A limit that is never reached must not be reported as a stop.
    #[tokio::test]
    async fn run_with_a_generous_page_limit_finishes_normally() {
        let spider = Arc::new(CountingSpider {
            scraped: AtomicUsize::new(0),
            processed: AtomicUsize::new(0),
        });
        let dyn_spider: Arc<dyn Spider<Item = u32>> = spider.clone();
        let crawler = Crawler::new(Duration::from_millis(0), 2, 1).with_max_pages(Some(100));

        let stats = tokio::time::timeout(Duration::from_secs(2), crawler.run(dyn_spider))
            .await
            .expect("crawler should finish")
            .expect("nothing fails here");

        assert_eq!(stats.stop_reason, None);
        assert_eq!(stats.pages_scraped, 2);
    }

    #[tokio::test]
    async fn run_stops_when_cancelled_and_finishes_in_flight_pages() {
        let spider: Arc<dyn Spider<Item = usize>> = Arc::new(EndlessSpider);
        let token = CancellationToken::new();
        // a small delay per page so the crawl is still going when we cancel
        let crawler = Crawler::new(Duration::from_millis(1), 2, 1).with_cancellation(token.clone());

        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            token.cancel();
        });

        let stats = tokio::time::timeout(Duration::from_secs(2), crawler.run(spider))
            .await
            .expect("cancellation should end an endless crawl")
            .expect("cancellation is not a failure");

        assert_eq!(stats.stop_reason, Some(StopReason::Cancelled));
        assert!(
            stats.pages_scraped > 0,
            "the crawl should have made progress"
        );
        // every page handed out was reported and its item processed: the loop
        // waited for in-flight work instead of abandoning it
        assert_eq!(stats.items_processed, stats.pages_scraped);
        assert!(!stats.has_failures());
    }
}
