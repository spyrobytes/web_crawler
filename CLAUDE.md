# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Purpose and end goal

This workspace is a Rust learning journal with one destination: turn the owner's
growing understanding of Rust into a crawler that solves a real-world problem.
The scaffold is the chapter 5 crawler from *Black Hat Rust* by Sylvain Kerkour,
living in `web_crawler_blackhat/`. Everything else in the workspace is a drill or
prototype that fed into it.

Two things follow from that:

- **Understanding is part of the deliverable.** Explain the mechanism behind a
  bug before fixing it, and leave a short comment at the site saying why the
  code is shaped the way it is. Preserve existing comments that explain Rust
  concepts.
- **Judge changes against "does this move the blackhat crawler toward a tool
  someone could run unattended".** Compiling is the floor, not the bar.

The concrete real-world target has not been chosen yet. The three spiders
(`cvedetails`, `github`, `quotes`) are the book's demos. When the target is
chosen, record it here.

## Objectives (in rough order)

Each item is both a robustness gap left by the book and a Rust lesson.

1. **Shutdown correctness** in `crawler.rs`: a `Drop` guard for the
   active-spider counter and awaiting the scraper's join handle, so a panic in
   any future spider cannot hang the control loop. (RAII, tokio task failure.)
2. **Structured errors**: variants per failure kind in `error.rs` so callers
   can decide what to retry, instead of stringly `Internal`. (`thiserror`.)
3. **Persistence**: processors write to SQLite (`sqlx` or `rusqlite`) instead
   of `println!`. (async DB access, `serde` end to end.)
4. **Politeness**: per-host rate limiting, `robots.txt`, retry with backoff on
   429/5xx. (`tokio::time`, semaphores.)
5. **Network-free crawler tests**: crawl a local `axum` or `wiremock` server
   to prove the control loop terminates.
6. **Consolidation**: fold the URL normalisation and deterministic tests from
   `web_crawler/` into the blackhat crate, retire the duplicate
   `web_crawler_v2.rs`, and update `AGENTS.md`/`README.md`, which still call
   `web_crawler/` the centre of the workspace.

The itemised issue list behind these objectives, with status per item, is
`web_crawler_blackhat/docs/HARDENING.md`. Update it when an item is fixed or
a new one is found.

## Commands

`AGENTS.md` lists the general workspace commands and naming conventions. The
ones that matter day to day for the main line:

```bash
cargo check --workspace --all-targets          # whole workspace, fast
cargo test -p web_crawler_blackhat             # blackhat unit tests
cargo test -p web_crawler_blackhat spiders::cvedetails::tests::parses_a_well_formed_row
cargo clippy -p web_crawler_blackhat --all-targets
cargo fmt -p web_crawler_blackhat -- --check   # several files still have pre-existing fmt drift

cargo run -p web_crawler_blackhat -- spiders                 # list spiders
cargo run -p web_crawler_blackhat -- run --spider github     # the one that reliably works live
cargo run -p web_crawler_blackhat -- run --spider github --max-pages 2
```

Runtime notes:

- Logging defaults to `info`; `RUST_LOG` overrides it as usual.
- Ctrl-C stops the crawl gracefully (in-flight pages finish, summary prints,
  exit 130); a second Ctrl-C aborts.
- The `quotes` spider needs a WebDriver on `localhost:4444`
  (`chromedriver --port=4444`; see `web_crawler_blackhat/docs/README.md`).
- The `cvedetails` site now answers our requests with HTTP 403, so a live run
  fails on the first page with that status (before the status check it looked
  like a successful crawl of zero items). Use the tests to exercise that spider.
- Live runs need network access. Prefer deterministic tests: inline HTML for
  parsing, and a `wiremock` server for the fetch path (both HTTP spiders take
  `with_base_url` for this).

## Architecture of the blackhat crawler

The design only makes sense across five files: `spiders/mod.rs`, `crawler.rs`,
`main.rs`, `error.rs`, `links.rs`.

**The `Spider` trait** (`spiders/mod.rs`) is the extension point. A spider
declares an associated `Item` type and two async methods: `scrape(url)` returns
`(items, new_urls)`, and `process(item)` consumes one item. Spiders own their
own HTTP or WebDriver client, expose a `pub const NAME` that `main` uses for
both subcommands, fetch through the checked helpers in `spiders/mod.rs`
(`get_text`, `get_json`: any non-2xx becomes an error before parsing), and
resolve links with `links::resolve(page_url, href)`. The HTTP spiders take
`with_base_url` so tests can point them at a local server. The crawler never
looks inside an item; it is generic over `T` and holds `Arc<dyn Spider<Item = T>>`.

**`Crawler::run`** (`crawler.rs`) wires three parties together with three
bounded mpsc channels:

