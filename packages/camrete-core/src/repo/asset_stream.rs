use std::{
    collections::HashMap, io::Cursor, path::{Path, PathBuf}, sync::Arc
};

use async_compression::tokio::bufread::GzipDecoder;
use derive_more::From;
use futures_core::stream::BoxStream;
use futures_util::{StreamExt, TryStreamExt, stream::try_unfold};
use strum::EnumDiscriminants;
use tokio::{
    fs::{ReadDir, read, read_dir},
    io::{AsyncBufRead, AsyncReadExt},
};
use tokio_tar::Archive;

use crate::{
    Error, Result,
    database::models::BuildRecord,
    json::{JsonModule, RepositoryRefList},
};

#[derive(Debug, From, EnumDiscriminants)]
#[strum_discriminants(name(RepoAssetVariant))]
pub enum RepoAsset {
    Builds(Vec<BuildRecord>),
    Release(Box<JsonModule>),
    DownloadCounts(HashMap<String, i32>),
    RepositoryRefList(RepositoryRefList),
}

impl RepoAssetVariant {
    pub fn from_path(path: &Path) -> Option<Self> {
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

/// A byte buffer containing the serialized data for an asset.
#[derive(Debug, Clone)]
pub struct RepoAssetBuf {
    /// The path which this asset was sourced from in the repo.
    pub path: PathBuf,
    /// The asset which this buffer holds.
    pub variant: RepoAssetVariant,
    /// The serialized asset data.
    pub data: Box<[u8]>,
}

pub trait RepoAssetLoader<'a> {
    /// Returns a stream of items in the repository as they are downloaded.
    fn asset_stream(self) -> Result<BoxStream<'a, Result<RepoAssetBuf>>>;
}

/// Unpacks a streamed gzipped tar archive of a repository.
pub struct TarGzAssetLoader<R: AsyncBufRead + Unpin> {
    archive: Archive<GzipDecoder<R>>,
}

impl<R: AsyncBufRead + Unpin> TarGzAssetLoader<R> {
    pub fn new(stream: R) -> Self {
        Self {
            archive: Archive::new(GzipDecoder::new(stream)),
        }
    }

}

impl TarGzAssetLoader<Cursor<Vec<u8>>> {
    pub fn from_buf(buf: Vec<u8>) -> Self {
        let cursor = Cursor::new(buf);
        Self {
            archive: Archive::new(GzipDecoder::new(cursor)),
        }
    }
}

impl<'a, R: AsyncBufRead + Unpin + Send + 'a> RepoAssetLoader<'a> for TarGzAssetLoader<R> {
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

/// Loaders for benchmarking.
#[cfg(feature = "bench")]
pub mod bench {
    use async_walkdir::WalkDir;

    use super::*;

    /// Reads a directory containing assets from the file system.
    pub struct AssetDirLoader {
        path: PathBuf,
        reader: WalkDir,
    }

    impl AssetDirLoader {
        pub fn new(path: PathBuf) -> Self {
            Self {
                reader: WalkDir::new(&path),
                path,
            }
        }
    }

    impl<'a> RepoAssetLoader<'a> for AssetDirLoader {
        fn asset_stream(self) -> Result<BoxStream<'a, Result<RepoAssetBuf>>> {
            let read_stream = self.reader;
            let base_path = Arc::new(self.path);

            let assets = read_stream
                .map_err(|err| Error::from(err.into_io().unwrap()))
                .try_filter_map(move |item| {
                    let base_path = base_path.clone();
                    async move {
                        let path = item.path();
                        let Some(variant) = RepoAssetVariant::from_path(&path) else {
                            return Ok(None);
                        };

                        let asset = RepoAssetBuf {
                            variant,
                            data: read(&path).await?.into(),
                            path: path.strip_prefix(&*base_path).unwrap().to_owned(),
                        };

                        Ok(Some(asset))
                    }
                })
                .boxed();

            Ok(assets)
        }
    }

    /// An asset loader which holds all future assets in-memory and performs no I/O.
    #[derive(Debug, Clone)]
    pub struct InMemoryAssetLoader {
        pub assets: Vec<RepoAssetBuf>,
    }

    impl From<Vec<RepoAssetBuf>> for InMemoryAssetLoader {
        fn from(assets: Vec<RepoAssetBuf>) -> Self {
            Self { assets }
        }
    }

    impl InMemoryAssetLoader {
        pub async fn from_loader<'a>(other: impl RepoAssetLoader<'a>) -> Result<Self> {
            let assets = other.asset_stream()?.try_collect().await?;
            Ok(Self { assets })
        }
    }

    impl<'a> RepoAssetLoader<'a> for InMemoryAssetLoader {
        fn asset_stream(self) -> Result<BoxStream<'a, Result<RepoAssetBuf>>> {
            let stream = self.assets.into_iter().map(Ok);
            Ok(futures_util::stream::iter(stream).boxed())
        }
    }
}
