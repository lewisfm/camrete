//! Wrappers for use from other languages.
//!
//! Having a separate API means that Camrete's main code can be idiomatic Rust while still allowing
//! for an easy-to-use API from C#.

use crate::{
    DbConnection, Result,
    database::{
        self, ModuleId, ReleaseId,
        models::{
            Module, ModuleRelease, Repository,
            module::{ModuleRelationship, ModuleRelationshipGroup},
        },
    },
    repo,
};
use diesel::{OptionalExtension, QueryDsl, RunQueryDsl};
use parking_lot::{Mutex, MutexGuard, RwLock};

#[derive(Debug, uniffi::Object)]
struct RepoManager {
    mgr: RwLock<repo::RepoManager>,
}

#[uniffi::export]
impl RepoManager {
    #[uniffi::constructor]
    fn new(url: String) -> crate::Result<Self> {
        Ok(Self {
            mgr: RwLock::new(repo::RepoManager::new(&url)?),
        })
    }

    fn database(&self) -> crate::Result<RepoDB> {
        Ok(self.mgr.read().db()?.into())
    }
}

#[derive(uniffi::Object)]
struct RepoDB {
    db: Mutex<database::RepoDB<DbConnection>>,
}

impl From<database::RepoDB<DbConnection>> for RepoDB {
    fn from(value: database::RepoDB<DbConnection>) -> Self {
        Self {
            db: Mutex::new(value),
        }
    }
}

impl RepoDB {
    fn db(&self) -> MutexGuard<'_, database::RepoDB<DbConnection>> {
        self.db.lock()
    }
}

#[uniffi::export]
impl RepoDB {
    pub fn all_repos(&self, create_default: bool) -> Result<Vec<Repository>> {
        Ok(self.db.lock().all_repos(create_default)?)
    }

    pub fn module_by_slug(&self, slug: String) -> Result<Option<Module>> {
        let module = Module::all()
            .filter(Module::with_slug(&slug))
            .get_result(self.db().as_mut())
            .optional()?;

        Ok(module)
    }

    pub fn releases_with_parent(&self, parent_id: ModuleId) -> Result<Vec<ModuleRelease>> {
        let releases = ModuleRelease::all()
            .filter(ModuleRelease::with_parent(parent_id))
            .order_by(ModuleRelease::by_version())
            .load(self.db().as_mut())?;

        Ok(releases)
    }

    pub fn associated_release_data(&self, release_id: ReleaseId) -> Result<AssociatedReleaseData> {
        let mut db = self.db();

        let tags = ModuleRelease::tags_for(release_id).load(db.as_mut())?;
        let authors = ModuleRelease::authors_for(release_id).load(db.as_mut())?;
        let licenses = ModuleRelease::licenses_for(release_id).load(db.as_mut())?;
        let locales = ModuleRelease::locales_for(release_id).load(db.as_mut())?;

        Ok(AssociatedReleaseData {
            tags,
            authors,
            licenses,
            locales,
        })
    }

    pub fn relationships_for_release(
        &self,
        release_id: ReleaseId,
    ) -> Result<Vec<FullRelationship>> {
        let mut db = self.db();

        let relationships =
            ModuleRelease::relationships_for(release_id).load::<FullRelationship>(db.as_mut())?;

        Ok(relationships)
    }
}

#[derive(uniffi::Record)]
struct AssociatedReleaseData {
    tags: Vec<String>,
    authors: Vec<String>,
    licenses: Vec<String>,
    locales: Vec<String>,
}

#[derive(Debug, diesel::Queryable, uniffi::Record)]
struct FullRelationship {
    group: ModuleRelationshipGroup,
    description: ModuleRelationship,
}
