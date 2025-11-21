use std::{borrow::Cow, ops::DerefMut, sync::Arc};

use derive_more::From;
use diesel::{insert_into, prelude::*, replace_into, update, upsert::excluded};
use reqwest::header::HeaderValue;
use tokio::{runtime::Handle, task::block_in_place};
use tracing::{debug, info, instrument, trace};
use url::Url;

use crate::{
    Error,
    database::{
        models::{
            BuildRecord, NewModule, NewRelease, ReleaseMetadata, Repository, RepositoryRef,
            module::{
                NewModuleAuthor, NewModuleLocale, NewModuleRelationship,
                NewModuleRelationshipGroup, NewModuleTag, SortableRelease,
            },
        },
        schema::*,
    },
    json::JsonModule,
    repo::client::RepoUnpackError,
};

mod helpers;
pub mod models;
pub mod schema;

pub use helpers::*;

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

    /// Fetches all repositories from the database, ordered by name. If `create_default` is specified and
    /// no repos currently exist, the default repo will be created and returned.
    #[instrument(skip(self))]
    pub fn all_repos(&mut self, create_default: bool) -> QueryResult<Vec<Repository>> {
        use schema::repositories::dsl::*;

        debug!("Loading repository list");

        let mut repos = Repository::all().get_results(&mut *self.connection)?;

        if create_default && repos.is_empty() {
            info!("Creating default repository");

            let default_url =
                Url::parse("https://github.com/KSP-CKAN/CKAN-meta/archive/master.tar.gz").unwrap();
            let default_repo = RepositoryRef::shared("KSP-default", &default_url);

            repos = insert_into(repositories)
                .values(default_repo)
                .returning(Repository::as_returning())
                .get_results(&mut *self.connection)?;
        }

        Ok(repos)
    }

    /// Create a new repository with the given name. Any previous repository
    /// with the same name will be overwritten.
    #[instrument(skip_all)]
    pub fn create_empty_repo(&mut self, new_repo: RepositoryRef<'_>) -> QueryResult<Repository> {
        use schema::repositories::dsl::*;

        info!(
            name = ?new_repo.name,
            url = ?new_repo.url,
            "Creating an empty repository"
        );

        let id = replace_into(repositories)
            .values(new_repo)
            .returning(Repository::as_returning())
            .get_result(&mut *self.connection)?;

        Ok(id)
    }

    /// Register a module with the given name. This will never overwrite any module, it just
    /// ensures one exists and returns its ID.
    #[instrument(skip_all)]
    pub fn register_module(&mut self, new_module: NewModule) -> QueryResult<ModuleId> {
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
    pub fn create_release(
        &mut self,
        json: &JsonModule,
        repo_id: RepoId,
    ) -> QueryResult<(ModuleId, ReleaseId)> {
        debug!(
            mod_name = ?json.name,
            version = ?json.version,
            "Creating release"
        );

        let module_id = self.register_module(NewModule {
            repo_id,
            module_name: &json.name,
        })?;

        let metadata = ReleaseMetadata {
            comment: json.comment.as_deref().map(Cow::Borrowed),
            download: Cow::Borrowed(&json.download),
            download_content_type: json.download_content_type.as_deref().map(Cow::Borrowed),
            download_hash: Cow::Borrowed(&json.download_hash),
            install: Cow::Borrowed(&json.install),
            resources: Cow::Borrowed(&json.resources),
        };

        let new_release = NewRelease {
            module_id,
            version: &json.version,
            kind: json.kind,
            summary: &json.r#abstract,
            metadata,
            description: json.description.as_deref(),
            release_status: json.release_status,
            game_version: if !json.ksp_version.is_empty() {
                json.ksp_version.into()
            } else {
                json.ksp_version_min.into()
            },
            game_version_min: json.ksp_version_min.into(),
            game_version_strict: json.ksp_version_strict,
            download_size: json.download_size,
            install_size: json.install_size,
            release_date: json.release_date,
        };

        // Some mods have duplicate releases, which isn't allowed but it's better to ignore that
        // than to error here.
        let release_id = replace_into(module_releases::table)
            .values(new_release)
            .returning(module_releases::release_id)
            .get_result::<ReleaseId>(&mut *self.connection)?;

        // Add auxiliary many-to-one tables - tags, authors, locales, dependencies.
        // These aren't included in the encoded metadata so they can be easily searched.

        let tags = json
            .tags
            .iter()
            .enumerate()
            .map(|(ordinal, tag)| NewModuleTag {
                release_id,
                ordinal: ordinal.try_into().unwrap(),
                tag,
            })
            .collect::<Vec<_>>();

        insert_into(module_tags::table)
            .values(tags)
            .execute(&mut *self.connection)?;

        let authors = json
            .author
            .iter()
            .enumerate()
            .map(|(ordinal, author)| NewModuleAuthor {
                release_id,
                ordinal: ordinal.try_into().unwrap(),
                author,
            })
            .collect::<Vec<_>>();

        insert_into(module_authors::table)
            .values(authors)
            .execute(&mut *self.connection)?;

        let locales = json
            .localizations
            .iter()
            .map(|locale| NewModuleLocale { release_id, locale })
            .collect::<Vec<_>>();

        insert_into(module_localizations::table)
            .values(locales)
            .execute(&mut *self.connection)?;

        // Relationships are a little more complicated because they can be stored either as direct or any_of groups.
        // In the database these are the same thing, so we have to convert first.

        for (ordinal, (rel_type, relation)) in json.relationships().enumerate() {
            let group = NewModuleRelationshipGroup {
                release_id,
                ordinal: ordinal.try_into().unwrap(),
                rel_type,
                choice_help_text: relation.choice_help_text.as_deref(),
                suppress_recommendations: relation.suppress_recommendations,
            };

            // Insert by explicitly specifying each column so Diesel can infer the correct InsertValues.
            // Convert `rel_type` to a SQL-compatible value (here using `.into()` / cast to i32 if appropriate).
            let group_id = insert_into(module_relationship_groups::table)
                .values(group)
                .returning(module_relationship_groups::group_id)
                .get_result::<DepGroupId>(&mut *self.connection)?;

            let members = relation
                .descriptor
                .flatten()
                .into_iter()
                .enumerate()
                .map(|(ordinal, member)| NewModuleRelationship {
                    group_id,
                    ordinal: ordinal.try_into().unwrap(),
                    target_name: &member.name,
                    target_version: member.max_version.as_deref().or(member.version.as_deref()),
                    target_version_min: member.min_version.as_deref(),
                })
                .collect::<Vec<_>>();

            insert_into(module_relationships::table)
                .values(members)
                .execute(&mut *self.connection)?;
        }

        Ok((module_id, release_id))
    }

    /// Add the given builds to the build-id/version map.
    #[instrument(skip_all)]
    pub fn register_builds(&mut self, new_builds: Vec<BuildRecord>) -> QueryResult<()> {
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
    pub fn add_download_counts<'a, C>(&mut self, repo: RepoId, counts: C) -> QueryResult<()>
    where
        C: IntoIterator<Item = (&'a String, &'a i32)>,
        C::IntoIter: ExactSizeIterator,
    {
        use schema::modules::dsl::*;

        let counts = counts.into_iter();
        debug!(num_counts = %counts.len(), "Adding download counts to modules");

        let rows = counts
            .map(|(name, count)| {
                (
                    repo_id.eq(repo),
                    module_name.eq(name),
                    download_count.eq(count),
                )
            })
            .collect::<Vec<_>>();

        insert_into(modules)
            .values(rows)
            .on_conflict((repo_id, module_name))
            .do_update()
            .set(download_count.eq(excluded(download_count)))
            .execute(&mut *self.connection)?;

        Ok(())
    }

    #[instrument(skip(self))]
    pub fn add_repo_ref(&mut self, referrer: RepoId, new_ref: RepositoryRef) -> QueryResult<()> {
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

    /// Update the sort order of the module.
    #[instrument(skip(self))]
    pub fn update_derived_module_data(&mut self, mod_id: ModuleId) -> QueryResult<()> {
        debug!("Recomputing derived module data");

        let mut releases = SortableRelease::all()
            .filter(SortableRelease::with_parent(mod_id))
            .get_results(&mut *self.connection)?;

        releases.sort_unstable_by(|l, r| l.version.cmp(&r.version));

        trace!(num_releases = %releases.len());

        let mut is_most_recent = true;
        for (ordinal, release) in releases.into_iter().enumerate().rev() {
            update(module_releases::table)
                .filter(module_releases::release_id.eq(release.release_id))
                .set((
                    module_releases::sort_index.eq(ordinal as i32),
                    module_releases::up_to_date.eq(is_most_recent),
                ))
                .execute(&mut *self.connection)?;

            is_most_recent = false;
        }

        Ok(())
    }
}

impl<T: DerefMut<Target = SqliteConnection> + Send> RepoDB<T> {
    #[instrument(skip_all)]
    pub fn async_transaction<R>(
        &mut self,
        func: impl AsyncFnOnce(RepoDB<&mut SqliteConnection>) -> Result<R, Error>,
    ) -> Result<R, Error> {
        trace!("Performing a transaction");
        block_in_place(|| {
            self.connection.transaction(|conn| {
                Handle::current().block_on(async move { func(RepoDB::new(conn)).await })
            })
        })
    }
}
