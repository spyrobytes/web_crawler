use crate::error::Error;
use crate::spiders::Spider;
use futures::stream::StreamExt;
use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::mpsc,
    task::{JoinError, JoinHandle},
    time::sleep,
};

pub struct Crawler {
    delay: Duration,
    crawling_concurrency: usize,
    processing_concurrency: usize,
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
        }
    }

    pub async fn run<T: Send + 'static>(
        &self,
        spider: Arc<dyn Spider<Item = T>>,
    ) -> Result<(), Error> {
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
        loop {
            if frontier.is_empty() && outstanding == 0 {
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
                permit = urls_to_visit_tx.reserve(), if !frontier.is_empty() => {
                    let Ok(permit) = permit else {
                        // The receiver is gone: same situation as above.
                        log::error!(
                            "crawler: scrapers stopped with {outstanding} urls outstanding"
                        );
                        break;
                    };

                    let url = frontier.pop_front().expect("branch is guarded by !is_empty");
                    outstanding += 1;
                    permit.send(url);
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

        scrapers.and(processors)
    }

    // Launching the processors is a matter of spawning a new task with a
    // stream and for_each_concurrent. The task ends when the stream ends, and
    // the caller learns about it through the returned join handle.
    fn launch_processors<T: Send + 'static>(
        &self,
        concurrency: usize,
        spider: Arc<dyn Spider<Item = T>>,
        items: mpsc::Receiver<T>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            tokio_stream::wrappers::ReceiverStream::new(items)
                .for_each_concurrent(concurrency, |item| async {
                    let _ = spider.process(item).await;
                })
                .await;
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
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            tokio_stream::wrappers::ReceiverStream::new(urls_to_visit)
                .for_each_concurrent(concurrency, |queued_url| async {
                    let mut urls = Vec::new();
                    let res = spider
                        .scrape(queued_url.clone())
                        .await
                        .map_err(|err| {
                            log::error!("{}", err);
                            err
                        })
                        .ok();

                    if let Some((items, new_urls)) = res {
                        for item in items {
                            let _ = items_tx.send(item).await;
                        }
                        urls = new_urls;
                    }

                    let _ = new_urls_tx.send((queued_url, urls)).await;
                    sleep(delay).await;
                })
                .await;

            drop(items_tx);
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

        tokio::time::timeout(Duration::from_secs(2), crawler.run(dyn_spider))
            .await
            .expect("crawler should finish")
            .expect("a well-behaved spider should not produce an error");

        assert_eq!(spider.scraped.load(Ordering::SeqCst), 2);
        assert_eq!(spider.processed.load(Ordering::SeqCst), 5);
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
}
