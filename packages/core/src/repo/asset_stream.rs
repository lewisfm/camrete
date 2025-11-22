//! Stream repository assets from archive files.
//!
//! This module contains a set of [`RepoAssetLoader`]s that can parse various archive formats
//! (such as `tar.gz`) and stream the [unparsed assets](RepoAssetBuf) they contain as more of the archive
//! is received. The loaders don't need the entire archive's contents to start unpacking, making
//! them ideal for using with internet downloads.
//!
//! Assets can then be parsed into [`RepoAsset`]s for usage or inclusion in a database.

use std::{
    collections::HashMap,
    io::Cursor,
    path::{Path, PathBuf},
};

use async_compression::tokio::bufread::GzipDecoder;
use derive_more::From;
use futures_core::stream::BoxStream;
use futures_util::{StreamExt, TryStreamExt};
use strum::EnumDiscriminants;
use tokio::io::{AsyncBufRead, AsyncReadExt};
use tokio_tar::Archive;

use crate::{
    Error, Result,
    database::models::BuildRecord,
    json::{JsonModule, RepositoryRefList},
};

/// A parsed asset contained in a repository archive.
///
/// Assets contain a subset of the data of a repository,
/// such as the download counts for its modules or details
/// about a module's release.
#[derive(Debug, From, EnumDiscriminants)]
#[strum_discriminants(name(RepoAssetVariant))]
pub enum RepoAsset {
    /// A list of know game versions, compared to their build IDs.
    Builds(Vec<BuildRecord>),
    /// A complete description of a module's release.
    Release(Box<JsonModule>),
    /// A map from module IDs to that module's download count.
    DownloadCounts(HashMap<String, i32>),
    /// A list of other repositories which this repo suggests using.
    RepositoryRefList(RepositoryRefList),
}

impl RepoAssetVariant {
    /// Determine which asset the given file contains based on its path.
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

/// Unpack archive formats and stream the repository assets they contain.
pub trait RepoAssetLoader<'a> {
    /// Returns a stream of items in the repository as they are received from the archive data.
    fn asset_stream(self) -> Result<BoxStream<'a, Result<RepoAssetBuf>>>;
}

/// Unpacks a streamed gzipped tar archive of a repository.
pub struct TarGzAssetLoader<R: AsyncBufRead + Unpin> {
    archive: Archive<GzipDecoder<R>>,
}

impl<R: AsyncBufRead + Unpin> TarGzAssetLoader<R> {
    /// Create the loader using a byte stream such as a download.
    ///
    /// Data is not read from the stream until assets are requested by creating an
    /// [asset stream](RepoAssetLoader::asset_stream) and polling it.
    ///
    /// For files already completely in memory, use [`Self::from_buf`].
    pub fn new(stream: R) -> Self {
        Self {
            archive: Archive::new(GzipDecoder::new(stream)),
        }
    }
}

impl<T: AsRef<[u8]> + Unpin> TarGzAssetLoader<Cursor<T>> {
    /// Creates a tar asset loader from an archive already completely in memory,
    /// such as a [`Vec<u8>`] or byte slice.
    pub fn from_buf(buf: T) -> Self {
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

/// An asset loader which holds all future assets in-memory and performs no
/// I/O.
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

#[cfg(test)]
pub(crate) mod test {
    use super::*;

    #[test]
    fn asset_from_path() {
        let p1 = PathBuf::from("./repo/Parallax/Parallax-0.1.1.ckan");
        let p2 = PathBuf::from("CKAN-meta-master/repositories.json");
        let p3 = PathBuf::from("builds.json");
        let p4 = PathBuf::from("/absolute/path/to/download_counts.json");

        assert_eq!(
            RepoAssetVariant::from_path(&p1).unwrap(),
            RepoAssetVariant::Release
        );
        assert_eq!(
            RepoAssetVariant::from_path(&p2).unwrap(),
            RepoAssetVariant::RepositoryRefList
        );
        assert_eq!(
            RepoAssetVariant::from_path(&p3).unwrap(),
            RepoAssetVariant::Builds
        );
        assert_eq!(
            RepoAssetVariant::from_path(&p4).unwrap(),
            RepoAssetVariant::DownloadCounts
        );
    }

    #[test]
    fn frozen_is_not_an_asset() {
        let p1 = PathBuf::from("OKM-3.frozen");
        assert!(RepoAssetVariant::from_path(&p1).is_none());
    }

    #[test]
    fn extensionless_is_not_an_asset() {
        let p1 = PathBuf::from(".DS_Store");
        assert!(RepoAssetVariant::from_path(&p1).is_none());
    }

    #[test]
    fn folders_are_not_assets() {
        let p1 = PathBuf::from("/repo/4kSPExpanded");
        assert!(RepoAssetVariant::from_path(&p1).is_none());
    }

    pub async fn load_test_repo() -> Vec<RepoAssetBuf> {
        let repo_buf = include_bytes!("../../benches/mini_repo.tgz");

        let loader = TarGzAssetLoader::from_buf(repo_buf);
        let stream = loader.asset_stream().unwrap();

        stream.try_collect::<Vec<_>>().await.unwrap()
    }

    #[tokio::test]
    async fn load_tgz_in_memory() {
        let assets = load_test_repo().await;

        assert!(
            assets
                .iter()
                .any(|a| a.variant == RepoAssetVariant::Release)
        );
    }
}
