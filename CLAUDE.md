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
```

Runtime notes:

- `main.rs` sets `RUST_LOG` unconditionally, so the environment variable has no
  effect until that is changed.
- The `quotes` spider needs a WebDriver on `localhost:4444`
  (`chromedriver --port=4444`; see `web_crawler_blackhat/docs/README.md`).
- The `cvedetails` site no longer serves the table markup the spider parses, so
  a live run exits immediately with zero items. Use the unit tests to exercise
  the parsing path.
- Live runs need network access. Prefer deterministic tests with inline HTML,
  as in `spiders/cvedetails.rs` and `spiders/quotes.rs`.

## Architecture of the blackhat crawler

The design only makes sense across four files: `spiders/mod.rs`, `crawler.rs`,
`main.rs`, `error.rs`.

**The `Spider` trait** (`spiders/mod.rs`) is the extension point. A spider
declares an associated `Item` type and two async methods: `scrape(url)` returns
`(items, new_urls)`, and `process(item)` consumes one item. Spiders own their
own HTTP or WebDriver client and their own URL normalisation. The crawler never
looks inside an item; it is generic over `T` and holds `Arc<dyn Spider<Item = T>>`.

**`Crawler::run`** (`crawler.rs`) wires three parties together with three
bounded mpsc channels and a three-party `Barrier`:

- The **scraper task**: a `ReceiverStream` over `urls_to_visit`, driven by
  `for_each_concurrent(crawling_concurrency)`. Each scrape increments an
  `active_spiders` counter, sends items into `items_tx`, reports
  `(visited_url, new_urls)` into `new_urls_tx`, sleeps `delay`, then decrements.
- The **processor task**: a `ReceiverStream` over `items`, driven by
  `for_each_concurrent(processing_concurrency)`, calling `spider.process`.
- The **control loop** (runs inline in `run`): the only owner of the
  `visited_urls` set. It drains `new_urls_rx`, dedupes, and pushes unseen URLs
  into `urls_to_visit_tx`. It exits when both URL channels are empty **and**
  `active_spiders == 0`, then drops `urls_to_visit_tx`, which ends the scraper
  stream, which drops `items_tx`, which ends the processor stream. All three
  then meet at the barrier.

Consequences worth knowing before touching it:

- Dedup lives only in the control loop. Spiders may emit duplicates freely.
- The per-request `delay` sits inside the concurrency slot, so throughput is
  bounded by `crawling_concurrency / delay`, not by the number of URLs.
- A panic inside `scrape` or `process` does **not** crash the process. Tokio
  catches it at the task boundary, the whole scraper (or processor) task is
  dropped, the counter is never decremented, and the control loop waits
  forever. That is why spiders must return `Error` rather than unwrap, and why
  objective 1 exists.
- Scrape errors are logged in `crawler.rs` and the URL is reported as visited
  with no children. Returning an error from `scrape` therefore skips that page
  and its pagination; the spiders instead skip individual bad rows inside
  `scrape` so pagination survives.
- `main.rs` hard-codes the spider list for the `spiders` subcommand separately
  from each spider's `name()`. Keep them in sync, or better, derive one from
  the other.

**Errors** (`error.rs`): a single `Error` enum with string payloads, plus
`From` impls for reqwest and fantoccini errors. Display strings must include
the payload (`"Internal: {0}"`), since `crawler.rs` logs errors by `Display`.

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
