use std::{
    borrow::Cow,
    fmt::{Debug, Formatter},
    marker::PhantomData,
};

use diesel::{
    Queryable,
    backend::Backend,
    deserialize::{FromSql, FromSqlRow},
    expression::AsExpression,
    serialize::{IsNull, Output, ToSql},
    sql_types::{Binary, Integer, Jsonb, Nullable},
};
use serde_json::Value;
use simd_json::{from_value, to_value};
use url::Url;

use crate::{database::models, repo::game::GameVersion};

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

mod id {
    macro_rules! tag {
        ($($name:ident),*) => {
            $(
            #[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $name;
            )*
        };
    }

    tag!(Repo, Module, Release, DepGroup);
}

pub type RepoId = Id<id::Repo>;
pub type ModuleId = Id<id::Module>;
pub type ReleaseId = Id<id::Release>;
pub type DepGroupId = Id<id::DepGroup>;

#[derive(Debug, FromSqlRow, AsExpression)]
#[diesel(sql_type = Binary)]
pub struct JsonbValue(pub Value);

// These traits are for converting this helper struct to serialized data for SQL.

// BLOB NOT NULL -> Self

impl<DB> FromSql<Binary, DB> for JsonbValue
where
    DB: Backend,
    Value: FromSql<Jsonb, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        Value::from_sql(bytes).map(Self)
    }
}

// BLOB -> Self

impl<DB> FromSql<Nullable<Binary>, DB> for JsonbValue
where
    DB: Backend,
    Value: FromSql<Jsonb, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        Value::from_sql(bytes).map(Self)
    }

    fn from_nullable_sql(
        bytes: Option<<DB as Backend>::RawValue<'_>>,
    ) -> diesel::deserialize::Result<Self> {
        if let Some(bytes) = bytes {
            Value::from_sql(bytes).map(Self)
        } else {
            Ok(Self(Value::Null))
        }
    }
}

// Self -> BLOB (NOT NULL)

impl<DB> ToSql<Binary, DB> for JsonbValue
where
    DB: Backend,
    Value: ToSql<Jsonb, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> diesel::serialize::Result {
        if self.0.is_null() {
            Ok(IsNull::Yes)
        } else {
            self.0.to_sql(out)
        }
    }
}

// These traits are for converting this helper struct to and from strongly typed data.
// Other types <-> Self

macro_rules! jsonb_convertable {
    ($($type:ty),+) => {
        $(
            impl From<$type> for JsonbValue {
                fn from(value: $type) -> Self {
                    (&value).into()
                }
            }

            impl From<&$type> for JsonbValue {
                fn from(value: &$type) -> Self {
                    Self(to_value(value).expect("failed to serialize value to json"))
                }
            }

            impl From<Option<&$type>> for JsonbValue {
                fn from(value: Option<&$type>) -> Self {
                    Self(to_value(value).expect("failed to serialize value to json"))
                }
            }

            impl TryFrom<JsonbValue> for $type {
                type Error = serde_json::Error;

                fn try_from(value: JsonbValue) -> Result<Self, Self::Error> {
                    from_value(value.0)
                }
            }

            impl TryFrom<JsonbValue> for Option<$type> {
                type Error = serde_json::Error;

                fn try_from(value: JsonbValue) -> Result<Self, Self::Error> {
                    from_value(value.0)
                }
            }
        )+
    };
}

impl From<&Url> for JsonbValue {
    fn from(value: &Url) -> Self {
        Self(to_value(value).expect("failed to serialize value to json"))
    }
}
impl TryFrom<JsonbValue> for Url {
    type Error = serde_json::Error;
    fn try_from(value: JsonbValue) -> Result<Self, Self::Error> {
        from_value(value.0)
    }
}

jsonb_convertable!(models::ReleaseMetadata<'_>, GameVersion);

// Support for Self <-> Cow<Other types>

impl<'a, T: ToOwned> From<Cow<'a, T>> for JsonbValue
where
    Self: for<'any> From<&'any T>,
{
    fn from(value: Cow<'a, T>) -> Self {
        value.as_ref().into()
    }
}

impl<'a, T: ToOwned> From<Option<Cow<'a, T>>> for JsonbValue
where
    Self: for<'any> From<Option<&'any T>>,
{
    fn from(value: Option<Cow<'a, T>>) -> Self {
        value.as_deref().into()
    }
}

impl<T: Clone> TryFrom<JsonbValue> for Cow<'static, T>
where
    T: TryFrom<JsonbValue>,
{
    type Error = T::Error;

    fn try_from(value: JsonbValue) -> Result<Self, Self::Error> {
        let t = T::try_from(value)?;
        Ok(Cow::Owned(t))
    }
}

impl<T: Clone> TryFrom<JsonbValue> for Option<Cow<'static, T>>
where
    Option<T>: TryFrom<JsonbValue>,
{
    type Error = <Option<T> as TryFrom<JsonbValue>>::Error;

    fn try_from(value: JsonbValue) -> Result<Self, Self::Error> {
        let t = <Option<T>>::try_from(value)?;
        Ok(t.map(Cow::Owned))
    }
}
