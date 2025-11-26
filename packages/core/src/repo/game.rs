use std::{cmp::Ordering, fmt::Debug, num::ParseIntError, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Copy, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct GameVersion {
    major: Option<u32>,
    minor: Option<u32>,
    patch: Option<u32>,
    build: Option<u32>,
}

impl GameVersion {
    pub const fn empty() -> Self {
        Self {
            major: None,
            minor: None,
            patch: None,
            build: None,
        }
    }

    pub fn new(
        major: Option<u32>,
        minor: Option<u32>,
        patch: Option<u32>,
        build: Option<u32>,
    ) -> Self {
        Self {
            major,
            minor,
            patch,
            build,
        }
    }

    pub fn major(&self) -> Option<u32> {
        self.major
    }

    pub fn minor(&self) -> Option<u32> {
        self.minor
    }

    pub fn patch(&self) -> Option<u32> {
        self.patch
    }

    pub fn build(&self) -> Option<u32> {
        self.build
    }

    pub fn is_empty(&self) -> bool {
        self == &GameVersion::empty()
    }
}

impl PartialEq for GameVersion {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
            && self.build == other.build
    }
}

impl Eq for GameVersion {}

impl Ord for GameVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let major_eq = self.major.cmp(&other.major);
        if major_eq != Ordering::Equal {
            return major_eq;
        }

        let minor_eq = self.minor.cmp(&other.minor);
        if minor_eq != Ordering::Equal {
            return minor_eq;
        }

        let patch_eq = self.patch.cmp(&other.patch);
        if patch_eq != Ordering::Equal {
            return patch_eq;
        }

        let build_eq = self.build.cmp(&other.build);
        if build_eq != Ordering::Equal {
            return build_eq;
        }

        Ordering::Equal
    }
}

impl PartialOrd for GameVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Default for GameVersion {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum GameVersionParseError {
    #[error("Too many identifiers in game version")]
    TooManyParts,
    #[error("A version identifier was not a valid integer")]
    NotInteger(#[from] ParseIntError),
}

impl FromStr for GameVersion {
    type Err = GameVersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut version = Self::empty();

        if s == "any" {
            return Ok(version);
        }

        let mut parts = s.split('.');
        let mut get_next = || parts.next().map(|i| i.trim().parse::<u32>()).transpose();

        version.major = get_next()?;
        version.minor = get_next()?;
        version.patch = get_next()?;
        version.patch = get_next()?;

        if parts.next().is_some() {
            return Err(GameVersionParseError::TooManyParts);
        }

        Ok(version)
    }
}

impl Debug for GameVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = [self.major, self.minor, self.patch, self.build]
            .map(|part| {
                part.map(|i| i.to_string())
                    .unwrap_or_else(|| "x".to_string())
            })
            .join(".");

        write!(f, "v{string}")
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::repo::game::{GameVersion, GameVersionParseError};

    #[test]
    fn parse_any() {
        let v1 = GameVersion::from_str("any").unwrap();
        assert!(v1.is_empty());
    }

    #[test]
    fn not_parse_word() {
        let v1 = GameVersion::from_str("any version");
        assert!(matches!(v1, Err(GameVersionParseError::NotInteger(_))));

        let v2 = GameVersion::from_str("foobar");
        assert!(matches!(v2, Err(GameVersionParseError::NotInteger(_))));
    }

    #[test]
    fn parse_major_only() {
        let v1 = GameVersion::from_str("5").unwrap();
        assert_eq!(v1.major(), Some(5));
        assert_eq!(v1.minor(), None);
        assert_eq!(v1.patch(), None);
    }

    #[test]
    fn parse_major_minor() {
        let v1 = GameVersion::from_str("1.8").unwrap();
        assert_eq!(v1.major(), Some(1));
        assert_eq!(v1.minor(), Some(8));
        assert_eq!(v1.patch(), None);
    }

    #[test]
    fn parse_std() {
        let v1 = GameVersion::from_str("3.14.15").unwrap();
        assert_eq!(v1.major(), Some(3));
        assert_eq!(v1.minor(), Some(14));
        assert_eq!(v1.patch(), Some(15));
    }

    #[test]
    fn parse_build() {
        let v1 = GameVersion::from_str("0.0.0.15").unwrap();
        assert_eq!(v1.major(), Some(0));
        assert_eq!(v1.minor(), Some(0));
        assert_eq!(v1.patch(), Some(0));
        assert_eq!(v1.build(), Some(15));
    }

    #[test]
    fn not_parse_too_many() {
        let v1 = GameVersion::from_str("1.2.3.4.5");
        assert_eq!(v1, Err(GameVersionParseError::TooManyParts));
    }

    #[test]
    fn not_parse_letters() {
        let v1 = GameVersion::from_str("1.2.3b");
        assert!(matches!(v1, Err(GameVersionParseError::NotInteger(_))));
    }
}
