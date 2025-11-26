//! Serializable spec version.
//!
//! It has a special case for v1.0, which is serialized as integer `1` instead
//! of string `"v1.0"` like other versions would be.

use std::fmt::{self, Formatter};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Unexpected, Visitor},
};

/// The version of a CKAN metadata file.
#[derive(Debug, PartialEq, Eq, uniffi::Record)]
pub struct SpecVersion {
    pub major: u16,
    pub minor: u16,
}

impl<'a> Deserialize<'a> for SpecVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a>,
    {
        struct Visit;

        impl Visitor<'_> for Visit {
            type Value = SpecVersion;

            fn expecting(&self, f: &mut Formatter) -> fmt::Result {
                write!(f, "spec version (\"vN.N\" or 1)")
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                if v == 1 {
                    Ok(SpecVersion {
                        major: v as u16,
                        minor: 0,
                    })
                } else {
                    Err(de::Error::invalid_value(Unexpected::Unsigned(v), &Visit))
                }
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let err = || de::Error::invalid_value(Unexpected::Str(v), &Visit);

                let trimmed = v.strip_prefix('v').ok_or_else(err)?;
                let (major, minor) = trimmed.split_once('.').ok_or_else(err)?;

                let major: u16 = major.parse().ok().ok_or_else(err)?;
                let minor: u16 = minor.parse().ok().ok_or_else(err)?;

                Ok(SpecVersion { major, minor })
            }
        }

        deserializer.deserialize_any(Visit)
    }
}

impl Serialize for SpecVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if matches!(self, SpecVersion { major: 1, minor: 0 }) {
            serializer.serialize_u64(1)
        } else {
            serializer.collect_str(&format_args!("v{}.{}", self.major, self.minor))
        }
    }
}

#[cfg(test)]
mod test {
    use serde_test::{Token, assert_tokens};

    use super::SpecVersion;

    #[test]
    fn ser_de_v1_special_case() {
        let v1 = SpecVersion { major: 1, minor: 0 };

        assert_tokens(&v1, &[Token::U64(1)]);
    }

    #[test]
    fn ser_de_v1_minor() {
        let v1 = SpecVersion { major: 1, minor: 1 };

        assert_tokens(&v1, &[Token::Str("v1.1")]);
    }

    #[test]
    fn ser_de_major_only() {
        let v1 = SpecVersion { major: 2, minor: 0 };

        assert_tokens(&v1, &[Token::Str("v2.0")]);
    }

    #[test]
    fn ser_de_major_minor() {
        let v1 = SpecVersion {
            major: 5,
            minor: 12,
        };

        assert_tokens(&v1, &[Token::Str("v5.12")]);
    }
}
