use camrete_core::{
    DbConnection, database::RepoDB as CoreRepoDB, diesel, repo::RepoManager as CoreRepoManager
};
use parking_lot::{Mutex, RwLock};
use std::fmt::{Display, Formatter};

#[derive(Debug, uniffi::Error)]
#[uniffi(flat_error)]
pub enum CamreteError {
    Core(camrete_core::Error),
}

impl Display for CamreteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CamreteError::Core(error) => error.fmt(f),
        }
    }
}

impl From<camrete_core::Error> for CamreteError {
    fn from(value: camrete_core::Error) -> Self {
        Self::Core(value)
    }
}

impl From<diesel::result::Error> for CamreteError {
    fn from(value: diesel::result::Error) -> Self {
        Self::Core(value.into())
    }
}

#[derive(Debug, uniffi::Object)]
struct RepoManager {
    mgr: RwLock<CoreRepoManager>,
}

#[uniffi::export]
impl RepoManager {
    #[uniffi::constructor]
    fn new(url: String) -> Result<Self, CamreteError> {
        Ok(Self {
            mgr: RwLock::new(CoreRepoManager::new(&url)?),
        })
    }

    fn database(&self) -> Result<RepoDB, CamreteError> {
        Ok(self.mgr.read().db()?.into())
    }
}

#[derive(uniffi::Object)]
struct RepoDB {
    db: Mutex<CoreRepoDB<DbConnection>>,
}

impl From<CoreRepoDB<DbConnection>> for RepoDB {
    fn from(value: CoreRepoDB<DbConnection>) -> Self {
        Self {
            db: Mutex::new(value),
        }
    }
}

#[uniffi::export]
impl RepoDB {
    pub fn all_repos(&self, create_default: bool) -> Result<Vec<String>, CamreteError> {
        let repos = self.db.lock().all_repos(create_default)?;
        Ok(repos.into_iter().map(|r| r.name).collect())
    }
}

uniffi::setup_scaffolding!();
