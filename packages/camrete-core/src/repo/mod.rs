pub mod client;
pub mod game;
pub mod module;
pub mod asset_stream;

pub use client::{RepoManager, RepoUnpackError, DownloadProgress};
pub use asset_stream::{RepoAsset, RepoAssetVariant, TarGzAssetLoader, RepoAssetLoader, RepoAssetBuf};

// #[derive(Debug, PartialEq, Eq, Default)]
// struct Repository {
//     modules: HashMap<String, Module>,
//     download_counts: HashMap<String, u32>,
//     known_game_versions: BTreeSet<GameVersion>,
//     repositories: HashSet<RepoDescription>,
//     /// Does any module require a newer client?
//     unsupported_spec: bool,
// }