- The **scraper task**: a `ReceiverStream` over `urls_to_visit`, driven by
  `for_each_concurrent(crawling_concurrency)`. Each scrape sends items into
  `items_tx`, then sends exactly one `(visited_url, new_urls)` report into
  `new_urls_tx` whether it succeeded or not, then sleeps `delay`. The task
  owns the only `new_urls` sender.
- The **processor task**: a `ReceiverStream` over `items`, driven by
  `for_each_concurrent(processing_concurrency)`, calling `spider.process`.
- The **control loop** (runs inline in `run`): the sole owner of the
  `visited_urls` set, a `VecDeque` frontier, and an `outstanding` count of
  URLs handed over but not yet reported. Each turn is a `tokio::select!`
  between a report arriving and `urls_to_visit_tx.reserve()` finding room
  (that branch only enabled while the frontier is non-empty and the crawl
  is not stopping), plus a `CancellationToken` branch. A page limit or a
  cancellation sets `stop_reason`, which stops dispatching; the loop then
  exits when nothing is outstanding, or at once when `new_urls` closes,
  which can only mean the scraper task died. It then drops
  `urls_to_visit_tx`, which ends the scraper stream, which drops `items_tx`,
  which ends the processor stream. `run` awaits both join handles, always
  both, turns a `JoinError` into `Error::Internal`, and otherwise returns
  the tallies each task carried back as `CrawlStats`.

Consequences worth knowing before touching it:

- Dedup lives only in the control loop. Spiders may emit duplicates freely.
- The control loop must never block on a send. `urls_to_visit` and `new_urls`
  form a cycle; a loop that blocked on a full `urls_to_visit` would stop
  draining `new_urls`, scrapers would then block on their reports, and the
  crawl would deadlock (one page with more than twice the channel capacity in
  unseen links was enough). `reserve()` inside `select!` is what prevents it.
- The per-request `delay` sits inside the concurrency slot, so throughput is
  bounded by `crawling_concurrency / delay`, not by the number of URLs.
- A panic inside `scrape` or `process` does **not** crash the process. Tokio
  catches it at the task boundary and the whole scraper (or processor) task is
  dropped. The crawler now survives that (channel closure ends the loop, the
  join handle reports the panic, `run` returns an error), but spiders should
  still return `Error` rather than unwrap so one bad page costs one page, not
  the crawl.
- Scrape and process errors are logged in `crawler.rs` with the spider's
  name, counted in `CrawlStats`, and make `main` exit non-zero; a failed
  scrape still reports its URL as visited with no children. Returning an error from `scrape` therefore skips that page
  and its pagination; the spiders instead skip individual bad rows inside
  `scrape` so pagination survives.

**Errors** (`error.rs`): one `Error` enum grouped by what a caller could do
about it: `Fetch` (transport, with the `reqwest` source), `HttpStatus`,
`RateLimited` (with `Retry-After`), `Parse` (the page is not what the spider
expects, including a missing table or non-JSON body), `WebDriver`, `Internal`
(our own bug), `InvalidSpider`. `is_retryable()` is the hook for backoff;
nothing retries yet. Display strings include their payload, since `crawler.rs`
logs errors by `Display`.

## The other crates

They are drills, not the product. Do not rewrite them unless asked.

- `web_crawler/`: an earlier hand-rolled design (`UrlManager`, `Scraper`,
  `Processor`). Its two binaries are byte-for-byte the same logic despite the
  `v2` changelog header, and its "concurrent" loop awaits each spawned task
  immediately, so it is sequential. Its URL normalisation and tests are the
  parts worth keeping (objective 6).
- `crawler_playground/`: sequential and Rayon scrapers of a fixed blog, plus
  IMDb and Hacker News one-offs. They write fetched pages under `static/`
  (gitignored) using the raw URL path, so treat them as local experiments only.
- `concurrency_pattern/`: associated types, `'static` bounds, progress bars.
  Reads sample files from `data/`.
- `wiki_crawler/`: Rayon over the `wikipedia` crate.

`src/main.rs` at the workspace root is a stray hello-world; the root manifest is
a virtual workspace with no package.

## Conventions specific to this repo

- Keep the learning-oriented, explicit style even where it is more verbose than
  production code (`AGENTS.md`).
- Tests sit in `#[cfg(test)] mod tests` beside the code and use inline HTML;
  no live-site tests without saying so in the test name or a comment.
- Commit subjects are short and imperative; add a body when a change touches
  architecture, dependencies, or crawler behaviour.

## Workspace-wide guidelines

The general build commands, naming conventions, testing and commit guidelines
live in `AGENTS.md` and are included here so both files stay in sync:

@AGENTS.md
