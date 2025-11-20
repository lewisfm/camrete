use std::{cmp::Ordering, fmt::Debug, num::ParseIntError, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct GameVersion(Option<u32>, Option<u32>, Option<u32>, Option<u32>);

impl GameVersion {
    pub const fn empty() -> Self {
        Self(None, None, None, None)
    }

    pub fn new(
        major: Option<u32>,
        minor: Option<u32>,
        patch: Option<u32>,
        build: Option<u32>,
    ) -> Self {
        Self(major, minor, patch, build)
    }

    pub fn major(&self) -> Option<u32> {
        self.0
    }

    pub fn minor(&self) -> Option<u32> {
        self.1
    }

    pub fn patch(&self) -> Option<u32> {
        self.2
    }

    pub fn build(&self) -> Option<u32> {
        self.3
    }

    pub fn is_empty(&self) -> bool {
        self == &GameVersion::empty()
    }
}

impl PartialEq for GameVersion {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1 && self.2 == other.2 && self.3 == other.3
    }
}

impl Eq for GameVersion {}

impl Ord for GameVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let major_eq = self.0.cmp(&other.0);
        if major_eq != Ordering::Equal {
            return major_eq;
        }

        let minor_eq = self.1.cmp(&other.1);
        if minor_eq != Ordering::Equal {
            return minor_eq;
        }

        let patch_eq = self.2.cmp(&other.2);
        if patch_eq != Ordering::Equal {
            return patch_eq;
        }

        let patch_eq = self.2.cmp(&other.2);
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

        version.0 = get_next()?;
        version.1 = get_next()?;
        version.2 = get_next()?;
        version.3 = get_next()?;

        if parts.next().is_some() {
            return Err(GameVersionParseError::TooManyParts);
        }

        Ok(version)
    }
}

impl Debug for GameVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = [self.0, self.1, self.2, self.3]
            .map(|part| {
                part.map(|i| i.to_string())
                    .unwrap_or_else(|| "x".to_string())
            })
            .join(".");

        write!(f, "v{string}")
    }
}
