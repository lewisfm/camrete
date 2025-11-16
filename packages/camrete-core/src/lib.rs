use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::LazyLock,
};

use diesel::prelude::*;
use directories::ProjectDirs;
use miette::Diagnostic;
use percent_encoding::percent_encode;
use repo::{RepoDescription, client::RepoUnpackError};
use thiserror::Error;
use url::Url;

use crate::repo::game::GameVersionParseError;

extern crate serde_json as simd_json;

pub mod json;
mod models;
pub mod repo;
mod schema;
mod io;

pub type Result<T, E = Error> = std::result::Result<T, E>;
pub type DatabaseConnection = SqliteConnection;

pub static DIRS: LazyLock<ProjectDirs> =
    LazyLock::new(|| ProjectDirs::from("", "", "CKAN").expect("user home dir available"));

static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    #[error("failed to open the on-device CKAN database")]
    #[diagnostic(code(camrete::database::open))]
    DbConnection(#[from] diesel::ConnectionError),
    #[error("A request to the on-device CKAN database failed")]
    #[diagnostic(code(camrete::database::request))]
    Db(#[from] diesel::result::Error),
    #[error("HTTP request failed")]
    #[diagnostic(code(camrete::http))]
    Http(#[from] reqwest::Error),
    #[error("Failed to unpack a CKAN repository")]
    #[diagnostic(transparent)]
    Network(#[from] RepoUnpackError),
    #[error(transparent)]
    #[diagnostic(code(camrete::io))]
    Io(#[from] std::io::Error),
    #[error("failed to parse a JSON document")]
    #[diagnostic(code(camrete::json))]
    Json(#[from] simd_json::Error),
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
