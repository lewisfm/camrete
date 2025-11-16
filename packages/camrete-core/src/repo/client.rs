use std::{
    borrow::Cow,
    collections::HashMap,
    io::ErrorKind,
    path::{Path, PathBuf},
    pin::Pin,
    str::FromStr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    task::{Context, Poll},
};

use async_compression::tokio::bufread::GzipDecoder;
use bytes::Bytes;
use diesel::prelude::*;
use futures_core::{Stream, future};
use futures_util::{StreamExt, TryStreamExt};
use miette::Diagnostic;
use reqwest::{
    Response,
    header::{ACCEPT, CONTENT_TYPE, HeaderValue},
};
use tokio::{
    io::{self, AsyncBufRead, AsyncReadExt},
    spawn,
    task::{JoinSet, spawn_blocking},
};
use tokio_tar::Archive;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{debug, info, instrument, trace};
use url::Url;

use crate::{
    DIRS, DatabaseConnection, Error, Result, USER_AGENT,
    io::AsyncReadExt as _,
    json::{JsonBuilds, JsonModule},
    repo::{
        RepoDescription, Repository,
        game::{GameVersion, GameVersionParseError},
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
        source: simd_json::Error,
        url: Box<Url>,
        path: PathBuf,
    },
}

pub struct RepoClient {
    database: DatabaseConnection,
    http: reqwest::Client,
}

impl RepoClient {
    pub async fn new(url: &str) -> Result<Self> {
        Ok(Self::from_db_client(DatabaseConnection::establish(url)?))
    }

    pub async fn from_data_dir() -> Result<Self> {
        let repos_file = DIRS.data_local_dir().join("repos.sqlite");
        let url = Url::from_file_path(repos_file).expect("path is valid");
        // url.set_query(Some("mode=rwc")); // create database if not exists

        let database = DatabaseConnection::establish(url.as_str())?;

        Ok(Self::from_db_client(database))
    }

    fn from_db_client(database: DatabaseConnection) -> Self {
        Self {
            database,
            http: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .expect("http client initialized"),
        }
    }

    #[instrument(skip(self, progress_reporter))]
    pub async fn download(
        &mut self,
        repo_url: Url,
        progress_reporter: impl Fn(UnpackProgress) + Send + Sync + 'static,
    ) -> Result<(), Error> {
        info!("Downloading an online CKAN repository");

        let response = self
            .http
            .get(repo_url.clone())
            .header(
                ACCEPT,
                "application/gzip,application/x-gzip,application/zip",
            )
            .send()
            .await?;

        let download_size = response.content_length();

        let content_type =
            content_type(&response).ok_or_else(|| RepoUnpackError::MissingContentType {
                url: repo_url.clone(),
            })?;

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
                    url: repo_url.clone(),
                }
                .into());
            }
        };

        let repo = Arc::new(Mutex::new(Repository::default()));
        let repo_url = Arc::new(repo_url);

        unpacker
            .try_for_each_concurrent(None, |mut asset| {
                let repo = repo.clone();
                let progress = progress.clone();
                let repo_url = repo_url.clone();

                async move {
                    match apply_resource(&repo, &mut asset) {
                        Ok(_) => {
                            progress.report_unpacked_item();
                            Ok(())
                        }
                        Err(Error::Json(err)) => Err(RepoUnpackError::InvalidJsonFile {
                            source: err,
                            url: (*repo_url).clone().into(),
                            path: asset.path,
                        })?,
                        Err(err) => Err(err),
                    }
                }
            })
            .await?;

        let repo = Arc::into_inner(repo).unwrap().into_inner().unwrap();

        println!("mod[0] = {:?}", repo.modules.iter().next());
        println!("dc[0] = {:?}", repo.download_counts.iter().next());
        println!("versions = {:?}", repo.known_game_versions);
        println!("repo refs = {:?}", repo.repositories);
        println!("unsupported_spec = {:?}", repo.unsupported_spec);

        // As each file in the archive is received, begin decoding it in parallel.

        // let (tx, rx) = mpsc::channel();

        // let db_task = spawn_blocking(|| {
        //     self.database.transaction(|txn| Ok::<_, Error>(()))?;

        //     Ok::<_, Error>(())
        // });

        // while let Some(item) = unpacker.try_next().await? {
        //     let tx = tx.clone();

        //     spawn(async move {
        //         match item.variant {
        //             RepoItemVariant::Module => {}
        //             _ => todo!()
        //         }
        //     });
        // }

        // db_task.await.unwrap()?;

        Ok(())
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

fn apply_resource(repo: &Mutex<Repository>, mut asset: &mut RepoAsset) -> Result<()> {
    match asset.variant {
        RepoAssetVariant::Builds => {
            let parsed: JsonBuilds = simd_json::from_slice(&mut asset.data)?;
            let versions = parsed
                .builds
                .values()
                .map(|s| Ok(s.parse()?))
                .collect::<Result<Vec<GameVersion>, RepoUnpackError>>()?;

            let mut repo = repo.lock().unwrap();
            repo.known_game_versions.extend(versions);
        }
        RepoAssetVariant::DownloadCounts => {}
        RepoAssetVariant::Module => {
            let parsed: JsonModule = simd_json::from_slice(&mut asset.data)?;
        }
        RepoAssetVariant::RepositoryRefs => {}
    }

    Ok(())
}

trait RepoUnpacker {
    /// Returns a stream of items in the repository as they are downloaded.
    fn asset_stream(self) -> Result<impl Stream<Item = Result<RepoAsset>>>;
}

enum RepoAssetVariant {
    Module,
    Builds,
    DownloadCounts,
    RepositoryRefs,
}

impl RepoAssetVariant {
    fn from_path(path: &Path) -> Option<Self> {
        let filename = path.file_name()?;

        Some(match filename.as_encoded_bytes() {
            b"builds.json" => Self::Builds,
            b"repositories.json" => Self::RepositoryRefs,
            b"download_counts.json" => Self::DownloadCounts,
            name if name.ends_with(b".ckan") => Self::Module,
            _ => return None,
        })
    }
}

struct RepoAsset {
    path: PathBuf,
    variant: RepoAssetVariant,
    data: Box<[u8]>,
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
    fn asset_stream(mut self) -> Result<impl Stream<Item = Result<RepoAsset>>> {
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

                let asset = RepoAsset {
                    variant,
                    path,
                    data: buf.into_boxed_slice(),
                };

                Ok(Some(asset))
            }))
    }
}

struct ZipUnpacker {}

impl ZipUnpacker {
    fn unpack(stream: impl Stream<Item = Result<Bytes>>) -> Self {
        todo!()
    }
}

impl Stream for ZipUnpacker {
    type Item = Result<RepoAsset>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        todo!()
    }
}
