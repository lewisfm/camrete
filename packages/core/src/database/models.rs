use diesel::{prelude::*, sqlite::Sqlite};

use crate::{
    database::{JsonbValue, schema::*},
    repo::game::GameVersion,
};

pub mod module;
pub mod repository;

pub use module::{Module, ModuleRelease, NewModule, NewRelease, ReleaseMetadata};
pub use repository::{Repository, RepositoryRef};

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = builds)]
#[diesel(check_for_backend(Sqlite))]
pub struct BuildRecord {
    pub build_id: i32,
    #[diesel(serialize_as = JsonbValue, deserialize_as = JsonbValue)]
    pub version: GameVersion,
}
