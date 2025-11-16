//! Adapter structs for reading JSON-based NetKAN archives.

pub mod game_version;
mod one_or_many;
pub mod spec_version;

use std::{borrow::Cow, collections::HashMap};

use game_version::GameVersionSpec;
use serde::Deserialize;
use spec_version::SpecVersion;
use time::{OffsetDateTime, serde::iso8601};
use url::Url;

#[derive(Debug, Deserialize)]
pub struct JsonModule {
    pub spec_version: SpecVersion,
    pub name: String,
    pub identifier: String,
    pub version: String,
    #[serde(default)]
    pub kind: ModuleKind,
    pub r#abstract: String,
    pub description: Option<String>,
    pub comment: Option<String>,
    #[serde(with = "one_or_many")]
    pub author: Vec<String>,
    #[serde(with = "one_or_many")]
    #[serde(default)]
    pub download: Vec<Url>,
    #[serde(default)]
    pub download_size: Option<u32>,
    #[serde(default)]
    pub download_hash: ModuleChecksum,
    #[serde(default)]
    pub download_content_type: Option<String>,
    #[serde(default)]
    pub install_size: Option<u32>,
    #[serde(with = "one_or_many")]
    #[serde(default)]
    pub license: Vec<String>,
    #[serde(default)]
    pub ksp_version: GameVersionSpec,
    #[serde(default)]
    pub ksp_version_min: GameVersionSpec,
    #[serde(default)]
    pub ksp_version_max: GameVersionSpec,
    #[serde(default)]
    pub resources: ModuleResources,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub depends: Vec<RelationshipDescriptor>,
    #[serde(default)]
    pub conflicts: Vec<RelationshipDescriptor>,
    #[serde(default)]
    pub install: Vec<ModuleInstallDescriptor>,
    #[serde(with = "iso8601::option", default)]
    pub release_date: Option<OffsetDateTime>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModuleKind {
    #[default]
    Package,
    Metapackage,
    Dlc,
}

#[derive(Debug, Deserialize, Default)]
pub struct ModuleResources {
    pub homepage: Option<String>,
    pub spacedock: Option<String>,
    pub repository: Option<String>,
    pub bugtracker: Option<String>,
    #[serde(rename = "remote-avc")]
    pub remote_avc: Option<String>,
    pub x_screenshot: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MetaRelationship {
    #[serde(flatten)]
    pub descriptor: RelationshipDescriptor,
    #[serde(default)]
    pub choice_help_text: Option<String>,
    #[serde(default)]
    pub suppress_recommendations: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum RelationshipDescriptor {
    Direct(DirectRelationshipDescriptor),
    AnyOf(AnyOfRelationshipDescriptor),
}

#[derive(Debug, Deserialize)]
pub struct DirectRelationshipDescriptor {
    pub name: String,
    #[serde(default)]
    pub max_version: Option<String>,
    #[serde(default)]
    pub min_version: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AnyOfRelationshipDescriptor {
    pub any_of: Vec<MetaRelationship>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ModuleChecksum {
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModuleInstallDescriptor {
    #[serde(flatten)]
    pub source: ModuleInstallSourceDirective,
    pub install_to: String,
    #[serde(default)]
    pub find_matches_files: bool,
    #[serde(default)]
    pub r#as: Option<String>,
    #[serde(default)]
    #[serde(with = "one_or_many")]
    pub filter: Vec<String>,
    #[serde(default)]
    #[serde(with = "one_or_many")]
    pub filter_regexp: Vec<String>,
    #[serde(default)]
    #[serde(with = "one_or_many")]
    pub include_only: Vec<String>,
    #[serde(default)]
    #[serde(with = "one_or_many")]
    pub include_only_regexp: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleInstallSourceDirective {
    File(String),
    Find(String),
    FindRegexp(String),
}

#[derive(Debug, Deserialize)]
pub struct JsonBuilds<'a> {
    pub builds: HashMap<Cow<'a, str>, Cow<'a, str>>,
}
