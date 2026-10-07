/**
 * Main entry point for the web crawler.
 *
 *
 */
use clap::{Arg, Command};
use std::{sync::Arc, time::Duration};

mod crawler;
mod error;
mod links;
mod spiders;

use crate::crawler::{Crawler, StopReason};
use crate::spiders::{cvedetails::CveDetailsSpider, github::GitHubSpider, quotes::QuotesSpider};
use error::Error;
use tokio_util::sync::CancellationToken;

// One list, derived from each spider's own `NAME`, drives both subcommands.
const SPIDER_NAMES: &[&str] = &[
    CveDetailsSpider::NAME,
    GitHubSpider::NAME,
    QuotesSpider::NAME,
];

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Command::new(clap::crate_name!())
        .version(clap::crate_version!())
        .about(clap::crate_description!())
        .subcommand(Command::new("spiders").about("List all spiders"))
        .subcommand(
            Command::new("run")
                .about("Run a spider")
                .arg(
                    Arg::new("spider")
                        .short('s')
                        .long("spider")
                        .help("The spider to run")
                        .required(true),
                )
                .arg(
                    Arg::new("max-pages")
                        .long("max-pages")
                        .value_parser(clap::value_parser!(usize))
                        .help("Stop after handing out this many pages"),
                ),
        )
        .arg_required_else_help(true)
        .get_matches();

    // `info` unless the user says otherwise. Setting RUST_LOG from inside
    // the program would make the environment variable a no-op.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    if cli.subcommand_matches("spiders").is_some() {
        for name in SPIDER_NAMES {
            println!("{name}");
        }
    } else if let Some(matches) = cli.subcommand_matches("run") {
        // we can safely unwrap as the argument is required
        let spider_name = matches
            .get_one::<String>("spider")
            .expect("spider argument is required")
            .as_str();
        let max_pages = matches.get_one::<usize>("max-pages").copied();

        let cancellation = CancellationToken::new();
        stop_on_ctrl_c(cancellation.clone());

        let crawler = Crawler::new(Duration::from_millis(200), 2, 500)
            .with_max_pages(max_pages)
            .with_cancellation(cancellation);

        let stats = match spider_name {
            CveDetailsSpider::NAME => crawler.run(Arc::new(CveDetailsSpider::new())).await?,
            GitHubSpider::NAME => crawler.run(Arc::new(GitHubSpider::new())).await?,
            QuotesSpider::NAME => crawler.run(Arc::new(QuotesSpider::new().await?)).await?,
            _ => return Err(Error::InvalidSpider(spider_name.to_string()).into()),
        };

        // A cancelled crawl shut down cleanly, but it did not finish. Exit
        // with the conventional status for "interrupted" so a supervisor
        // can tell the two apart.
        if stats.stop_reason == Some(StopReason::Cancelled) {
            std::process::exit(130);
        }

        // The crawl itself ran to completion; whether it counts as a success
        // is the caller's call. For an unattended run, anything lost along
        // the way should show up in the exit code, not just in the log.
        if stats.has_failures() {
            return Err(Error::Internal(format!("crawl finished with failures ({stats})")).into());
        }
    }

    Ok(())
}

// Turn Ctrl-C into a graceful stop. Once tokio has installed its handler the
// default "kill the process" behaviour is gone, so a second Ctrl-C has to be
// handled here too, otherwise a user whose first press seems to do nothing
// has no way out.
fn stop_on_ctrl_c(cancellation: CancellationToken) {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_err() {
            return;
        }
        log::warn!("ctrl-c: stopping after in-flight pages (press again to abort)");
        cancellation.cancel();

        if tokio::signal::ctrl_c().await.is_ok() {
            log::error!("ctrl-c: aborting");
            std::process::exit(130);
        }
    });
}
