use std::{
    env::args,
    sync::{Arc, LazyLock},
};

use camrete_core::repo::client::RepoClient;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use miette::Diagnostic;
use thiserror::Error;
use tracing_subscriber::{EnvFilter, util::SubscriberInitExt};
use url::Url;

#[derive(Debug, Error, Diagnostic)]
enum CliError {
    #[error("Failed to parse URL\n{0}")]
    #[diagnostic(code(camrete::cli::invalid_url))]
    InvalidUrl(String),
}

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

    let mut repo_client = RepoClient::new("sqlite:development.db").await?;

    let bar = Arc::new(ProgressBar::no_length().with_style(PROGRESS_STYLE_MSG.clone()));

    repo_client
        .download(args.url, {
            let bar = bar.clone();
            move |p| {
                if let Some(bytes_expected) = p.bytes_expected {
                    bar.set_length(bytes_expected);
                }

                bar.set_position(p.bytes_downloaded);
                bar.set_message(format!("{} items unpacked", p.items_unpacked));
            }
        })
        .await?;

    bar.finish();

    Ok(())
}

const PROGRESS_CHARS: &str = "=> ";
pub static PROGRESS_STYLE_MSG: LazyLock<ProgressStyle> = LazyLock::new(|| {
    ProgressStyle::with_template("{percent:>3.bold}% [{bar:40.green}] {msg} ({eta} remaining)")
        .expect("progress style valid")
        .progress_chars(PROGRESS_CHARS)
});
