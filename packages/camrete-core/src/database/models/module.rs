use std::borrow::Cow;

use derive_more::TryFrom;
use diesel::{
    backend::Backend,
    deserialize::FromSql,
    dsl::{self, AsSelect, Select},
    expression::AsExpression,
    prelude::*,
    serialize::{IsNull, Output, ToSql},
    sql_types::Integer,
    sqlite::Sqlite,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use url::Url;

use crate::{
    database::{DepGroupId, DepId, JsonbValue, ModuleId, ReleaseId, RepoId, schema::*},
    json::{DownloadChecksum, ModuleInstallDescriptor, ModuleKind, ModuleResources, ReleaseStatus},
    repo::game::GameVersion,
};

mod version;

pub use version::ModuleVersion;

pub type AllModules = Select<modules::table, AsSelect<Module, Sqlite>>;
pub type AllReleases = Select<module_releases::table, AsSelect<ModuleRelease, Sqlite>>;
type AllDepGroups =
    Select<module_relationship_groups::table, AsSelect<ModuleRelationshipGroup, Sqlite>>;
type AllDeps = Select<module_relationships::table, AsSelect<ModuleRelationship, Sqlite>>;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = modules)]
#[diesel(check_for_backend(Sqlite))]
pub struct Module {
    #[diesel(column_name = module_id)]
    pub id: ModuleId,
    pub repo_id: RepoId,
    #[diesel(column_name = module_slug)]
    pub slug: String,
    pub download_count: i32,
}

impl Module {
    pub fn all() -> AllModules {
        modules::table.select(Self::as_select())
    }

    #[dsl::auto_type(no_type_alias)]
    pub fn in_repo(repo: RepoId) -> _ {
        modules::repo_id.eq(repo)
    }

    #[dsl::auto_type(no_type_alias)]
    pub fn with_slug(slug: &'_ str) -> _ {
        modules::module_slug.eq(slug)
    }
}

#[derive(Debug, Insertable)]
#[diesel(table_name = modules)]
#[diesel(check_for_backend(Sqlite))]
pub struct NewModule<'a> {
    pub repo_id: RepoId,
    #[diesel(column_name = module_slug)]
    pub slug: &'a str,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = module_releases)]
#[diesel(check_for_backend(Sqlite))]
pub struct NewRelease<'a> {
    pub module_id: ModuleId,
    pub version: &'a str,
    pub display_name: &'a str,
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
    #[diesel(column_name = release_id)]
    pub id: ReleaseId,
    pub module_id: ModuleId,
    pub version: String,
    pub display_name: String,
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

impl ModuleRelease {
    pub fn all() -> AllReleases {
        module_releases::table.select(ModuleRelease::as_select())
    }

    #[dsl::auto_type(no_type_alias)]
    pub fn by_version() -> _ {
        module_releases::version
    }

    #[dsl::auto_type(no_type_alias)]
    pub fn with_parent(module_id: ModuleId) -> _ {
        module_releases::module_id.eq(module_id)
    }

    #[dsl::auto_type(no_type_alias)]
    pub fn tags_for(release: ReleaseId) -> _ {
        module_tags::table
            .select(module_tags::tag)
            .filter(module_tags::release_id.eq(release))
            .order(module_tags::ordinal)
    }

    #[dsl::auto_type(no_type_alias)]
    pub fn authors_for(release: ReleaseId) -> _ {
        module_authors::table
            .select(module_authors::author)
            .filter(module_authors::release_id.eq(release))
            .order(module_authors::ordinal)
    }

    #[dsl::auto_type(no_type_alias)]
    pub fn licenses_for(release: ReleaseId) -> _ {
        module_licenses::table
            .select(module_licenses::license)
            .filter(module_licenses::release_id.eq(release))
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

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = module_relationship_groups)]
#[diesel(check_for_backend(Sqlite))]
pub struct ModuleRelationshipGroup {
    #[diesel(column_name = group_id)]
    pub id: DepGroupId,
    pub release_id: ReleaseId,
    pub ordinal: i32,
    pub rel_type: RelationshipType,
}

impl ModuleRelationshipGroup {
    #[dsl::auto_type(no_type_alias)]
    pub fn all() -> _ {
        let select: AllDepGroups = module_relationship_groups::table.select(Self::as_select());
        select
            .order(module_relationship_groups::rel_type)
            .then_order_by(module_relationship_groups::ordinal)
    }

    #[dsl::auto_type(no_type_alias)]
    pub fn for_release(release_id: ReleaseId) -> _ {
        module_relationship_groups::release_id.eq(release_id)
    }
}

#[derive(Debug, AsExpression, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFrom)]
#[diesel(sql_type = Integer)]
#[try_from(repr)]
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

impl<DB> Queryable<Integer, DB> for RelationshipType
where
    DB: Backend,
    i32: FromSql<Integer, DB>,
{
    type Row = i32;
    fn build(repr: i32) -> diesel::deserialize::Result<Self> {
        Ok(repr.try_into()?)
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

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = module_relationships)]
#[diesel(check_for_backend(Sqlite))]
pub struct ModuleRelationship {
    #[diesel(column_name = relationship_id)]
    pub id: DepId,
    pub group_id: DepGroupId,
    pub ordinal: i32,
    pub target_name: String,
    pub target_version: Option<String>,
    pub target_version_min: Option<String>,
}

impl ModuleRelationship {
    #[dsl::auto_type(no_type_alias)]
    pub fn all() -> _ {
        let select: AllDeps = module_relationships::table.select(Self::as_select());
        select.order(module_relationships::ordinal)
    }

    #[dsl::auto_type(no_type_alias)]
    pub fn in_group(group_id: DepGroupId) -> _ {
        module_relationships::group_id.eq(group_id)
    }
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
