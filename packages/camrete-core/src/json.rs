//! Adapter structs for reading JSON-based NetKAN archives.

pub mod game_version;
mod one_or_many;
pub mod spec_version;

use std::{borrow::Cow, collections::HashMap};

use derive_more::TryFrom;
use game_version::MetaGameVersion;
use miette::Diagnostic;
use serde::{Deserialize, Serialize};
use spec_version::SpecVersion;
use thiserror::Error;
use time::{OffsetDateTime, serde::iso8601};
use url::Url;

use crate::{
    database::models::{RepositoryRef, module::RelationshipType},
    repo::game::GameVersion,
};

#[derive(Debug, Error, Diagnostic)]
pub enum JsonError {
    #[error(
        "The module incorrectly specifies both `ksp_version` (as {:?}) and `{}` (as {:?}).",
        generic_constraint,
        if *specific_is_max { "ksp_version_max" } else { "ksp_version_min" },
        specific_constraint,
    )]
    #[diagnostic(code(camrete::json::duplicate_module_version_constraint))]
    DuplicateVersionConstraint {
        generic_constraint: GameVersion,
        specific_is_max: bool,
        specific_constraint: GameVersion,
    },
    #[error("The module incorrectly specifies `max_version` in its `replaced_by` relationship.")]
    #[diagnostic(code(camrete::json::disallowed_replaced_by_max_version))]
    DisallowedMaxVersionInReplacement,
    #[diagnostic(code(camrete::json::parse))]
    #[error(transparent)]
    Parse(#[from] serde_json::Error),
}

/// A full complete release of a module, suitable for encoding into JSON.
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
    #[serde(default)]
    pub release_status: ReleaseStatus,
    pub comment: Option<String>,
    #[serde(with = "one_or_many")]
    pub author: Vec<String>,
    #[serde(with = "one_or_many")]
    #[serde(default)]
    pub download: Vec<Url>,
    #[serde(default)]
    pub download_size: Option<i64>,
    #[serde(default)]
    pub download_hash: DownloadChecksum,
    #[serde(default)]
    pub download_content_type: Option<String>,
    #[serde(default)]
    pub install_size: Option<i64>,
    #[serde(with = "one_or_many")]
    #[serde(default)]
    pub license: Vec<String>,
    #[serde(default)]
    pub ksp_version: MetaGameVersion,
    #[serde(default)]
    pub ksp_version_min: MetaGameVersion,
    #[serde(default)]
    pub ksp_version_max: MetaGameVersion,
    #[serde(default)]
    pub ksp_version_strict: bool,
    #[serde(default)]
    pub resources: ModuleResources,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub localizations: Vec<String>,
    #[serde(default)]
    pub depends: Vec<MetaRelationship>,
    #[serde(default)]
    pub recommends: Vec<MetaRelationship>,
    #[serde(default)]
    pub suggests: Vec<MetaRelationship>,
    #[serde(default)]
    pub supports: Vec<MetaRelationship>,
    #[serde(default)]
    pub conflicts: Vec<MetaRelationship>,
    #[serde(default)]
    pub replaced_by: Option<DirectRelationshipDescriptor>,
    #[serde(default)]
    pub install: Vec<ModuleInstallDescriptor>,
    #[serde(with = "iso8601::option", default)]
    pub release_date: Option<OffsetDateTime>,
}

impl JsonModule {
    pub fn verify(&self) -> Result<(), JsonError> {
        if !self.ksp_version.is_empty() {
            let has_max = !self.ksp_version_max.is_empty();
            if has_max || !self.ksp_version_min.is_empty() {
                return Err(JsonError::DuplicateVersionConstraint {
                    generic_constraint: *self.ksp_version,
                    specific_is_max: has_max,
                    specific_constraint: if has_max {
                        *self.ksp_version_max
                    } else {
                        *self.ksp_version_min
                    },
                });
            }
        }

        if let Some(replaced_by) = &self.replaced_by
            && replaced_by.max_version.is_some()
        {
            return Err(JsonError::DisallowedMaxVersionInReplacement);
        }

        Ok(())
    }

    /// Returns all relationships (not including any replaced_by specs)
    /// alongside their corresponding types.
    pub fn relationships(&self) -> impl Iterator<Item = (RelationshipType, &MetaRelationship)> {
        self.depends
            .iter()
            .map(|d| (RelationshipType::Depends, d))
            .chain(
                self.recommends
                    .iter()
                    .map(|d| (RelationshipType::Recommends, d)),
            )
            .chain(
                self.suggests
                    .iter()
                    .map(|d| (RelationshipType::Suggests, d)),
            )
            .chain(
                self.supports
                    .iter()
                    .map(|d| (RelationshipType::Supports, d)),
            )
            .chain(
                self.conflicts
                    .iter()
                    .map(|d| (RelationshipType::Conflicts, d)),
            )
    }
}

#[derive(Debug, Deserialize, Default, TryFrom, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
#[try_from(repr)]
#[repr(i32)]
pub enum ModuleKind {
    #[default]
    Package = 0,
    Metapackage,
    Dlc,
}

impl From<ModuleKind> for i32 {
    fn from(value: ModuleKind) -> Self {
        value as i32
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ModuleResources {
    pub homepage: Option<String>,
    pub spacedock: Option<String>,
    pub repository: Option<String>,
    pub bugtracker: Option<String>,
    #[serde(rename = "remote-avc")]
    pub remote_avc: Option<String>,
    pub x_screenshot: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetaRelationship {
    #[serde(flatten)]
    pub descriptor: RelationshipDescriptor,
    #[serde(default)]
    pub choice_help_text: Option<String>,
    #[serde(default)]
    pub suppress_recommendations: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RelationshipDescriptor {
    Direct(DirectRelationshipDescriptor),
    AnyOf(AnyOfRelationshipDescriptor),
}

impl RelationshipDescriptor {
    pub fn flatten(&self) -> Vec<&DirectRelationshipDescriptor> {
        let mut members = vec![];
        self.flatten_inner(&mut members);
        members
    }

    fn flatten_inner<'a>(&'a self, list: &mut Vec<&'a DirectRelationshipDescriptor>) {
        match self {
            Self::Direct(d) => list.push(d),
            Self::AnyOf(d) => {
                for relation in &d.any_of {
                    relation.descriptor.flatten_inner(list);
                }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirectRelationshipDescriptor {
    pub name: String,
    #[serde(default)]
    pub max_version: Option<String>,
    #[serde(default)]
    pub min_version: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnyOfRelationshipDescriptor {
    pub any_of: Vec<MetaRelationship>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct DownloadChecksum {
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ModuleInstallSourceDirective {
    File(String),
    Find(String),
    FindRegexp(String),
}

#[derive(
    Debug, Serialize, Deserialize, Default, TryFrom, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "snake_case")]
#[try_from(repr)]
#[repr(i32)]
pub enum ReleaseStatus {
    #[default]
    Stable = 0,
    Testing,
    Development,
}

impl From<ReleaseStatus> for i32 {
    fn from(value: ReleaseStatus) -> Self {
        value as i32
    }
}

#[derive(Debug, Deserialize)]
pub struct JsonBuilds<'a> {
    pub builds: HashMap<i32, Cow<'a, str>>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Hash)]
pub struct RepositoryRefList {
    pub repositories: Vec<RepositoryRef<'static>>,
}
