//! Serialize and deserialize game versions as strings.
//!
//! This module contains a wrapper for GameVersion which serializes and deserializes
//! to a string instead of an encoded struct, for inclusion in JSON dumps of releases.
//!
//! Game versions are (de)serialized in the format `1[.2][.3][.4]`, in order from major
//! to build version numbers. Empty game versions are serialized as nulls.
//!
//! When deserializing, a string with the value "any" is parsed the same as a null.

use std::{
    fmt::{self, Formatter, Write},
    str::FromStr,
};

use derive_more::{Deref, From, Into};
use serde::{
    Deserialize, Serialize,
    de::{self, Unexpected, Visitor},
};

use crate::repo::game::GameVersion;

/// A wrapper for GameVersion which serializes as a string.
#[derive(Debug, Copy, Clone, Deref, From, Into, PartialEq, Eq, Default)]
pub struct MetaGameVersion(pub GameVersion);

uniffi::custom_newtype!(MetaGameVersion, GameVersion);

impl Serialize for MetaGameVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let Some(major) = self.0.major() else {
            return serializer.serialize_none();
        };
        let mut string = major.to_string();

        let Some(minor) = self.0.minor() else {
            return serializer.serialize_str(&string);
        };
        write!(string, ".{minor}").unwrap();

        let Some(patch) = self.0.patch() else {
            return serializer.serialize_str(&string);
        };
        write!(string, ".{patch}").unwrap();

        let Some(build) = self.0.build() else {
            return serializer.serialize_str(&string);
        };
        write!(string, ".{build}").unwrap();

        serializer.serialize_str(&string)
    }
}

impl<'a> Deserialize<'a> for MetaGameVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        struct Visit;

        impl Visitor<'_> for Visit {
            type Value = MetaGameVersion;

            fn expecting(&self, f: &mut Formatter) -> fmt::Result {
                write!(f, "string \"any\" or null or \"N[.N[.N]]\"")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(MetaGameVersion::default())
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let parsed = GameVersion::from_str(v)
                    .map_err(|_| de::Error::invalid_value(Unexpected::Str(v), &Visit))?;

                Ok(parsed.into())
            }
        }

        deserializer.deserialize_str(Visit)
    }
}

#[cfg(test)]
mod test {
    use serde_test::{Token, assert_de_tokens, assert_tokens};

    use super::MetaGameVersion;
    use crate::repo::game::GameVersion;

    #[test]
    fn de_any() {
        let val = MetaGameVersion::default();

        assert_de_tokens(&val, &[Token::Str("any")]);
    }

    #[test]
    fn ser_de_major_only() {
        let val = MetaGameVersion(GameVersion::new(Some(1), None, None, None));

        assert_tokens(&val, &[Token::Str("1")]);
    }

    #[test]
    fn ser_de_major_minor() {
        let val = MetaGameVersion(GameVersion::new(Some(1), Some(2), None, None));

        assert_tokens(&val, &[Token::Str("1.2")]);
    }

    #[test]
    fn ser_de_major_minor_patch() {
        let val = MetaGameVersion(GameVersion::new(Some(1), Some(2), Some(3), None));

        assert_tokens(&val, &[Token::Str("1.2.3")]);
    }

    #[test]
    fn ser_de_none() {
        let val = MetaGameVersion::default();

        assert_tokens(&val, &[Token::None]);
    }
}
