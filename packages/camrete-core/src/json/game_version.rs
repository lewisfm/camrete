use serde::{
    Deserialize, Serialize,
    de::{self, Unexpected, Visitor},
};
use std::fmt::{self, Formatter, Write};

#[derive(Debug, PartialEq, Eq, Default)]
pub enum GameVersionSpec {
    #[default]
    Any,
    Named {
        major: u16,
        minor: Option<u16>,
        patch: Option<u16>,
    },
}

impl GameVersionSpec {
    pub fn is_any(&self) -> bool {
        matches!(self, Self::Any)
    }

    pub fn major(&self) -> Option<u16> {
        if let &Self::Named { major, .. } = self {
            Some(major)
        } else {
            None
        }
    }

    pub fn minor(&self) -> Option<u16> {
        if let &Self::Named { minor, .. } = self {
            minor
        } else {
            None
        }
    }

    pub fn patch(&self) -> Option<u16> {
        if let &Self::Named { patch, .. } = self {
            patch
        } else {
            None
        }
    }
}

impl Serialize for GameVersionSpec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            GameVersionSpec::Any => serializer.serialize_none(),
            &GameVersionSpec::Named {
                major,
                minor,
                patch,
            } => {
                let mut string = major.to_string();

                if let Some(minor) = minor {
                    write!(string, ".{minor}").unwrap();

                    if let Some(patch) = patch {
                        write!(string, ".{patch}").unwrap();
                    }
                }

                serializer.serialize_str(&string)
            }
        }
    }
}

impl<'a> Deserialize<'a> for GameVersionSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        struct Visit;

        impl Visitor<'_> for Visit {
            type Value = GameVersionSpec;

            fn expecting(&self, f: &mut Formatter) -> fmt::Result {
                write!(f, "string \"any\" or null or \"N[.N[.N]]\"")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
                where
                    E: de::Error, {
                Ok(GameVersionSpec::Any)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v == "any" {
                    return Ok(GameVersionSpec::Any);
                }

                let mut parts = v.split('.').map(|part| {
                    part.parse()
                        .map_err(|_| de::Error::invalid_value(Unexpected::Str(v), &Visit))
                });

                let major = parts.next().unwrap()?;
                let minor = parts.next().transpose()?;
                let patch = parts.next().transpose()?;

                Ok(GameVersionSpec::Named {
                    major,
                    minor,
                    patch,
                })
            }
        }

        deserializer.deserialize_str(Visit)
    }
}

#[cfg(test)]
mod test {
    use serde_test::{assert_de_tokens, assert_tokens, Token};

    use super::GameVersionSpec;

    #[test]
    fn de_any() {
        let val = GameVersionSpec::Any;

        assert_de_tokens(&val, &[Token::Str("any")]);
    }

    #[test]
    fn ser_de_major_only() {
        let val = GameVersionSpec::Named {
            major: 1,
            minor: None,
            patch: None,
        };

        assert_tokens(&val, &[Token::Str("1")]);
    }

    #[test]
    fn ser_de_major_minor() {
        let val = GameVersionSpec::Named {
            major: 1,
            minor: Some(2),
            patch: None,
        };

        assert_tokens(&val, &[Token::Str("1.2")]);
    }

    #[test]
    fn ser_de_major_minor_patch() {
        let val = GameVersionSpec::Named {
            major: 1,
            minor: Some(2),
            patch: Some(3),
        };

        assert_tokens(&val, &[Token::Str("1.2.3")]);
    }

    #[test]
    fn ser_de_none() {
        let val = GameVersionSpec::Any;

        assert_tokens(&val, &[Token::None]);
    }
}
