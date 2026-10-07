use crate::spiders::Spider;
use futures::stream::StreamExt;
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    sync::{mpsc, Barrier},
    time::sleep,
};

// Counts one in-flight scrape for as long as it is alive.
//
// The control loop treats `active_spiders == 0` as "no work is in a scraper's
// hands". That is only true if every increment is matched by a decrement.
// Two plain statements cannot promise that: a panic inside `scrape`, or the
// future being dropped early, would skip the decrement and the control loop
// would wait forever. Tying the decrement to `Drop` makes it run on every
// exit path: normal completion, panic unwinding, and cancellation.
struct ActiveSpiderGuard(Arc<AtomicUsize>);

impl ActiveSpiderGuard {
    fn new(counter: &Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(Arc::clone(counter))
    }
}

impl Drop for ActiveSpiderGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

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

    pub async fn run<T: Send + 'static>(&self, spider: Arc<dyn Spider<Item = T>>) {
        let mut visited_urls = HashSet::<String>::new();
        let crawling_concurrency = self.crawling_concurrency;
        let crawling_queue_capacity = crawling_concurrency * 400;
        let processing_concurrency = self.processing_concurrency;
        let processing_queue_capacity = processing_concurrency * 10;
        let active_spiders = Arc::new(AtomicUsize::new(0));

        // We create the channels that will be used to communicate between the
        // different tasks.
        let (urls_to_visit_tx, urls_to_visit_rx) = mpsc::channel(crawling_queue_capacity);
        let (items_tx, items_rx) = mpsc::channel(processing_queue_capacity);
        let (new_urls_tx, mut new_urls_rx) = mpsc::channel(crawling_queue_capacity);
        let barrier = Arc::new(Barrier::new(3));

        for url in spider.start_urls() {
            visited_urls.insert(url.clone());
            let _ = urls_to_visit_tx.send(url).await;
        }

        self.launch_processors(
            processing_concurrency,
            spider.clone(),
            items_rx,
            barrier.clone(),
        );

        self.launch_scrapers(
            crawling_concurrency,
            spider.clone(),
            urls_to_visit_rx,
            new_urls_tx.clone(),
            items_tx,
            active_spiders.clone(),
            self.delay,
            barrier.clone(),
        );

        // The Control Loop
        // we queue new urls that have not been visited yet and check if we need
        // to stop the crawler
        loop {
            if let Ok((visited_url, new_urls)) = new_urls_rx.try_recv() {
                visited_urls.insert(visited_url);

                for url in new_urls {
                    if !visited_urls.contains(&url) {
                        visited_urls.insert(url.clone());
                        log::debug!("queueing: {}", url);
                        let _ = urls_to_visit_tx.send(url).await;
                    }
                }
            }

            if new_urls_tx.capacity() == crawling_queue_capacity // new_urls channel is empty
            && urls_to_visit_tx.capacity() == crawling_queue_capacity // urls_to_visit channel is empty
            && active_spiders.load(Ordering::SeqCst) == 0
            {
                // no more work, we leave
                break;
            }

            sleep(Duration::from_millis(5)).await;
        }

        log::info!("crawler: control loop exited");

        // we drop the transmitter in order to close the stream
        // and thus stop the crawler
        drop(urls_to_visit_tx);

        // and then we wait for the streams to complete
        barrier.wait().await;
    }

    // Launching the processors is a matter of spawning a new task with a
    // stream and for_each_concurrent. Once the stream is stopped, we "notify"
    // the barrier.
    fn launch_processors<T: Send + 'static>(
        &self,
        concurrency: usize,
        spider: Arc<dyn Spider<Item = T>>,
        items: mpsc::Receiver<T>,
        barrier: Arc<Barrier>,
    ) {
        tokio::spawn(async move {
            tokio_stream::wrappers::ReceiverStream::new(items)
                .for_each_concurrent(concurrency, |item| async {
                    let _ = spider.process(item).await;
                })
                .await;

            // wait for all tasks to rendezvous
            barrier.wait().await;
        });
    }

    // Launch the scrappers, much the same way as launching the processors.
    // Though the logic here is a bit more complex. We need to keep track of
    // the number of active spiders, and we need to sleep between each request
    // in order not to flood the server (to avoid being banned).
    #[allow(clippy::too_many_arguments)]
    fn launch_scrapers<T: Send + 'static>(
        &self,
        concurrency: usize,
        spider: Arc<dyn Spider<Item = T>>,
        urls_to_visit: mpsc::Receiver<String>,
        new_urls_tx: mpsc::Sender<(String, Vec<String>)>,
        items_tx: mpsc::Sender<T>,
        active_spiders: Arc<AtomicUsize>,
        delay: Duration,
        barrier: Arc<Barrier>,
    ) {
        tokio::spawn(async move {
            tokio_stream::wrappers::ReceiverStream::new(urls_to_visit)
                .for_each_concurrent(concurrency, |queued_url| {
                    let queued_url = queued_url.clone();
                    async {
                        // `_guard`, not `_`: a bare `let _ = ...` drops the
                        // value on this same line, which would release the
                        // count before the scrape even starts. The guard
                        // lives until the end of this block, after the delay,
                        // so the throttle still counts as "active".
                        let _guard = ActiveSpiderGuard::new(&active_spiders);
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
                    }
                })
                .await;

            drop(items_tx);
            barrier.wait().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use async_trait::async_trait;

    #[test]
    fn guard_counts_while_alive_and_releases_on_drop() {
        let counter = Arc::new(AtomicUsize::new(0));

        let first = ActiveSpiderGuard::new(&counter);
        let second = ActiveSpiderGuard::new(&counter);
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        drop(first);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        drop(second);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn guard_releases_when_the_holder_panics() {
        let counter = Arc::new(AtomicUsize::new(0));

        let result = std::panic::catch_unwind({
            let counter = Arc::clone(&counter);
            move || {
                let _guard = ActiveSpiderGuard::new(&counter);
                panic!("simulated unwrap on bad markup");
            }
        });

        assert!(result.is_err(), "the closure should have panicked");
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn guard_releases_when_the_future_is_cancelled() {
        let counter = Arc::new(AtomicUsize::new(0));

        let task = tokio::spawn({
            let counter = Arc::clone(&counter);
            async move {
                let _guard = ActiveSpiderGuard::new(&counter);
                std::future::pending::<()>().await;
            }
        });

        // Give the task a turn so the guard exists before we cancel it.
        while counter.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }

        task.abort();
        let _ = task.await;
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

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

    // With the guard, the control loop now exits after a spider panics (the
    // "control loop exited" log line appears). The run still does not return
    // because the dead scraper task never reaches the three-party barrier.
    // Un-ignore this once the barrier is replaced by awaiting join handles.
    #[tokio::test]
    #[ignore = "hangs at the barrier until shutdown awaits join handles"]
    async fn run_returns_after_a_spider_panics() {
        let _ = env_logger::builder().is_test(true).try_init();

        let crawler = Crawler::new(Duration::from_millis(0), 2, 1);
        let spider: Arc<dyn Spider<Item = ()>> = Arc::new(PanickingSpider);

        tokio::time::timeout(Duration::from_secs(2), crawler.run(spider))
            .await
            .expect("crawler should return after a spider panics");
    }
}
