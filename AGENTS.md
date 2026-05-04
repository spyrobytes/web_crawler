# Repository Guidelines

## Project Structure & Module Organization

This is a Cargo workspace centered on `web_crawler/`. The main crawler binaries live in `web_crawler/src/main.rs` and `web_crawler/src/web_crawler_v2.rs`. Supporting learning crates are organized by topic: `crawler_playground/` for scraping experiments, `concurrency_pattern/` for Rust concept drills, `wiki_crawler/` for Rayon-based page processing, and `web_crawler_blackhat/` for a fuller reference crawler. Notes and diagrams live in `docs/`; small sample files live in `data/`.

## Build, Test, and Development Commands

- `cargo build`: build the full workspace.
- `cargo check`: type-check the workspace quickly.
- `cargo run --package web_crawler --bin web_crawler_main`: run the main crawler test drive.
- `cargo run --package web_crawler --bin web_crawler_v2`: run the v2 crawler binary.
- `cargo test --package web_crawler --bin web_crawler_main`: run focused tests for the main crawler.
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
