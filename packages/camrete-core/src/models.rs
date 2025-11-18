use std::borrow::Cow;

use diesel::{
    prelude::*,
    sqlite::Sqlite,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;

use self::helpers::*;
use crate::{
    json::{DownloadChecksum, ModuleInstallDescriptor, ModuleKind, ModuleResources, ReleaseStatus},
    repo::game::GameVersion,
    schema::*,
};

mod helpers;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = repositories)]
#[diesel(check_for_backend(Sqlite))]
pub struct Repository {
    pub repo_id: i32,
    #[diesel(deserialize_as = JsonbValue)]
    pub url: Url,
    pub name: String,
    pub priority: i32,
    pub x_mirror: bool,
    pub x_comment: Option<String>,
}

#[derive(Debug, Insertable, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[diesel(table_name = repositories)]
#[diesel(check_for_backend(Sqlite))]
pub struct RepositoryRef<'a> {
    pub name: Cow<'a, str>,
    #[diesel(serialize_as = JsonbValue)]
    pub url: Cow<'a, Url>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub x_mirror: bool,
    #[serde(default)]
    pub x_comment: Option<Cow<'a, str>>,
}

impl<'a> RepositoryRef<'a> {
    pub fn new(name: &'a str, url: &'a Url) -> Self {
        Self {
            name: Cow::Borrowed(name),
            url: Cow::Borrowed(url),
            priority: 0,
            x_mirror: false,
            x_comment: None,
        }
    }
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = modules)]
#[diesel(check_for_backend(Sqlite))]
pub struct Module {
    pub module_id: i32,
    pub repo_id: i32,
    pub module_name: String,
    pub download_count: i32,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = modules)]
#[diesel(check_for_backend(Sqlite))]
pub struct NewModule<'a> {
    pub repo_id: i32,
    pub module_name: Cow<'a, str>,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = module_releases)]
#[diesel(check_for_backend(Sqlite))]
pub struct NewModuleRelease {
    pub module_id: i32,
    pub version: String,
    #[diesel(deserialize_as = i32)]
    pub kind: ModuleKind,
    pub summary: String,
    #[diesel(deserialize_as = JsonbValue)]
    pub metadata: ReleaseMetadata,
    pub description: Option<String>,
    #[diesel(deserialize_as = i32)]
    pub release_status: ReleaseStatus,
    #[diesel(deserialize_as = JsonbValue)]
    pub game_version: GameVersion,
    #[diesel(deserialize_as = JsonbValue)]
    pub game_version_min: GameVersion,
    pub game_version_strict: bool,
    pub download_size: Option<i64>,
    pub install_size: Option<i64>,
    pub release_date: Option<OffsetDateTime>,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = module_releases)]
#[diesel(check_for_backend(Sqlite))]
pub struct ModuleRelease {
    pub release_id: i32,
    pub module_id: i32,
    pub version: String,
    pub sort_index: i32,
    pub summary: String,
    #[diesel(deserialize_as = JsonbValue)]
    pub metadata: ReleaseMetadata,
    pub description: Option<String>,
    #[diesel(deserialize_as = i32)]
    pub release_status: ReleaseStatus,
    #[diesel(deserialize_as = JsonbValue)]
    pub game_version: GameVersion,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseMetadata {
    pub comment: Option<String>,
    pub download: Vec<Url>,
    pub download_hash: DownloadChecksum,
    pub download_content_type: Option<String>,
    pub resources: ModuleResources,
    pub install: Vec<ModuleInstallDescriptor>,
}

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = builds)]
#[diesel(check_for_backend(Sqlite))]
pub struct Build {
    pub build_id: i32,
    #[diesel(serialize_as = JsonbValue, deserialize_as = JsonbValue)]
    pub version: GameVersion,
}
