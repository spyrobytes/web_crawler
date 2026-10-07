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

7. **No way to stop a crawl.** `done`
   There was no cancellation, no page limit, and Ctrl-C killed the process
   mid-write with no summary. Stopping is now "behave as if the frontier were
   empty": a `CancellationToken` branch and a `max_pages` check in the
   control loop set a `stop_reason`, dispatching stops, in-flight pages
   finish, items are processed, and `run` returns stats carrying the reason
   and the frontier size. `main` wires Ctrl-C to the token (a second press
   aborts), adds `--max-pages`, and exits 130 on cancellation. Covered by the
   endless-spider tests. Interrupting an in-flight request itself is still
   open; stop latency is bounded by concurrency × (HTTP timeout + delay).

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

10. **`RUST_LOG` is overwritten.** `done`
    `env::set_var("RUST_LOG", ..)` ran before `env_logger::init()`, so the
    user's setting was ignored. Now `Env::default().default_filter_or("info")`:
    `info` unless the environment says otherwise.

11. **Spider names are listed twice.** `done`
    Each spider now has a `pub const NAME`; `name()` returns it, and `main`
    builds the `spiders` listing and the `run` match from the same constants.

## Errors (`error.rs`)

12. **`Internal` discarded its payload.** `done` (471f07a)
    The display string was `"Internal"`, so every message built by a spider
    logged as the bare word.

13. **Errors are stringly and carry no retry information.** `done`
    `Error` now has `Fetch` (with the `reqwest` source attached), `HttpStatus`,
    `RateLimited` (with `Retry-After` when given), `Parse`, `WebDriver`,
    `Internal` and `InvalidSpider`, plus `is_retryable()`. Nothing retries yet
    (that is item 9); the scrape-error log already says which failures would
    have been worth it.

## Spiders (`spiders/`)

14. **Unwraps on site markup.** `done` (471f07a)
    Replaced with errors; bad rows are skipped with a warning so pagination
    survives. Covered by inline-HTML unit tests.

15. **No HTTP status check.** `done`
    `send().await?` did not fail on 4xx/5xx, so a rate-limit page was fed to
    the parser. `spiders::get_checked`/`get_text`/`get_json` now turn any
    non-2xx into `HttpStatus` or `RateLimited` (429; GitHub's 403 with
    `x-ratelimit-remaining: 0`; 503 with `Retry-After`) before the body is
    looked at. Covered by mock-server tests in both HTTP spiders.

16. **"Zero items" on a page that should have some is not detected.** `done`
    cvedetails now returns `Parse` when a list page has no
    `#vulnslisttable`, and quotes when a page has no `.quote` blocks, instead
    of a successful crawl of nothing. (Live, cvedetails currently answers
    HTTP 403; with item 15 that is reported as such rather than as an empty
    page.) `CrawlStats` also reports `empty_pages`
    (scraped fine, no items, no links) so an all-empty crawl stands out.

17. **URL normalisation is hand-rolled per spider and the `url` crate is
    unused.** `done`
    `links::resolve(page_url, href)` joins with `Url::join`, strips
    fragments, and drops non-page links; both HTML spiders use it. Ported
    from `web_crawler/` with its tests.

18. **The quotes spider serialises on a mutex.** `done` (documented)
    A comment on the field now says that scrapes are serialised and why.

## Tests

19. **No end-to-end crawler test.** `done`
    Nothing proves the control loop terminates or that processing finishes
    before `run` returns. Now covered in `crawler.rs` by a canned spider that
    panics (`run` returns an error instead of hanging) and a counting spider
    whose five items are all processed before `run` returns.

20. **No network-free fetch tests.** `done`
    Both HTTP spiders take `with_base_url(..)` and are tested against a
    `wiremock` server (dev-dependency): happy path with pagination, 5xx, 429
    with `Retry-After`, GitHub's 403 quota signal, and bodies that are not
    the expected page.
