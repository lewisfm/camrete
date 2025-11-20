use std::{
    any::type_name,
    collections::HashMap,
    fmt::{Debug, Formatter, Pointer},
    marker::PhantomData,
    ops::DerefMut,
    sync::Arc,
};

use derive_more::From;
use diesel::{
    Connection, ExpressionMethods, Queryable, RunQueryDsl, SqliteConnection,
    backend::Backend,
    deserialize::FromSql,
    expression::AsExpression,
    insert_into, replace_into,
    serialize::{Output, ToSql},
    sql_types::Integer,
    update,
    upsert::excluded,
};
use reqwest::header::HeaderValue;
use tracing::{debug, info, instrument, trace};
use url::Url;

use crate::{
    DbConnection, Error,
    database::{
        models::{Build, NewModule, NewRelease, RepositoryRef, helpers::JsonbValue},
        schema::{modules, repositories},
    },
    repo::client::RepoUnpackError,
};

pub mod models;
pub mod schema;

#[derive(From)]
pub struct RepoDB<T> {
    pub connection: T,
}

impl<T: DerefMut<Target = SqliteConnection>> RepoDB<T> {
    pub fn new(connection: T) -> Self {
        Self { connection }
    }

    #[instrument(skip_all)]
    pub fn transaction<R>(
        &mut self,
        func: impl FnOnce(RepoDB<&mut SqliteConnection>) -> Result<R, Error>,
    ) -> Result<R, Error> {
        trace!("Performing a transaction");
        self.connection.transaction(|conn| func(RepoDB::new(conn)))
    }

    /// Create a new repository with the given name. Any previous repository
    /// with the same name will be overwritten.
    #[instrument(skip_all)]
    pub fn create_empty_repo(&mut self, new_repo: RepositoryRef<'_>) -> Result<RepoId, Error> {
        use schema::repositories::dsl::*;

        info!(
            name = ?new_repo.name,
            url = ?new_repo.url,
            "Creating a new repository"
        );

        let id = replace_into(repositories)
            .values(new_repo)
            .returning(repo_id)
            .get_result::<RepoId>(&mut *self.connection)?;

        Ok(id)
    }

    /// Register a module with the given name. This will never overwrite any module, it just
    /// ensures one exists and returns its ID.
    #[instrument(skip_all)]
    pub fn register_module(&mut self, new_module: NewModule) -> Result<ModuleId, Error> {
        use schema::modules::dsl::*;

        debug!(
            repo_id = ?new_module.repo_id,
            name = ?new_module.module_name,
            "Registering a module"
        );

        let id = insert_into(modules)
            .values(new_module)
            .on_conflict((repo_id, module_name))
            .do_update()
            .set(module_name.eq(excluded(module_name)))
            .returning(module_id)
            .get_result::<ModuleId>(&mut *self.connection)?;

        Ok(id)
    }

    /// Add a release to an existing module.
    #[instrument(skip_all)]
    pub fn create_release(&mut self, new_release: NewRelease) -> Result<ReleaseId, Error> {
        use schema::module_releases::dsl::*;

        debug!(
            mod_id = ?new_release.module_id,
            version = ?new_release.version,
            "Creating release"
        );

        let id = replace_into(module_releases)
            .values(new_release)
            .returning(release_id)
            .get_result::<ReleaseId>(&mut *self.connection)?;

        Ok(id)
    }

    /// Add the given builds to the build-id/version map.
    #[instrument(skip_all)]
    pub fn register_builds(&mut self, new_builds: Vec<Build>) -> Result<(), Error> {
        use schema::builds::dsl::*;

        debug!(count = %new_builds.len(), "Registering new builds");

        replace_into(builds)
            .values(new_builds)
            .execute(&mut *self.connection)?;

        Ok(())
    }

    /// Attach the given download counts to their corresponding modules. Creates the modules if they
    /// don't exist yet.
    #[instrument(skip(self, counts))]
    pub fn add_download_counts<'a, C>(&mut self, repo: RepoId, counts: C) -> Result<(), Error>
    where
        C: IntoIterator<Item = (&'a String, &'a i32)>,
        C::IntoIter: ExactSizeIterator,
    {
        use schema::modules::dsl::*;

        let counts = counts.into_iter();
        debug!(num_counts = %counts.len(), "Adding download counts to modules");

        for (mod_name, count) in counts {
            insert_into(modules)
                .values((
                    repo_id.eq(repo),
                    module_name.eq(mod_name),
                    download_count.eq(count),
                ))
                .on_conflict((repo_id, module_name))
                .do_update()
                .set(download_count.eq(count))
                .execute(&mut *self.connection)?;
        }

        Ok(())
    }

    #[instrument(skip(self))]
    pub fn add_repo_ref(&mut self, referrer: RepoId, new_ref: RepositoryRef) -> Result<(), Error> {
        use schema::repository_refs::dsl::*;

        replace_into(repository_refs)
            .values((referrer_id.eq(referrer), new_ref))
            .execute(&mut *self.connection)?;

        Ok(())
    }

    pub fn set_etag(
        &mut self,
        source_url: Arc<Url>,
        etag_header: Option<&HeaderValue>,
    ) -> Result<(), Error> {
        use schema::etags::dsl::*;

        let encoded_url = JsonbValue::from(&*source_url);
        let etag_str = if let Some(value) = etag_header {
            let str = value
                .to_str()
                .map_err(|_| RepoUnpackError::InvalidEtag { url: source_url })?;
            Some(str)
        } else {
            None
        };

        replace_into(etags)
            .values((url.eq(encoded_url), etag.eq(etag_str)))
            .execute(&mut *self.connection)?;

        Ok(())
    }
}

#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, AsExpression)]
#[diesel(sql_type = Integer)]
pub struct Id<T>(pub i32, PhantomData<T>);

impl<T> Id<T> {
    pub fn new(id: i32) -> Self {
        Self(id, PhantomData)
    }

    pub fn get(self) -> i32 {
        self.0
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> From<i32> for Id<T> {
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

impl<T> From<Id<T>> for i32 {
    fn from(value: Id<T>) -> Self {
        value.0
    }
}

impl<T: Debug + Default> Debug for Id<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Id<{:?}>({})", T::default(), self.get())
    }
}

impl<DB, T: Debug + Default> ToSql<Integer, DB> for Id<T>
where
    DB: Backend,
    i32: ToSql<Integer, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> diesel::serialize::Result {
        self.0.to_sql(out)
    }
}

impl<DB, T> Queryable<Integer, DB> for Id<T>
where
    DB: Backend,
    i32: FromSql<Integer, DB>,
{
    type Row = i32;
    fn build(id: i32) -> diesel::deserialize::Result<Self> {
        Ok(id.into())
    }
}

mod tag {
    macro_rules! tag {
        ($($name:ident),*) => {
            $(
            #[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $name;
            )*
        };
    }

    tag!(Repo, Module, Release);
}

pub type RepoId = Id<tag::Repo>;
pub type ModuleId = Id<tag::Module>;
pub type ReleaseId = Id<tag::Release>;
