use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    task::{Context, Poll},
};

use async_compression::tokio::bufread::GzipDecoder;
use bytes::Bytes;
use derive_more::From;
use diesel::{
    connection::SimpleConnection,
    debug_query, insert_into,
    prelude::*,
    r2d2::{ConnectionManager, Pool},
    sqlite::Sqlite,
};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use futures_core::{Stream, stream::BoxStream};
use futures_util::{StreamExt, TryStreamExt};
use miette::Diagnostic;
use reqwest::{
    Response,
    header::{ACCEPT, CONTENT_TYPE, ETAG, HeaderValue},
};
use strum::EnumDiscriminants;
use tokio::{
    io::{self, AsyncBufRead, AsyncReadExt},
    spawn,
    task::JoinSet,
};
use tokio_tar::Archive;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{debug, info, instrument, trace};
use url::Url;

use crate::{
    DIRS, DbPool, Error, Result, USER_AGENT,
    database::{
        ModuleId, RepoDB, RepoId,
        models::{Build, NewModule, NewRelease, ReleaseMetadata, Repository, RepositoryRef},
        schema::repositories,
    },
    io::AsyncReadExt as _,
    json::{JsonBuilds, JsonError, JsonModule, RepositoryRefList},
    repo::game::GameVersionParseError,
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
    InvalidEtag {
        url: Arc<Url>,
    }
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
        // sleep if the database is busy, this corresponds to up to 2 seconds sleeping time.
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

    /// Downloads the given repository from an online URL, unpacks it, then inserts it into the repository database.
    #[instrument(skip(self, progress_reporter))]
    pub async fn download(
        &mut self,
        repo: RepositoryRef<'_>,
        progress_reporter: Box<dyn Fn(DownloadProgress) + Send + Sync>,
    ) -> Result<(), Error> {
        info!("Downloading an online CKAN repository");

        let response = self
            .http
            .get(repo.url.clone().into_owned())
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
                url: repo.url.clone().into_owned(),
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

                let unpacker = TarGzUnpacker::new(download_stream);
                self.unpack_repo(repo, unpacker, new_etag, progress.clone())
                    .await?;
            }
            mime::ZIP => todo!("unpacking of .zip repos"),
            _ => {
                return Err(RepoUnpackError::UnsupportedContentType {
                    content_type: content_type.to_string(),
                    url: repo.url.clone().into_owned(),
                }
                .into());
            }
        };

        Ok(())
    }

    async fn unpack_repo(
        &mut self,
        repo: RepositoryRef<'_>,
        unpacker: impl RepoUnpacker<'_>,
        etag: Option<HeaderValue>,
        progress: Arc<DownloadProgressReporter>,
    ) -> Result<(), Error> {
        // Collect all repos into a big list so we can do the DB transaction synchronously.
        let mut asset_stream = unpacker.asset_stream()?;
        let repo_url = Arc::new(repo.url.clone().into_owned());

        let mut tasks = JoinSet::new();
        while let Some(mut asset) = asset_stream.try_next().await? {
            let progress = progress.clone();
            let repo_url = repo_url.clone();

            tasks.spawn(async move {
                match parse_asset(&mut asset) {
                    Ok(asset) => {
                        progress.report_unpacked_item();
                        Ok(asset)
                    }
                    Err(Error::Json(err)) => Err(RepoUnpackError::InvalidJsonFile {
                        source: err,
                        url: repo_url,
                        path: asset.path,
                    })?,
                    Err(err) => Err(err),
                }
            });
        }

        let task_results = tasks.join_all().await;

        let mut db = RepoDB::new(self.database.get()?);

        progress.report_committing();
        db.transaction(|mut db| {
            db.set_etag(repo_url.clone(), etag.as_ref())?;

            let repo_id = db.create_empty_repo(repo)?;

            for asset in task_results {
                match asset? {
                    RepoAsset::Release(parsed) => {
                        let module_id = db.register_module(NewModule {
                            repo_id,
                            module_name: parsed.name.into(),
                        })?;

                        db.create_release(NewRelease {
                            module_id,
                            version: parsed.version,
                            kind: parsed.kind,
                            summary: parsed.r#abstract,
                            metadata: ReleaseMetadata {
                                comment: parsed.comment,
                                download: parsed.download,
                                download_content_type: parsed.download_content_type,
                                download_hash: parsed.download_hash,
                                install: parsed.install,
                                resources: parsed.resources,
                            },
                            description: parsed.description,
                            release_status: parsed.release_status,
                            game_version: if !parsed.ksp_version.is_empty() {
                                parsed.ksp_version.into()
                            } else {
                                parsed.ksp_version_min.into()
                            },
                            game_version_min: parsed.ksp_version_min.into(),
                            game_version_strict: parsed.ksp_version_strict,
                            download_size: parsed.download_size,
                            install_size: parsed.install_size,
                            release_date: parsed.release_date,
                        })?;
                    }
                    RepoAsset::Builds(builds) => {
                        db.register_builds(builds)?;
                    }
                    RepoAsset::DownloadCounts(counts) => {
                        db.add_download_counts(repo_id, &counts)?;
                    }
                    RepoAsset::RepositoryRefList(ref_list) => {
                        for new_ref in ref_list.repositories {
                            db.add_repo_ref(repo_id, new_ref)?;
                        }
                    }
                }
            }

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
                    Ok(Build {
                        build_id,
                        version: version.parse()?,
                    })
                })
                .collect::<Result<_, RepoUnpackError>>()?;

            Ok(RepoAsset::Builds(versions))
        }
    }
}

