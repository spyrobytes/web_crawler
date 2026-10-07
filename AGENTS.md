# Repository Guidelines

## Project Structure & Module Organization

This is a Cargo workspace centered on `web_crawler_blackhat/`, the crawler scaffolded on *Black Hat Rust* chapter 5 that is being grown into a real-world tool; its control loop lives in `web_crawler_blackhat/src/crawler.rs` and site-specific spiders under `web_crawler_blackhat/src/spiders/`. Supporting learning crates are organized by topic: `web_crawler/` for the earlier hand-rolled design, `crawler_playground/` for scraping experiments, `concurrency_pattern/` for Rust concept drills, and `wiki_crawler/` for Rayon-based page processing. Notes and diagrams live in `docs/`; small sample files live in `data/`.

## Build, Test, and Development Commands

- `cargo build`: build the full workspace.
- `cargo check`: type-check the workspace quickly.
- `cargo run --package web_crawler_blackhat -- spiders`: list the available spiders.
- `cargo run --package web_crawler_blackhat -- run --spider github`: run the main crawler with one spider.
- `cargo test --package web_crawler_blackhat`: run the main crawler's unit tests (no network needed).
- `cargo run --package web_crawler --bin web_crawler_main`: run the earlier crawler design.
- `cargo clippy --workspace --all-targets`: lint all workspace targets.
- `cargo fmt`: format Rust sources with rustfmt.

Network-backed crawler runs may fail without DNS/network access. Prefer unit tests for deterministic validation.

## Coding Style & Naming Conventions

Use Rust 2021 edition and standard rustfmt formatting. Keep the learning-oriented structure explicit and readable, even when it is more verbose than production code. Use `snake_case` for functions, variables, and modules; `PascalCase` for structs, traits, and enums; and `SCREAMING_SNAKE_CASE` for constants such as `MAX_PAGES`. Prefer clear names like `pending_urls` and `visited_urls` over abbreviations.

## Testing Guidelines

Tests use Rust’s built-in test framework. Keep tests close to the code in `#[cfg(test)] mod tests`. Prefer deterministic tests for URL normalization, filtering, queue behavior, and HTML parsing. Avoid tests that require live websites unless explicitly marked or documented. Run focused checks before broader workspace commands.

## Commit & Pull Request Guidelines

Commit history uses short imperative subjects, for example `Implement working web crawler flow`. Keep the subject concise and add a detailed body when the change affects architecture, dependencies, or crawler behavior. Pull requests should describe the learning goal, implementation changes, commands run, and any network assumptions. Link related notes in `docs/` when relevant.

## Agent-Specific Instructions

Keep changes scoped to the requested crate or file. Do not rewrite learning examples unless asked. Preserve comments that explain Rust concepts, but update stale or incorrect comments when changing behavior. Do not commit generated output from `/target/`, `/static/`, editor files, or local logs.
