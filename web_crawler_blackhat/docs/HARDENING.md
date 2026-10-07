# Hardening the crawler: issue list

The book's crawler is a readable chapter, not a tool you can leave running.
This is the running list of what stands between the two, found while reviewing
and hardening the crate. Each entry says where the problem is, what goes wrong,
and the fix. Status: **done**, **in progress**, or **open**.

## Control loop and shutdown (`crawler.rs`)

1. **Active-spider count not paired with its decrement.** `done` (29a41a6, then superseded)
   The increment and decrement were two statements. A panic or an early drop of
   the per-URL future skipped the decrement and the control loop waited
   forever. First fixed with `ActiveSpiderGuard`, which decrements in `Drop`.
   The control-loop rewrite for item 5 then removed the shared counter
   altogether: the loop counts URLs from hand-over to report, and a dead
   scraper task is detected by the `new_urls` channel closing.

2. **Shutdown waits on a barrier that a dead task never reaches.** `done`
   `Barrier::new(3)` assumes all three parties arrive. A task that panicked is
   dropped by tokio and never calls `wait`, so `run` hangs after the control
   loop has already exited. Fix: both launch functions return their
   `JoinHandle`; `run` awaits them in order and turns a `JoinError` into an
   `Error`. The barrier, its `Arc` clones, and the magic number three are gone.
   Covered by `run_returns_an_error_after_a_spider_panics`.

3. **`run` cannot report failure.** `done`
   It returns `()`, so a crawl whose scraper task died still exits with status
   zero. `run` now returns `Result<CrawlStats, Error>`: `Err` for the crawler's
   own machinery failing, `Ok(stats)` with counts of pages scraped, scrape
   errors, items processed and processing errors otherwise. `main` logs the
   summary and exits non-zero if anything was lost.

4. **Termination check can fire while a URL is between the channel and its
   guard.** `done`
   A URL leaves `urls_to_visit` (restoring the channel's capacity) slightly
   before its `ActiveSpiderGuard` exists. The control loop, on another worker
   thread, can observe "channels empty, count zero" in that gap and exit early.
   Fixed with item 5: `outstanding` is incremented when the control loop
   hands a URL over and decremented when the report arrives, so the count
   never dips during the hand-off.

5. **Bounded channels in a cycle can deadlock.** `done`
   The control loop blocks on `urls_to_visit_tx.send(..).await` when that
   channel is full, and while blocked it does not drain `new_urls_rx`. Scrapers
   block on `new_urls_tx.send(..).await` when that fills. Once both are full
   nobody moves. With capacity 800 and pages that link to ~100 new URLs each,
   this is reachable on a real site within seconds; a single page with more
   than 1,600 unseen links was enough. Fixed: the control loop owns a
   `VecDeque` frontier and uses `select!` over `new_urls_rx.recv()` and
   `urls_to_visit_tx.reserve()`, so it never blocks on a send. Covered by
   `run_survives_a_page_with_more_links_than_the_channels_hold`.

6. **Control loop polls instead of awaiting.** `done`
   `try_recv` followed by `sleep(5ms)` wakes two hundred times a second doing
   nothing. Fixed with item 5: the loop now sleeps inside `select!` until a
   report arrives or channel capacity frees up.

7. **No way to stop a crawl.** `open`
   There is no cancellation token, no page limit, and no Ctrl-C handling. A
   crawl runs until the frontier is empty. Fix: a `CancellationToken` (or a
   `watch` channel) checked by the control loop, plus an optional
   `max_pages`.

8. **Processing errors are swallowed.** `done`
   `let _ = spider.process(item).await;` discarded the error without even a
   log line. Now logged with the spider's name and counted in `CrawlStats`
   (item 3), alongside scrape failures. The remaining `let _ =` discards on
   channel sends carry a comment saying why they are safe. Covered by
   `run_counts_scrape_and_process_failures_instead_of_hiding_them`.

9. **Delay is per slot, not per host.** `open` (roadmap: politeness)
   `sleep(delay)` inside each concurrency slot bounds throughput to
   `concurrency / delay` across all hosts. Real politeness is per host, with
   `robots.txt` and backoff on 429/5xx.

## CLI (`main.rs`)

10. **`RUST_LOG` is overwritten.** `open`
    `env::set_var("RUST_LOG", ..)` runs before `env_logger::init()`, so the
    user's setting is ignored. Fix: `Env::default().default_filter_or(..)`.

11. **Spider names are listed twice.** `open`
    The `spiders` subcommand prints a hard-coded list; each spider also has a
    `name()` that until recently nothing called (it now prefixes the
    scrape and process error logs). Fix: one
    registry of constructors keyed by `name()`, used by both subcommands.

## Errors (`error.rs`)

12. **`Internal` discarded its payload.** `done` (471f07a)
    The display string was `"Internal"`, so every message built by a spider
    logged as the bare word.

13. **Errors are stringly and carry no retry information.** `open`
    (roadmap: structured errors) Callers cannot tell a 429 from a parse error.
    Fix: variants per failure kind, with the source error attached where the
    `From` impls currently flatten it to a `String`.

## Spiders (`spiders/`)

14. **Unwraps on site markup.** `done` (471f07a)
    Replaced with errors; bad rows are skipped with a warning so pagination
    survives. Covered by inline-HTML unit tests.

15. **No HTTP status check.** `open`
    `send().await?` does not fail on 4xx/5xx. A GitHub rate-limit response
    (403/429) is fed to `.json()`, fails to parse, and is logged as a reqwest
    error; pagination then stops silently. Fix: `error_for_status()` and a
    distinct error variant for rate limiting.

16. **"Zero items" on a page that should have some is not detected.** `open`
    cvedetails has changed its markup since the book; the spider now returns
    zero rows and exits cleanly, which looks like success. Fix: a spider
    should be able to say "this page should have had items", and the crawler
    should report pages that produced nothing.

17. **URL normalisation is hand-rolled per spider and the `url` crate is
    unused.** `open` (roadmap: consolidation)
    Each spider string-prefixes relative links. `web_crawler/` already has a
    correct `Url::join`-based version with tests; move it here.

18. **The quotes spider serialises on a mutex.** `open` (documentation only)
    One WebDriver client behind `tokio::sync::Mutex` means crawling
    concurrency is effectively one for that spider. Fine for now; say so in
    the spider.

## Tests

19. **No end-to-end crawler test.** `done`
    Nothing proves the control loop terminates or that processing finishes
    before `run` returns. Now covered in `crawler.rs` by a canned spider that
    panics (`run` returns an error instead of hanging) and a counting spider
    whose five items are all processed before `run` returns.

20. **No network-free fetch tests.** `open` (roadmap)
    Spider `scrape` paths that touch HTTP are untested. Fix: a local `axum` or
    `wiremock` server in tests.
