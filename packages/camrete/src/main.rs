use std::{
    env::args,
    sync::{Arc, LazyLock}, time::Duration,
};

use camrete_core::{database::models::RepositoryRef, repo::client::RepoManager};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use tracing_subscriber::{EnvFilter, util::SubscriberInitExt};
use url::Url;

#[derive(Debug, clap::Parser)]
struct Args {
    url: Url,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::fmt()
        .pretty()
        .with_env_filter(EnvFilter::from_default_env())
        .finish()
        .init();

    let args = Args::parse();

    let mut repo_client = RepoManager::new("development.db")?;

    let bar = Arc::new(ProgressBar::no_length().with_style(PROGRESS_STYLE_MSG.clone()));
    bar.enable_steady_tick(Duration::from_millis(100));

    let repo = RepositoryRef::new("KSP-default", &args.url);

    repo_client
        .download(repo, {
            let bar = bar.clone();
            Box::new(move |p| {
                if let Some(bytes_expected) = p.bytes_expected {
                    bar.set_length(bytes_expected);
                }

                bar.set_position(p.bytes_downloaded);
                if p.is_committing {
                    bar.set_message("Committing changes...");
                } else {
                    bar.set_message(format!("{} items unpacked", p.items_unpacked));
                }
            })
        })
        .await?;

    bar.finish_with_message("Done");

    Ok(())
}

const PROGRESS_CHARS: &str = "=> ";
pub static PROGRESS_STYLE_MSG: LazyLock<ProgressStyle> = LazyLock::new(|| {
    ProgressStyle::with_template("{percent:>3.bold}% [{bar:40.green}] {msg} ({eta} remaining)")
        .expect("progress style valid")
        .progress_chars(PROGRESS_CHARS)
});
