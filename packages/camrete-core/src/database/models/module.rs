use std::borrow::Cow;

use derive_more::Into;
use diesel::{
    backend::Backend, dsl::{AsSelect, Eq, Select}, expression::AsExpression, prelude::*, serialize::{IsNull, Output, ToSql}, sql_types::Integer, sqlite::Sqlite
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;

use crate::{
    database::{DepGroupId, JsonbValue, ModuleId, ReleaseId, RepoId, models::module::version::ModuleVersion, schema::{self, *}},
    json::{DownloadChecksum, ModuleInstallDescriptor, ModuleKind, ModuleResources, ReleaseStatus},
    repo::game::GameVersion,
};

mod version;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = modules)]
#[diesel(check_for_backend(Sqlite))]
pub struct Module {
    pub module_id: ModuleId,
    pub repo_id: i32,
    pub module_name: String,
    pub download_count: i32,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = modules)]
#[diesel(check_for_backend(Sqlite))]
pub struct NewModule<'a> {
    pub repo_id: RepoId,
    pub module_name: &'a str,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = module_releases)]
#[diesel(check_for_backend(Sqlite))]
pub struct NewRelease<'a> {
    pub module_id: ModuleId,
    pub version: &'a str,
    #[diesel(serialize_as = i32)]
    pub kind: ModuleKind,
    pub summary: &'a str,
    #[diesel(serialize_as = JsonbValue)]
    pub metadata: ReleaseMetadata<'a>,
    pub description: Option<&'a str>,
    #[diesel(serialize_as = i32)]
    pub release_status: ReleaseStatus,
    #[diesel(serialize_as = JsonbValue)]
    pub game_version: GameVersion,
    #[diesel(serialize_as = JsonbValue)]
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
    pub release_id: ReleaseId,
    pub module_id: ModuleId,
    pub version: String,
    pub sort_index: i32,
    pub summary: String,
    #[diesel(deserialize_as = JsonbValue)]
    pub metadata: ReleaseMetadata<'static>,
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

type AllSortable = Select<module_releases::table, AsSelect<SortableRelease, Sqlite>>;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = module_releases)]
#[diesel(check_for_backend(Sqlite))]
pub struct SortableRelease {
    pub release_id: ReleaseId,
    pub version: ModuleVersion<'static>,
}

impl SortableRelease {
    pub fn all() -> AllSortable {
        module_releases::table.select(Self::as_select())
    }

    pub fn with_parent(mod_id: ModuleId) -> Eq<module_releases::module_id, ModuleId> {
        module_releases::module_id.eq(mod_id)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseMetadata<'a> {
    pub comment: Option<Cow<'a, str>>,
    pub download: Cow<'a, [Url]>,
    pub download_hash: Cow<'a, DownloadChecksum>,
    pub download_content_type: Option<Cow<'a, str>>,
    pub resources: Cow<'a, ModuleResources>,
    pub install: Cow<'a, [ModuleInstallDescriptor]>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = module_authors)]
#[diesel(check_for_backend(Sqlite))]
pub struct NewModuleAuthor<'a> {
    pub release_id: ReleaseId,
    pub ordinal: i32,
    pub author: &'a str,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = module_licenses)]
#[diesel(check_for_backend(Sqlite))]
pub struct NewModuleLicense<'a> {
    pub release_id: ReleaseId,
    pub license: &'a str,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = module_tags)]
#[diesel(check_for_backend(Sqlite))]
pub struct NewModuleTag<'a> {
    pub release_id: ReleaseId,
    pub ordinal: i32,
    pub tag: &'a str,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = module_localizations)]
#[diesel(check_for_backend(Sqlite))]
pub struct NewModuleLocale<'a> {
    pub release_id: ReleaseId,
    pub locale: &'a str,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = module_relationship_groups)]
#[diesel(check_for_backend(Sqlite))]
pub struct NewModuleRelationshipGroup<'a> {
    pub release_id: ReleaseId,
    pub ordinal: i32,
    pub rel_type: RelationshipType,
    pub choice_help_text: Option<&'a str>,
    pub suppress_recommendations: bool,
}

#[derive(Debug, AsExpression, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[diesel(sql_type = Integer)]
#[repr(i32)]
pub enum RelationshipType {
    Depends,
    Recommends,
    Suggests,
    Supports,
    Conflicts,
    Provides,
}

impl From<RelationshipType> for i32 {
    fn from(value: RelationshipType) -> Self {
        value as i32
    }
}

impl ToSql<Integer, Sqlite> for RelationshipType {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> diesel::serialize::Result {
        out.set_value(*self as i32);
        Ok(IsNull::No)
    }
}

#[derive(Debug, Insertable)]
#[diesel(table_name = module_relationships)]
#[diesel(check_for_backend(Sqlite))]
pub struct NewModuleRelationship<'a> {
    pub group_id: DepGroupId,
    pub ordinal: i32,
    pub target_name: &'a str,
    pub target_version: Option<&'a str>,
    pub target_version_min: Option<&'a str>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = module_replacements)]
#[diesel(check_for_backend(Sqlite))]
pub struct NewModuleReplacement<'a> {
    pub release_id: ReleaseId,
    pub target_name: &'a str,
    pub target_version: Option<&'a str>,
    pub target_version_min: Option<&'a str>,
}
