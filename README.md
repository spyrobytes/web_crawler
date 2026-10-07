# Rust Web Crawler Learning Workspace

This repository is a Rust learning journal built around one practical project:
building a web crawler from first principles, and then growing it into a tool
that solves a real-world problem.

The main line is the `web_crawler_blackhat` crate. It started as the chapter 5
crawler from *Black Hat Rust* by Sylvain Kerkour and is the scaffold everything
else now feeds into. The rest of the workspace holds the experiments, concept
drills, and earlier attempts that surfaced along the way: async tasks, URL
queues, ownership, trait objects, associated types, HTML parsing, blocking vs
non-blocking HTTP, Rayon, Tokio, and basic crawler architecture.

The goal is not only to produce a crawler, but to document the nitty-gritty of
learning Rust through a real problem.

## Workspace Layout

```text
.
├── web_crawler_blackhat/    # Main line: generic crawler + site-specific spiders
├── web_crawler/             # Earlier hand-rolled design (UrlManager, Scraper, Processor)
├── crawler_playground/      # Scraping experiments and small crawler prototypes
├── concurrency_pattern/     # Rust concept drills: traits, bounds, progress UI
├── wiki_crawler/            # Parallel Wikipedia example using Rayon
├── docs/                    # Notes, architecture sketches, and learning logs
├── data/                    # Small local files read by the drills
└── static/                  # Generated output from the playground (gitignored)
```

`CLAUDE.md` records the end goal, the ordered objectives, and the architecture
in more depth. `AGENTS.md` holds the workspace conventions.

## Main Thread: `web_crawler_blackhat`

A generic `Crawler` drives any type that implements the `Spider` trait. A spider
owns its own HTTP or WebDriver client and provides two async methods: `scrape`
turns a URL into structured items plus new URLs, and `process` consumes one
item. The crawler owns the URL queue, the visited set, the concurrency limits,
and shutdown.

Inside `Crawler::run` three parties cooperate over bounded channels: a scraper
task, a processor task, and a control loop that dedupes URLs and decides when
the crawl is finished. The design is described in
`web_crawler_blackhat/docs/NOTES.md` and in `CLAUDE.md`.

Three demo spiders ship with it:

| Spider | Source | Notes |
|---|---|---|
| `github` | GitHub REST API, JSON | Works live; paginates until a short page |
| `cvedetails` | HTML table | The site now answers HTTP 403 to the crawler, so a live run fails on the first page and exits non-zero. Parsing and fetching are covered by tests |
| `quotes` | JS-rendered page via WebDriver | Needs a driver on `localhost:4444` |

Run it:

```bash
cargo run --package web_crawler_blackhat -- spiders
cargo run --package web_crawler_blackhat -- run --spider github
cargo run --package web_crawler_blackhat -- run --spider github --max-pages 2
```

Test and lint it:

```bash
cargo test --package web_crawler_blackhat
cargo clippy --package web_crawler_blackhat --all-targets
```

Two things to know before running it:

- Logging defaults to `info`; set `RUST_LOG` to change it.
- The spiders return errors instead of panicking on unexpected markup. The
  crawler logs and counts those, prints a summary at the end, and exits
  non-zero if any page or item was lost.
- Ctrl-C stops the crawl gracefully: nothing new is fetched, in-flight pages
  finish, the summary prints, and the exit code is 130. A second Ctrl-C
  aborts. `--max-pages N` bounds a run the same graceful way.

### Where it is heading

The book's crawler is a readable chapter, not a tool you can leave running. The
next steps, in order, are: making shutdown robust against panics, structured
errors, persistence instead of `println!`, politeness (rate limits, robots.txt,
backoff), network-free tests against a local server, and folding the useful
parts of `web_crawler/` into this crate. The concrete real-world target is still
to be chosen; the demo spiders are placeholders for it.

## Supporting Crates

### `web_crawler`

The first design, built before adopting the book's structure. It explores URL
ownership with a `UrlManager`, a crawler shape made of `Spider`, `Scraper`, and
`Processor`, shared state with `Arc` and `Mutex`, and Tokio tasks with channels.

Two caveats worth knowing: the two binaries contain the same logic despite the
changelog in `web_crawler_v2.rs`, and the main loop awaits each spawned task
immediately, so the crawl is sequential. Its URL normalisation and its
deterministic tests are the parts that will move into the main line.

```bash
cargo run --package web_crawler --bin web_crawler_main
cargo test --package web_crawler
```

### `crawler_playground`

Small scraping programs used to understand the mechanics before folding ideas
back into the crawler: fetching with `reqwest`, extracting links with `select`,
CSS selectors with `scraper`, sequential and Rayon-parallel crawling, writing
fetched pages to `static/`, and IMDb and Hacker News one-offs.

```bash
cargo run --package crawler_playground --bin crawler_playground
cargo run --package crawler_playground --bin crawler_playground_concurrent
cargo run --package crawler_playground --bin imdb_web_scraper
```

### `concurrency_pattern`

Focused examples for Rust concepts that support the crawler work: associated
types, `'static` trait bounds, and terminal progress indicators. The drills read
sample files from `data/`, so run them from the workspace root.

### `wiki_crawler`

A parallel Wikipedia fetch with Rayon and the `wikipedia` crate, timing the work
per page.

## Documentation

The `docs/` directory contains rough notes and learning material, including:

- crawler architecture notes
- concurrency notes
- Rust trait and bound notes
- scraping references
- framework notes

These notes are intentionally part of the repository. They capture the learning
path, not just the final implementation.

## How To Read This Repo

Start here:

1. Read `docs/design-notes.md` for the architecture direction.
2. Read `web_crawler_blackhat/docs/NOTES.md` for the spider and control-loop
   design, then `web_crawler_blackhat/src/crawler.rs` to see it in code.
3. Read `web_crawler_blackhat/src/spiders/cvedetails.rs` for a spider that
   parses HTML defensively, with its tests.
4. Read `web_crawler/src/main.rs` to see the earlier attempt and compare the
   two designs.
5. Explore `crawler_playground` for scraping mechanics.
6. Use `concurrency_pattern` when a Rust concept needs to be isolated.

## Requirements

- Rust stable toolchain and Cargo
- Network access for examples that fetch live websites
- Optional: WebDriver running on `localhost:4444` for the JS-rendered quotes
  spider (see `web_crawler_blackhat/docs/README.md`)

## Common Commands

```bash
cargo check --workspace --all-targets   # type-check everything
cargo build                             # build the workspace
cargo test --workspace                  # all unit tests, no network needed
cargo clippy --workspace --all-targets  # lint
cargo fmt                               # format
```

## Project Philosophy

This is a learning-first codebase. Some modules are intentionally incomplete,
experimental, or verbose because they preserve the reasoning process.

The guiding questions are:

- How do ownership and borrowing shape crawler design?
- Where should state live?
- How should URLs move through the system?
- What belongs in a generic crawler versus a site-specific spider?
- When should concurrency use Tokio, Rayon, channels, or shared state?
- How do small Rust concepts show up in a real application?

The final crawler matters, but the path to understanding it matters just as much.
