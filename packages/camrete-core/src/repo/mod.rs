use std::collections::{BTreeSet, HashMap, HashSet};

use game::GameVersion;
use module::Module;
use url::Url;

pub mod client;
pub mod game;
pub mod module;

#[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq, Hash)]
pub struct RepoDescription {
    pub name: String,
    pub url: Url,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub x_mirror: bool,
    #[serde(default)]
    pub x_comment: Option<String>,
}

impl RepoDescription {
    pub fn new(name: String, url: Url) -> Self {
        Self {
            name,
            url,
            priority: 0,
            x_mirror: false,
            x_comment: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Default)]
struct Repository {
    modules: HashMap<String, Module>,
    download_counts: HashMap<String, u32>,
    known_game_versions: BTreeSet<GameVersion>,
    repositories: HashSet<RepoDescription>,
    /// Does any module require a newer client?
    unsupported_spec: bool,
}
