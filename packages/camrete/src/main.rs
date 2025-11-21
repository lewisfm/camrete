use std::{sync::LazyLock, time::Duration};

use camrete_core::{
    database::models::{Module, ModuleRelease, module::{ModuleRelationship, ModuleRelationshipGroup}},
    diesel::{self, OptionalExtension, QueryDsl, RunQueryDsl},
    json::ReleaseStatus,
    repo::client::RepoManager,
};
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use miette::Diagnostic;
use owo_colors::OwoColorize;
use termimad::MadSkin;
use thiserror::Error;
use time::{format_description::BorrowedFormatItem, macros::format_description};
use tracing_subscriber::{EnvFilter, util::SubscriberInitExt};

#[derive(Debug, Error, Diagnostic)]
enum CliError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Core(#[from] camrete_core::Error),

    #[error("No such module: {0}")]
    #[diagnostic(code(camrete::module_not_found))]
    ModuleNotFound(String),
}

impl From<diesel::result::Error> for CliError {
    fn from(value: diesel::result::Error) -> Self {
        camrete_core::Error::from(value).into()
    }
}

#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    Update {},
    /// Show the details for a mod.
    Show {
        identifier: String,
    },
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
        Command::Show { identifier } => {
            show(&mut repo_mgr, identifier).await?;
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

async fn show(repo_mgr: &mut RepoManager, slug: String) -> Result<(), CliError> {
    let md_skin = MadSkin::default();

    let mut db = repo_mgr.db()?;

    let Some(module) = Module::all()
        .filter(Module::with_slug(&slug))
        .get_result(db.as_mut())
        .optional()?
    else {
        return Err(CliError::ModuleNotFound(slug));
    };

    let releases: Vec<ModuleRelease> = ModuleRelease::all()
        .filter(ModuleRelease::with_parent(module.id))
        .order_by(ModuleRelease::by_version())
        .load(db.as_mut())?;

    let mut releases = releases.into_iter();
    let Some(first) = releases.next() else {
        return Err(CliError::ModuleNotFound(slug));
    };

    let tags = ModuleRelease::tags_for(first.id).load::<String>(db.as_mut())?;
    let authors = ModuleRelease::authors_for(first.id).load::<String>(db.as_mut())?;
    let licenses = ModuleRelease::licenses_for(first.id).load::<String>(db.as_mut())?;

    print!("{} {}", first.display_name.bright_green(), first.version);
    for tag in tags {
        print!(" {}", format!("#{tag}").blue());
    }
    if first.release_status != ReleaseStatus::Stable {
        print!(" ({})", format!("{:?}", first.release_status).red());
    }
    println!();

    println!("\n{}", md_skin.term_text(&first.summary));

    if let Some(description) = &first.description {
        println!("{}", md_skin.term_text(description));
        println!();
    }

    let resources = &first.metadata.resources;
    if let Some(homepage) = &resources.homepage {
        println!("{}", homepage.bold());
    }

    println!("Authors: {}", authors.join(", "));
    println!("License: {}", licenses.join(" or "));


    if let Some(link) = &resources.bugtracker {
        println!("Bug tracker: {}", link.bold());
    }
    if let Some(link) = &resources.repository {
        println!("Repository: {}", link.bold());
    }
    if let Some(link) = &resources.spacedock {
        println!("Spacedock: {}", link.bold());
    }

    if let Some(release_date) = first.release_date
        && let Ok(date_str) = release_date.format(DATE_TIME_FMT)
    {
        println!("Release date: {}", date_str);
    }

    if releases.len() != 0 {
        print!(
            "Other versions: {}",
            releases
                .by_ref()
                .map(|r| r.version)
                .take(3)
                .collect::<Vec<_>>()
                .join(", ")
        );

        let remaining = releases.len();
        if remaining != 0 {
            print!(" and {remaining} others");
        }

        println!();
    }

    let dep_groups = ModuleRelationshipGroup::all()
        .filter(ModuleRelationshipGroup::for_release(first.id))
        .load(db.as_mut())?;

    println!("\nRelationships:");

    if dep_groups.is_empty() {
        println!("  (None)");
    }

    for group in dep_groups {
        let members = ModuleRelationship::all()
            .filter(ModuleRelationship::in_group(group.id))
            .load(db.as_mut())?;
        let is_any_of = members.len() > 1;

        print!("  ({:?}) ", group.rel_type);

        if is_any_of {
            println!("- Any of:");
        }

        for member in members {
            if is_any_of {
                print!("    ");
            }
            print!("- {}", member.target_name);
            println!()
        }
    }

    Ok(())
}

const PROGRESS_CHARS: &str = "=> ";
pub static PROGRESS_STYLE_DOWNLOAD: LazyLock<ProgressStyle> = LazyLock::new(|| {
    ProgressStyle::with_template(
        "Download {percent:>3.bold}% [{bar:40.green}] [{decimal_bytes:>9}/{decimal_total_bytes:9}]",
    )
    .expect("progress style valid")
    .progress_chars(PROGRESS_CHARS)
});

pub static PROGRESS_STYLE_SPINNER: LazyLock<ProgressStyle> = LazyLock::new(|| {
    ProgressStyle::with_template("{spinner:.green} {msg:20}")
        .expect("progress style valid")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏✓")
});

const DATE_TIME_FMT: &[BorrowedFormatItem] =
    format_description!("[day] [month repr:short] [year], [hour]:[minute]");
