# Hardening the crawler: issue list

The book's crawler is a readable chapter, not a tool you can leave running.
This is the running list of what stands between the two, found while reviewing
and hardening the crate. Each entry says where the problem is, what goes wrong,
and the fix. Status: **done**, **in progress**, or **open**.

## Control loop and shutdown (`crawler.rs`)

1. **Active-spider count not paired with its decrement.** `done` (29a41a6)
   The increment and decrement were two statements. A panic or an early drop of
   the per-URL future skipped the decrement and the control loop waited
   forever. Fixed with `ActiveSpiderGuard`, which decrements in `Drop`.

2. **Shutdown waits on a barrier that a dead task never reaches.** `done`
   `Barrier::new(3)` assumes all three parties arrive. A task that panicked is
   dropped by tokio and never calls `wait`, so `run` hangs after the control
   loop has already exited. Fix: both launch functions return their
   `JoinHandle`; `run` awaits them in order and turns a `JoinError` into an
   `Error`. The barrier, its `Arc` clones, and the magic number three are gone.
   Covered by `run_returns_an_error_after_a_spider_panics`.

3. **`run` cannot report failure.** `done`
   It returns `()`, so a crawl whose scraper task died still exits with status
   zero. `run` now returns `Result<(), Error>` and `main` propagates it with `?`.

4. **Termination check can fire while a URL is between the channel and its
   guard.** `open`
   A URL leaves `urls_to_visit` (restoring the channel's capacity) slightly
   before its `ActiveSpiderGuard` exists. The control loop, on another worker
   thread, can observe "channels empty, count zero" in that gap and exit early.
   Fix: count a URL as outstanding when the control loop enqueues it and
   release it when the scraper reports back, so the count never dips during
   the hand-off.

5. **Bounded channels in a cycle can deadlock.** `open`
   The control loop blocks on `urls_to_visit_tx.send(..).await` when that
   channel is full, and while blocked it does not drain `new_urls_rx`. Scrapers
   block on `new_urls_tx.send(..).await` when that fills. Once both are full
   nobody moves. With capacity 800 and pages that link to ~100 new URLs each,
   this is reachable on a real site within seconds. Fix: the control loop
   must never block on a send; keep its own unbounded `VecDeque` of pending
   URLs and feed the channel only when there is capacity (`try_send` or a
   `select!` over recv and send).

6. **Control loop polls instead of awaiting.** `open`
   `try_recv` followed by `sleep(5ms)` wakes two hundred times a second doing
   nothing. Fix folds into item 5: `select!` over `new_urls_rx.recv()` and the
   pending send, with the termination check after each event.

7. **No way to stop a crawl.** `open`
   There is no cancellation token, no page limit, and no Ctrl-C handling. A
   crawl runs until the frontier is empty. Fix: a `CancellationToken` (or a
   `watch` channel) checked by the control loop, plus an optional
   `max_pages`.

8. **Processing errors are swallowed.** `open`
   `let _ = spider.process(item).await;` discards the error without even a
   log line. Fix: log it at error level; later, count failures so `run` can
   report them.

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
    `name()` that nothing calls (hence the dead-code warning). Fix: one
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
