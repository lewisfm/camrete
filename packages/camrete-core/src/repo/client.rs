use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll},
};

use async_compression::tokio::bufread::GzipDecoder;
use bytes::Bytes;
use derive_more::From;
use diesel::{
    connection::SimpleConnection,
    insert_into,
    prelude::*,
    r2d2::{ConnectionManager, Pool},
};
use futures_core::Stream;
use futures_util::{StreamExt, TryStreamExt};
use miette::Diagnostic;
use reqwest::{
    Response,
    header::{ACCEPT, CONTENT_TYPE},
};
use strum::EnumDiscriminants;
use tokio::{
    io::{self, AsyncBufRead, AsyncReadExt},
    spawn,
};
use tokio_tar::Archive;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{debug, info, instrument, trace};
use url::Url;

use crate::{
    DIRS, DbPool, Error, Result, USER_AGENT,
    io::AsyncReadExt as _,
    json::{JsonBuilds, JsonError, JsonModule, RepositoryRefList},
    models::{Build, NewModule, NewModuleRelease, ReleaseMetadata, RepositoryRef},
    repo::game::GameVersionParseError,
    schema::repositories,
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
    InvalidJsonFile {
        #[diagnostic(transparent)]
        source: JsonError,
        url: Box<Url>,
        path: PathBuf,
    },
}

const MAX_DB_CONNS: u32 = 16;

#[derive(Debug, Clone)]
pub struct RepoClient {
    database: DbPool,
    http: reqwest::Client,
}

impl RepoClient {
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

        Ok(Self {
            database: pool,
            http: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .expect("http client initialized"),
        })
    }

    #[instrument(skip(self, progress_reporter))]
    pub async fn download(
        &mut self,
        name: &str,
        url: Url,
        progress_reporter: impl Fn(UnpackProgress) + Send + Sync + 'static,
    ) -> Result<(), Error> {
        info!("Downloading an online CKAN repository");

        let response = self
            .http
            .get(url.clone())
            .header(
                ACCEPT,
                "application/gzip,application/x-gzip,application/zip",
            )
            .send()
            .await?;

        let download_size = response.content_length();

        let content_type = content_type(&response)
            .ok_or_else(|| RepoUnpackError::MissingContentType { url: url.clone() })?;

        trace!(%content_type);

        let progress = Arc::new(ProgressReporter {
            report_fn: progress_reporter,
            bytes_downloaded: AtomicU64::new(0),
            bytes_expected: download_size,
            items_unpacked: AtomicU64::new(0),
        });

        let download_stream = response
            .bytes_stream()
            .map_err(io::Error::other)
            .into_async_read()
            .compat()
            .progress(|bytes| {
                progress.report_download_progress(bytes);
            });

        let mut unpacker = match content_type.as_ref() {
            mime::GZIP | mime::X_GZIP => {
                debug!("Using tar.gz unpacker");
                TarGzUnpacker::new(download_stream).asset_stream()?.boxed()
            }
            mime::ZIP => todo!("unpacking of .zip repos"),
            _ => {
                return Err(RepoUnpackError::UnsupportedContentType {
                    content_type: content_type.to_string(),
                    url: url.clone(),
                }
                .into());
            }
        };

        let url = Arc::new(url);
        // let (item_tx, item_rx) = mpsc::channel(32);
        let mut db = self.database.get()?;

        // Add our repo to database first so we can use its ID when inserting other things.
        let new_repo = RepositoryRef::new(name, &url);

        let repo_id: i32 = insert_into(repositories::table)
            .values(new_repo)
            .on_conflict_do_nothing()
            .returning(repositories::repo_id)
            .get_result(&mut db)?;

        while let Some(mut asset) = unpacker.try_next().await? {
            let progress = progress.clone();
            let repo_url = url.clone();
            // let slot = item_tx.reserve().await.unwrap();
            // let mut mgr = self.clone();

            spawn(async move {
                match apply_resource(&mut asset, repo_id) {
                    Ok(asset) => {
                        progress.report_unpacked_item();
                        println!("{asset:?}");
                        Ok(())
                    }
                    Err(Error::Json(err)) => Err(RepoUnpackError::InvalidJsonFile {
                        source: err,
                        url: (*repo_url).clone().into(),
                        path: asset.path,
                    })?,
                    Err(err) => Err(err),
                }
            });
        }

        Ok(())
    }
}

fn apply_resource(asset: &mut RepoAssetBuf, repo_id: i32) -> Result<RepoAsset> {
    match asset.variant {
        RepoAssetVariant::Release => {
            let parsed: JsonModule = simd_json::from_slice(&asset.data)?;
            parsed.verify()?;

            let module = NewModule {
                repo_id,
                module_name: parsed.name.into(),
            };

            let release = NewModuleRelease {
                module_id: 0, // Unknown until inserted
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
            };

            Ok(RepoAsset::Release { module, release: release.into() })
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

struct ProgressReporter<F> {
    report_fn: F,
    bytes_downloaded: AtomicU64,
    bytes_expected: Option<u64>,
    items_unpacked: AtomicU64,
}

impl<F: Fn(UnpackProgress)> ProgressReporter<F> {
    fn report_download_progress(&self, bytes: u64) {
        self.bytes_downloaded.store(bytes, Ordering::Relaxed);

        (self.report_fn)(UnpackProgress {
            bytes_downloaded: bytes,
            bytes_expected: self.bytes_expected,
            items_unpacked: self.items_unpacked.load(Ordering::Relaxed),
        });
    }

    fn report_unpacked_item(&self) {
        let items = self.items_unpacked.fetch_add(1, Ordering::Relaxed) + 1;

        (self.report_fn)(UnpackProgress {
            bytes_downloaded: self.bytes_downloaded.load(Ordering::Relaxed),
            bytes_expected: self.bytes_expected,
            items_unpacked: items,
        });
    }
}

#[derive(Debug)]
pub struct UnpackProgress {
    pub bytes_downloaded: u64,
    pub bytes_expected: Option<u64>,
    pub items_unpacked: u64,
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
    Release {
        module: NewModule<'static>,
        release: Box<NewModuleRelease>,
    },
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

trait RepoUnpacker {
    /// Returns a stream of items in the repository as they are downloaded.
    fn asset_stream(self) -> Result<impl Stream<Item = Result<RepoAssetBuf>>>;
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

impl<R: AsyncBufRead + Unpin> RepoUnpacker for TarGzUnpacker<R> {
    fn asset_stream(mut self) -> Result<impl Stream<Item = Result<RepoAssetBuf>>> {
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
            }))
    }
}
