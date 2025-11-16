use diesel::{
    backend::Backend,
    deserialize::{FromSql, FromSqlRow},
    prelude::*,
    sql_types::Text,
};
use url::Url;

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::repositories)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Repository {
    #[diesel(deserialize_as = UrlString, serialize_as = String)]
    pub url: Url,
    pub name: String,
    pub priority: i32,
    pub x_mirror: bool,
    pub x_comment: Option<String>,
}

/// Wrapper struct for deserializing URLs from SQL Text rows.
#[derive(FromSqlRow)]
pub struct UrlString(String);

impl TryFrom<UrlString> for Url {
    type Error = url::ParseError;
    fn try_from(value: UrlString) -> Result<Self, Self::Error> {
        Url::parse(&value.0)
    }
}

impl<DB> FromSql<Text, DB> for UrlString
where
    DB: Backend,
    String: FromSql<Text, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        Ok(Self(String::from_sql(bytes)?))
    }
}