struct DownloadProgressReporter {
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
            is_committing: false,
        });
    }

    fn report_unpacked_item(&self) {
        let items = self.items_unpacked.fetch_add(1, Ordering::Relaxed) + 1;

        (self.report_fn)(DownloadProgress {
            bytes_downloaded: self.bytes_downloaded.load(Ordering::Relaxed),
            bytes_expected: self.bytes_expected,
            items_unpacked: items,
            is_committing: false,
        });
    }

    fn report_committing(&self) {
        (self.report_fn)(DownloadProgress {
            bytes_downloaded: self.bytes_downloaded.load(Ordering::Relaxed),
            bytes_expected: self.bytes_expected,
            items_unpacked: self.items_unpacked.load(Ordering::Relaxed),
            is_committing: true,
        });
    }
}

#[derive(Debug)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub bytes_expected: Option<u64>,
    pub items_unpacked: u64,
    pub is_committing: bool,
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

#[derive(Debug, From, EnumDiscriminants)]
#[strum_discriminants(name(RepoAssetVariant))]
enum RepoAsset {
    Builds(Vec<Build>),
    Release(Box<JsonModule>),
    DownloadCounts(HashMap<String, i32>),
    RepositoryRefList(RepositoryRefList),
}

impl RepoAssetVariant {
    fn from_path(path: &Path) -> Option<Self> {
        let filename = path.file_name()?;

        Some(match filename.as_encoded_bytes() {
            b"builds.json" => Self::Builds,
            b"repositories.json" => Self::RepositoryRefList,
            b"download_counts.json" => Self::DownloadCounts,
            name if name.ends_with(b".ckan") => Self::Release,
            _ => return None,
        })
    }
}

struct RepoAssetBuf {
    path: PathBuf,
    variant: RepoAssetVariant,
    data: Box<[u8]>,
}

trait RepoUnpacker<'a> {
    /// Returns a stream of items in the repository as they are downloaded.
    fn asset_stream(self) -> Result<BoxStream<'a, Result<RepoAssetBuf>>>;
}

/// Unpacks a gzipped tar archive of a repository.
struct TarGzUnpacker<R: AsyncBufRead + Unpin> {
    archive: Archive<GzipDecoder<R>>,
}

impl<R: AsyncBufRead + Unpin> TarGzUnpacker<R> {
    fn new(stream: R) -> Self {
        Self {
            archive: Archive::new(GzipDecoder::new(stream)),
        }
    }
}

impl<'a, R: AsyncBufRead + Unpin + Send + 'a> RepoUnpacker<'a> for TarGzUnpacker<R> {
    fn asset_stream(mut self) -> Result<BoxStream<'a, Result<RepoAssetBuf>>> {
        let entries = self.archive.entries()?;

        Ok(entries
            .map_err(Error::from)
            .try_filter_map(async |mut item| {
                let path = item.path()?.into_owned();
                let Some(variant) = RepoAssetVariant::from_path(path.as_ref()) else {
                    return Ok(None);
                };

                let mut buf = Vec::new();
                item.read_to_end(&mut buf).await?;

                let asset = RepoAssetBuf {
                    variant,
                    path,
                    data: buf.into_boxed_slice(),
                };

                Ok(Some(asset))
            })
            .boxed())
    }
}
