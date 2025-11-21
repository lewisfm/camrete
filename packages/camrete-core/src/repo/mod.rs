pub mod asset_stream;
pub mod client;
pub mod game;

pub use asset_stream::{
    RepoAsset, RepoAssetBuf, RepoAssetLoader, RepoAssetVariant, TarGzAssetLoader,
};
pub use client::{DownloadProgress, RepoManager, RepoUnpackError};

// #[derive(Debug, PartialEq, Eq, Default)]
// struct Repository {
//     modules: HashMap<String, Module>,
//     download_counts: HashMap<String, u32>,
//     known_game_versions: BTreeSet<GameVersion>,
//     repositories: HashSet<RepoDescription>,
//     /// Does any module require a newer client?
//     unsupported_spec: bool,
// }
