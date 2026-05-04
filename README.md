# Rust Web Crawler Learning Workspace

This repository is a Rust learning journal built around one practical project:
building a web crawler from first principles.

The main focus is the `web_crawler` crate. The rest of the workspace contains
small experiments, references, and concept drills for ideas that surfaced while
working on the crawler: async tasks, URL queues, ownership, trait objects,
associated types, HTML parsing, blocking vs non-blocking HTTP, Rayon, Tokio, and
basic crawler architecture.

The goal is not only to produce a crawler, but to document the nitty-gritty of
learning Rust through a real problem.

## Workspace Layout

```text
.
├── web_crawler/             # Main project: crawler structure and iterations
├── crawler_playground/      # Scraping experiments and small crawler prototypes
├── concurrency_pattern/     # Rust concept drills: traits, bounds, progress UI
├── wiki_crawler/            # Parallel Wikipedia example using Rayon
├── web_crawler_blackhat/    # More complete reference-style crawler
├── docs/                    # Notes, architecture sketches, and learning logs
├── data/                    # Small local files for examples
└── static/                  # Generated crawler output, when examples write pages
```

## Main Thread: `web_crawler`

`web_crawler` is where the crawler design is being worked out.

It currently explores:

- URL ownership and queue management with `UrlManager`
- A crawler shape built from `Spider`, `Scraper`, and `Processor`
- Sharing state with `Arc` and `Mutex`
- Running crawl work with Tokio tasks
- Sending completion signals with channels
- Iterating on bugs, such as a URL loop that never terminated

Run the main versions:

```bash
cargo run --package web_crawler --bin web_crawler_main
cargo run --package web_crawler --bin web_crawler_v2
```

Check or lint:

```bash
cargo check --package web_crawler --bin web_crawler_v2
cargo clippy --package web_crawler --bin web_crawler_v2
```

## Supporting Crates

### `crawler_playground`

Small scraping programs used to understand the mechanics before folding ideas
back into the main crawler.

Topics covered:

- Fetching pages with `reqwest`
- Extracting links with `select`
- Using CSS selectors with `scraper`
- Sequential crawling
- Parallel crawling with Rayon
- Writing fetched pages to local files
- Scraping examples such as IMDb and Hacker News

Examples:

```bash
cargo run --package crawler_playground --bin crawler_playground
cargo run --package crawler_playground --bin crawler_playground_concurrent
cargo run --package crawler_playground --bin imdb_web_scraper
```

### `concurrency_pattern`

Focused examples for Rust concepts that support the crawler work.

Topics include:

- Associated types
- Static trait bounds
- File IO
- Terminal spinners and progress indicators

Examples:

```bash
cargo run --package concurrency_pattern --bin associated_types_1
cargo run --package concurrency_pattern --bin static_trait_bound_1
cargo run --package concurrency_pattern --bin animate_with_indicatif
```

### `wiki_crawler`

A compact parallel-processing example using the `wikipedia` crate and Rayon.
It fetches several pages, processes their content, and prints timing metrics.

```bash
cargo run --package webcrawl-wikipedia-rayon
```

### `web_crawler_blackhat`

A more complete crawler reference inspired by Black Hat Rust style architecture.
It has a generic crawler engine and pluggable spiders.

This crate is useful for comparison with the simpler `web_crawler` crate because
it separates:

- The crawler control loop
- The spider trait
- URL scheduling
- Scraping
- Item processing

Examples:

```bash
cargo run --package web_crawler_blackhat -- spiders
cargo run --package web_crawler_blackhat -- run --spider github
```

The `quotes` spider uses WebDriver and expects a compatible driver at
`http://localhost:4444`.

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
2. Read `web_crawler/src/main.rs` to see the first crawler attempt.
3. Read `web_crawler/src/web_crawler_v2.rs` to see the first important fix.
4. Explore `crawler_playground` for scraping mechanics.
5. Use `concurrency_pattern` when a Rust concept needs to be isolated.
6. Compare against `web_crawler_blackhat` for a more developed crawler pattern.

## Requirements

- Rust stable toolchain
- Cargo
- Network access for examples that fetch live websites
- Optional: WebDriver running on `localhost:4444` for the JS-rendered quotes
  spider in `web_crawler_blackhat`

## Common Commands

Build the workspace:

```bash
cargo build
```

Check the workspace:

```bash
cargo check
```

Run Clippy:

```bash
cargo clippy --workspace --all-targets
```

Format:

```bash
cargo fmt
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
