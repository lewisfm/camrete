use std::sync::LazyLock;

use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool, PooledConnection},
};
use directories::ProjectDirs;
use miette::Diagnostic;
use repo::client::RepoUnpackError;
use thiserror::Error;

use crate::json::JsonError;

extern crate serde_json as simd_json;

pub mod database;
mod io;
pub mod json;
pub mod repo;

pub type Result<T, E = Error> = std::result::Result<T, E>;
pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;
pub type DbConnection = PooledConnection<ConnectionManager<SqliteConnection>>;

pub static DIRS: LazyLock<ProjectDirs> =
    LazyLock::new(|| ProjectDirs::from("", "", "CKAN").expect("user home dir available"));

static USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " <https://github.com/lewisfm/camrete>"
);

#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    #[error("failed to open the on-device CKAN database")]
    #[diagnostic(code(camrete::database::cannot_open))]
    DbConnection(#[from] diesel::ConnectionError),

    #[error("failed to establish a connection pool for the on-device CKAN database")]
    DbPool(#[from] diesel::r2d2::PoolError),

    #[error("failed to upgrade the on-device CKAN database")]
    #[diagnostic(code(camrete::database::upgrade_failure))]
    DbMigrations(Box<dyn std::error::Error + Send + Sync>),

    #[error("a request to the on-device CKAN database failed")]
    #[diagnostic(code(camrete::database::request_failure))]
    Db(#[from] diesel::result::Error),

    #[error("HTTP request failed")]
    #[diagnostic(code(camrete::http))]
    Http(#[from] reqwest::Error),

    #[error("failed to unpack a CKAN repository")]
    #[diagnostic(transparent)]
    Network(#[from] RepoUnpackError),

    #[error(transparent)]
    #[diagnostic(code(camrete::io))]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Json(#[from] JsonError),
}

impl From<diesel::r2d2::Error> for Error {
    fn from(value: diesel::r2d2::Error) -> Self {
        match value {
            diesel::r2d2::Error::ConnectionError(e) => e.into(),
            diesel::r2d2::Error::QueryError(e) => e.into(),
        }
    }
}

impl From<simd_json::Error> for Error {
    fn from(value: simd_json::Error) -> Self {
        JsonError::Parse(value).into()
    }
}

// impl<E: Into<Error> + std::error::Error> From<TransactionError<E>> for Error {
//     fn from(value: TransactionError<E>) -> Self {
//         match value {
//             TransactionError::Connection(e) => Self::Db(e),
//             TransactionError::Transaction(e) => e.into(),
//         }
//     }
// }

// uniffi::custom_type!(Url, String, {
//     remote,
//     lower: |s| s.to_string(),
//     try_lift: |s| Ok(Url::parse(&s)?),
// });

// uniffi::setup_scaffolding!();
