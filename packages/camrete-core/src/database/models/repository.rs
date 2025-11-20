use std::borrow::Cow;

use diesel::{
    dsl::{AsSelect, Select},
    prelude::*,
    sqlite::Sqlite,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::database::{JsonbValue, RepoId, schema::*};

type All = Select<repositories::table, AsSelect<Repository, Sqlite>>;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = repositories)]
#[diesel(check_for_backend(Sqlite))]
pub struct Repository {
    pub repo_id: RepoId,
    pub name: String,
    #[diesel(deserialize_as = JsonbValue)]
    pub url: Url,
    pub priority: i32,
}

impl Repository {
    pub fn all() -> All {
        repositories::table.select(Self::as_select())
    }

    pub fn as_ref(&self) -> RepositoryRef<'_> {
        RepositoryRef {
            name: Cow::Borrowed(&self.name),
            url: Cow::Borrowed(&self.url),
            priority: self.priority,
        }
    }
}

#[derive(Debug, Insertable, Deserialize, Serialize, PartialEq, Eq, Hash, Clone)]
#[diesel(table_name = repositories)]
#[diesel(table_name = repository_refs)]
#[diesel(check_for_backend(Sqlite))]
pub struct RepositoryRef<'a> {
    pub name: Cow<'a, str>,
    #[diesel(serialize_as = JsonbValue)]
    #[serde(rename = "uri")]
    pub url: Cow<'a, Url>,
    #[serde(default)]
    pub priority: i32,
}

impl<'a> RepositoryRef<'a> {
    pub fn new(name: String, url: Url) -> Self {
        Self {
            name: Cow::Owned(name),
            url: Cow::Owned(url),
            priority: 0,
        }
    }

    pub fn shared(name: &'a str, url: &'a Url) -> Self {
        Self {
            name: Cow::Borrowed(name),
            url: Cow::Borrowed(url),
            priority: 0,
        }
    }
}
