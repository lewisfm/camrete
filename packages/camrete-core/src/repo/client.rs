use std::{
    borrow::Cow,
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use diesel::{
    connection::SimpleConnection,
    delete,
    prelude::*,
    r2d2::{ConnectionManager, Pool},
};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use futures_util::TryStreamExt;
use miette::Diagnostic;
use reqwest::{
    Response,
    header::{ACCEPT, CONTENT_TYPE, ETAG, HeaderValue},
};
use tokio::{
    io::{self},
    task::JoinSet,
};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{debug, info, instrument, trace};
use url::Url;

use crate::{
    DIRS, DbConnection, DbPool, Error, Result, USER_AGENT,
    database::{
        RepoDB,
        models::{BuildRecord, Repository, module::ModuleVersion},
    },
    io::AsyncReadExt as _,
    json::{JsonBuilds, JsonError, JsonModule, RepositoryRefList},
    repo::{
        RepoAsset, RepoAssetBuf, RepoAssetLoader, RepoAssetVariant, TarGzAssetLoader,
        game::GameVersionParseError,
    },
};

mod mime {
    pub const GZIP: &str = "application/gzip";
    pub const X_GZIP: &str = "application/x-gzip";
    pub const ZIP: &str = "application/zip";
}

#[derive(Debug, Error, Diagnostic)]
pub enum RepoUnpackError {
    #[error("cannot determine the repository's data format\n(from {url})")]
    #[diagnostic(code(camrete::repo::download::content_type_missing))]
    MissingContentType { url: Url },
    #[error("cannot unpack {content_type:?} resources\n(from {url})")]
    #[diagnostic(code(camrete::repo::download::unsupported_format))]
    UnsupportedContentType { content_type: String, url: Url },
    #[error(transparent)]
    #[diagnostic(code(camrete::repo::game_version_invalid))]
    GameVersionParse(#[from] GameVersionParseError),
    #[error(
        "a JSON document in the repository was invalid\n\tdocument path: {path}\n\t(from {url})"
    )]
    #[diagnostic(code(camrete::repo::invalid_json))]
    InvalidJsonFile {
        source: JsonError,
        url: Arc<Url>,
        path: PathBuf,
    },
    #[error("the online repository's ETag was not valid UTF-8")]
    #[diagnostic(code(camrete::repo::bad_etag))]
    InvalidEtag { url: Arc<Url> },
    #[error("a release could not be saved to the database: {name:?}, {version:?}")]
    #[diagnostic(code(camrete::repo::bad_release_save))]
    InsertRelease {
        name: String,
        version: String,
        source: diesel::result::Error,
    },
    #[error("couldn't attach download counts to modules")]
    #[diagnostic(code(camrete::repo::bad_download_count_save))]
    InsertDownloadCounts(#[source] diesel::result::Error),
    #[error("couldn't save build ids")]
    #[diagnostic(code(camrete::repo::bad_builds_save))]
    InsertBuilds(#[source] diesel::result::Error),
    #[error("couldn't save repository ref for {name:?} ({url})")]
    #[diagnostic(code(camrete::repo::bad_repo_refs_save))]
    InsertRepoRefs {
        name: String,
        url: Box<Url>,
        source: diesel::result::Error,
    },
}

const MAX_DB_CONNS: u32 = 16;
const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations");

#[derive(Debug, Clone)]
pub struct RepoManager {
    database: DbPool,
    http: reqwest::Client,
}

impl RepoManager {
    pub async fn from_data_dir() -> Result<Self> {
        let repos_file = DIRS.data_local_dir().join("repos.sqlite");
        let url = Url::from_file_path(repos_file).expect("path is valid");

        Self::new(url.as_str())
    }

    pub fn new(url: &str) -> Result<Self> {
        let manager = ConnectionManager::<SqliteConnection>::new(url);
        let pool = Pool::builder().max_size(MAX_DB_CONNS).build(manager)?;

        let mut conn = pool.get()?;

        // see https://fractaledmind.github.io/2023/09/07/enhancing-rails-sqlite-fine-tuning/
        // sleep if the database is busy, this corresponds to up to 2 seconds sleeping
        // time.
        conn.batch_execute("PRAGMA busy_timeout = 2000;")?;
        // better write-concurrency
        conn.batch_execute("PRAGMA journal_mode = WAL;")?;
        // fsync only in critical moments
        conn.batch_execute("PRAGMA synchronous = NORMAL;")?;
        // write WAL changes back every 1000 pages, for an in average 1MB WAL file.
        // May affect readers if number is increased
        conn.batch_execute("PRAGMA wal_autocheckpoint = 1000;")?;
        // free some space by truncating possibly massive WAL files from the last run
        conn.batch_execute("PRAGMA wal_checkpoint(TRUNCATE);")?;

        conn.batch_execute("PRAGMA foreign_keys = ON;")?;

        conn.register_collation("MODULE_VERSION", |left: &str, right: &str| {
            ModuleVersion::from(left).cmp(&ModuleVersion::from(right))
        })?;

        conn.run_pending_migrations(MIGRATIONS)
            .map_err(Error::DbMigrations)?;

        Ok(Self {
            database: pool,
            http: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .expect("http client initialized"),
        })
    }

    pub fn db(&self) -> Result<RepoDB<DbConnection>, Error> {
        Ok(RepoDB::new(self.database.get()?))
    }

    /// Downloads the given repository from an online URL, unpacks it, then
    /// inserts it into the repository database.
    #[instrument(skip(self, progress_reporter))]
    pub async fn download(
        &mut self,
        repo: &Repository,
        progress_reporter: Box<dyn Fn(DownloadProgress) + Send + Sync>,
    ) -> Result<(), Error> {
        info!("Downloading an online CKAN repository");

        let response = self
            .http
            .get(repo.url.clone())
            .header(
                ACCEPT,
                "application/gzip,application/x-gzip,application/zip",
            )
            .send()
            .await?
            .error_for_status()?;

        let download_size = response.content_length();
        let new_etag = response.headers().get(ETAG).cloned();

        let content_type =
            content_type(&response).ok_or_else(|| RepoUnpackError::MissingContentType {
                url: repo.url.clone(),
            })?;

        trace!(%content_type);

        let progress = Arc::new(DownloadProgressReporter::new(
            download_size,
            progress_reporter,
        ));

        let download_stream = response
            .bytes_stream()
            .map_err(io::Error::other)
            .into_async_read()
            .compat()
            .progress(|bytes| {
                progress.report_download_progress(bytes);
            });

        match content_type.as_ref() {
            mime::GZIP | mime::X_GZIP => {
                debug!("Using tar.gz unpacker");

                let loader = TarGzAssetLoader::new(download_stream);
                self.unpack_repo(repo, loader, new_etag, progress.clone())
                    .await?;
            }
            mime::ZIP => todo!("unpacking of .zip repos"),
            _ => {
                return Err(RepoUnpackError::UnsupportedContentType {
                    content_type: content_type.to_string(),
                    url: repo.url.clone(),
                }
                .into());
            }
        };

        Ok(())
    }

    /// Uses the given unpacker to save a repository to the database.
    pub async fn unpack_repo(
        &mut self,
        repo: &Repository,
        loader: impl RepoAssetLoader<'_>,
        etag: Option<HeaderValue>,
        progress: Arc<DownloadProgressReporter>,
    ) -> Result<(), Error> {
        let mut asset_stream = loader.asset_stream()?;
        let repo_url = Arc::new(repo.url.clone());

        // Parse all the assets in parallel as we receive them. The fasted-parsed ones
        // will be inserted into the database first.
        let mut tasks = JoinSet::new();
        while let Some(mut asset) = asset_stream.try_next().await? {
            let repo_url = repo_url.clone();

            tasks.spawn(async move {
                match parse_asset(&mut asset) {
                    Ok(asset) => Ok(asset),
                    Err(Error::Json(err)) => Err(RepoUnpackError::InvalidJsonFile {
                        source: err,
                        url: repo_url,
                        path: asset.path,
                    })?,
                    Err(err) => Err(err),
                }
            });
        }

        let mut db = RepoDB::new(self.database.get()?);

        db.async_transaction(async |mut db| {
            use crate::database::schema::*;

            db.set_etag(repo_url.clone(), etag.as_ref())?;

            // Remove any previous modules so that we are only left with the ones currently
            // included in the repo.
            delete(modules::table)
                .filter(modules::repo_id.eq(repo.repo_id))
                .execute(db.connection)?;

            let mut updated_mods = HashMap::new();

            while let Some(asset) = tasks.join_next().await {
                match asset.unwrap()? {
                    RepoAsset::Release(json) => {
                        let existing_mod_id = updated_mods.get(&json.name).cloned();

                        let (mod_id, _) = db
                            .create_release(&json, repo.repo_id, existing_mod_id)
                            .map_err(|source| RepoUnpackError::InsertRelease {
                            name: json.name.clone(),
                            version: json.version.clone(),
                            source,
                        })?;

                        updated_mods.insert(json.name, mod_id);
                    }
                    RepoAsset::Builds(builds) => {
                        db.register_builds(builds)
                            .map_err(RepoUnpackError::InsertBuilds)?;
                    }
                    RepoAsset::DownloadCounts(counts) => {
                        db.add_download_counts(repo.repo_id, &counts)
                            .map_err(RepoUnpackError::InsertDownloadCounts)?;
                    }
                    RepoAsset::RepositoryRefList(ref_list) => {
                        for new_ref in ref_list.repositories {
                            db.add_repo_ref(repo.repo_id, new_ref.clone())
                                .map_err(|source| RepoUnpackError::InsertRepoRefs {
                                    source,
                                    name: new_ref.name.into_owned(),
                                    url: new_ref.url.into_owned().into(),
                                })?;
                        }
                    }
                }

                progress.report_unpacked_item();
            }

            // progress.report_indexing();

            Ok(())
        })?;

        Ok(())
    }
}

fn parse_asset(asset: &mut RepoAssetBuf) -> Result<RepoAsset> {
    match asset.variant {
        RepoAssetVariant::Release => {
            let parsed: Box<JsonModule> = simd_json::from_slice(&asset.data)?;
            parsed.verify()?;
            Ok(RepoAsset::Release(parsed))
        }
        RepoAssetVariant::DownloadCounts => {
            let map = simd_json::from_slice(&asset.data)?;
            Ok(RepoAsset::DownloadCounts(map))
        }
        RepoAssetVariant::RepositoryRefList => {
            let parsed: RepositoryRefList = simd_json::from_slice(&asset.data)?;
            Ok(RepoAsset::RepositoryRefList(parsed))
        }
        RepoAssetVariant::Builds => {
            let parsed: JsonBuilds = simd_json::from_slice(&asset.data)?;
            let versions = parsed
                .builds
                .into_iter()
                .map(|(build_id, version)| {
                    Ok(BuildRecord {
                        build_id,
                        version: version.parse()?,
                    })
                })
                .collect::<Result<_, RepoUnpackError>>()?;

            Ok(RepoAsset::Builds(versions))
        }
    }
}

/// Keeps track of the most recent progress updates and calls an external
/// function when there is a change.
pub struct DownloadProgressReporter {
    report_fn: Box<dyn Fn(DownloadProgress) + Send + Sync>,
    bytes_downloaded: AtomicU64,
    bytes_expected: Option<u64>,
    items_unpacked: AtomicU64,
}

impl DownloadProgressReporter {
    pub fn new(
        bytes_expected: Option<u64>,
        report_fn: Box<dyn Fn(DownloadProgress) + Send + Sync>,
    ) -> Self {
        Self {
            report_fn,
            bytes_downloaded: 0.into(),
            bytes_expected,
            items_unpacked: 0.into(),
        }
    }

    fn report_download_progress(&self, bytes: u64) {
        self.bytes_downloaded.store(bytes, Ordering::Relaxed);

        (self.report_fn)(DownloadProgress {
            bytes_downloaded: bytes,
            bytes_expected: self.bytes_expected,
            items_unpacked: self.items_unpacked.load(Ordering::Relaxed),
            is_computing_derived_data: false,
        });
    }

    fn report_unpacked_item(&self) {
        let items = self.items_unpacked.fetch_add(1, Ordering::Relaxed) + 1;

        (self.report_fn)(DownloadProgress {
            bytes_downloaded: self.bytes_downloaded.load(Ordering::Relaxed),
            bytes_expected: self.bytes_expected,
            items_unpacked: items,
            is_computing_derived_data: false,
        });
    }

    // fn report_indexing(&self) {
    //     (self.report_fn)(DownloadProgress {
    //         bytes_downloaded: self.bytes_downloaded.load(Ordering::Relaxed),
    //         bytes_expected: self.bytes_expected,
    //         items_unpacked: self.items_unpacked.load(Ordering::Relaxed),
    //         is_computing_derived_data: true,
    //     });
    // }
}

/// A snapshot of the progress of a repository download.
#[derive(Debug)]
pub struct DownloadProgress {
    /// The number of bytes that have been downloaded.
    pub bytes_downloaded: u64,
    /// The number of bytes that the server has reported it will send.
    pub bytes_expected: Option<u64>,
    /// The number of repository assets that have been unpacked so far.
    pub items_unpacked: u64,
    pub is_computing_derived_data: bool,
}

fn content_type(response: &Response) -> Option<Cow<'static, str>> {
    if let Some(header) = response.headers().get(CONTENT_TYPE)
        && let Ok(header_str) = header.to_str()
    {
        return Some(header_str.to_owned().into());
    }

    // Fallback - server didn't tell us what it sent.

    let url = response.url();
    let path = url.path();

    if path.ends_with(".tar.gz") || path.ends_with(".tgz") {
        return Some(mime::GZIP.into());
    }

    if path.ends_with(".zip") {
        return Some(mime::ZIP.into());
    }

    None
}
