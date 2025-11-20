use std::borrow::Cow;

use diesel::{dsl::Select, prelude::*, sqlite::Sqlite};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;

use crate::{
    Error, database::{JsonbValue, ModuleId, ReleaseId, RepoId, schema::*}, json::{DownloadChecksum, ModuleInstallDescriptor, ModuleKind, ModuleResources, ReleaseStatus}, repo::game::GameVersion
};

pub mod repository;
pub mod module;

pub use repository::{Repository, RepositoryRef};
pub use module::{Module, ModuleRelease, NewModule, NewRelease, ReleaseMetadata};

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = builds)]
#[diesel(check_for_backend(Sqlite))]
pub struct Build {
    pub build_id: i32,
    #[diesel(serialize_as = JsonbValue, deserialize_as = JsonbValue)]
    pub version: GameVersion,
}
