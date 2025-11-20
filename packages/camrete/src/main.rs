use std::{
    env::args,
    sync::{Arc, LazyLock},
    time::Duration,
};

use camrete_core::{database::models::RepositoryRef, repo::client::RepoManager};
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tracing_subscriber::{EnvFilter, util::SubscriberInitExt};
use url::Url;

#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    Update {},
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::fmt()
        .pretty()
        .with_env_filter(EnvFilter::from_default_env())
        .finish()
        .init();

    let args = Args::parse();

    let mut repo_mgr = RepoManager::new("development.db")?;

    match args.command {
        Command::Update {} => {
            update(&mut repo_mgr).await?;
        }
    }

    Ok(())
}

async fn update(repo_mgr: &mut RepoManager) -> camrete_core::Result<()> {
    let all_repos = repo_mgr.db()?.all_repos(true)?;

    for repo in all_repos {
        println!("Updating {} ({})", repo.name, repo.url);

        let bars = MultiProgress::new();

        let download_bar = ProgressBar::no_length().with_style(PROGRESS_STYLE_DOWNLOAD.clone());
        bars.add(download_bar.clone());
        download_bar.enable_steady_tick(Duration::from_millis(100));

        let unpack_bar = ProgressBar::no_length().with_style(PROGRESS_STYLE_SPINNER.clone());
        bars.add(unpack_bar.clone());
        unpack_bar.enable_steady_tick(Duration::from_millis(100));

        repo_mgr
            .download(&repo, {
                let download_bar = download_bar.clone();
                let unpack_bar = unpack_bar.clone();

                Box::new(move |p| {
                    if p.is_computing_derived_data {
                        unpack_bar.set_message("Rebuilding derived data...");
                    } else {
                        unpack_bar.set_message(format!("{} items unpacked", p.items_unpacked));
                    }


                    if download_bar.is_finished() {
                        return;
                    }

                    download_bar.set_position(p.bytes_downloaded);
                    if let Some(bytes_expected) = p.bytes_expected {
                        download_bar.set_length(bytes_expected);

                        if p.bytes_downloaded >= bytes_expected {
                            download_bar.finish();
                        }
                    }

                })
            })
            .await?;

        download_bar.finish();
        unpack_bar.finish_with_message("Update complete");
    }

    Ok(())
}

const PROGRESS_CHARS: &str = "=> ";
pub static PROGRESS_STYLE_DOWNLOAD: LazyLock<ProgressStyle> = LazyLock::new(|| {
    ProgressStyle::with_template("Download {percent:>3.bold}% [{bar:40.green}] [{decimal_bytes:>9}/{decimal_total_bytes:9}]")
        .expect("progress style valid")
        .progress_chars(PROGRESS_CHARS)
});

pub static PROGRESS_STYLE_SPINNER: LazyLock<ProgressStyle> = LazyLock::new(|| {
    ProgressStyle::with_template("{spinner:.green} {msg:20}")
        .expect("progress style valid")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏✓")
});
