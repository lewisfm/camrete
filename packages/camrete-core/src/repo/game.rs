use std::{cmp::Ordering, fmt::Debug, num::ParseIntError, str::FromStr, sync::Arc};

use thiserror::Error;

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

        let patch_eq = self.patch.cmp(&other.patch);
        if patch_eq != Ordering::Equal {
            return patch_eq;
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

#[derive(Debug, Error)]
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
        version.build = get_next()?;

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
