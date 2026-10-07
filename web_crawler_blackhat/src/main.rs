/**
 * Main entry point for the web crawler.
 *
 *
 */
use clap::{Arg, Command};
use std::{env, sync::Arc, time::Duration};

mod crawler;
mod error;
mod spiders;

use crate::crawler::{Crawler, StopReason};
use error::Error;
use tokio_util::sync::CancellationToken;

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

    env::set_var("RUST_LOG", "info,crawler=debug");
    env_logger::init();

    if cli.subcommand_matches("spiders").is_some() {
        let spider_names = vec!["cvedetails", "github", "quotes"];
        for name in spider_names {
            println!("{}", name);
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
            "cvedetails" => {
                let spider = Arc::new(spiders::cvedetails::CveDetailsSpider::new());
                crawler.run(spider).await?
            }
            "github" => {
                let spider = Arc::new(spiders::github::GitHubSpider::new());
                crawler.run(spider).await?
            }
            "quotes" => {
                let spider = spiders::quotes::QuotesSpider::new().await?;
                let spider = Arc::new(spider);
                crawler.run(spider).await?
            }
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
