use std::{
    borrow::{Borrow, Cow},
    cmp::Ordering,
    fmt::{Display, Formatter},
};

use diesel::{Queryable, backend::Backend, deserialize::FromSql, expression::AsExpression, sql_types::Text};

#[derive(Debug, Clone, PartialEq, Eq, AsExpression)]
#[diesel(sql_type = Text)]
pub struct ModuleVersion<'a> {
    epoch: Option<u32>,
    mod_version_start: usize,
    string: Cow<'a, str>,
}

impl<'a> ModuleVersion<'a> {
    pub fn epoch(&self) -> Option<u32> {
        self.epoch
    }

    pub fn mod_version(&self) -> &str {
        &self.string[self.mod_version_start..]
    }

    pub fn as_str(&self) -> &str {
        &self.string
    }

    pub fn into_inner(self) -> Cow<'a, str> {
        self.string
    }
}

impl<'a> From<Cow<'a, str>> for ModuleVersion<'a> {
    fn from(value: Cow<'a, str>) -> Self {
        if let Some(colon_idx) = value.find(':')
            && let Ok(epoch) = value[..colon_idx].parse()
        {
            return Self {
                epoch: Some(epoch),
                mod_version_start: colon_idx + 1,
                string: value,
            };
        }

        Self {
            epoch: None,
            mod_version_start: 0,
            string: value,
        }
    }
}

impl<'a> From<&'a str> for ModuleVersion<'a> {
    fn from(value: &'a str) -> Self {
        Self::from(Cow::Borrowed(value))
    }
}

impl From<String> for ModuleVersion<'static> {
    fn from(value: String) -> Self {
        Self::from(Cow::Owned(value))
    }
}

impl Display for ModuleVersion<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.string)
    }
}

impl<DB> Queryable<Text, DB> for ModuleVersion<'_>
where
    DB: Backend,
    String: FromSql<Text, DB>,
{
    type Row = String;
    fn build(version: String) -> diesel::deserialize::Result<Self> {
        Ok(version.into())
    }
}

impl PartialOrd for ModuleVersion<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ModuleVersion<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        let epoch = self.epoch.cmp(&other.epoch);
        if !epoch.is_eq() {
            return epoch;
        }

        let mut left = self.mod_version();
        let mut right = other.mod_version();

        if left == right {
            return Ordering::Equal;
        }

        // Split into pairs of strings and digits, then use a numerically-aware comparison for each.
        // e.g. 1.2.3a -> "", 1, ".", 2, ".", 3, "a", 0
        //      1.2.4b -> "", 1, ".", 2, ".", 4, "b", 0
        // 4 > 3 so the second one is larger.

        while !left.is_empty() && !right.is_empty() {
            let cmp = str_cmp(&mut left, &mut right);
            if !cmp.is_eq() {
                return cmp;
            }

            let cmp = num_cmp(&mut left, &mut right);
            if !cmp.is_eq() {
                return cmp;
            }
        }

        left.cmp(&right)
    }
}

/// Removes the non-digit prefix from the parameters, then compares those prefixes.
fn str_cmp(left: &mut &str, right: &mut &str) -> Ordering {
    // Start by removing the prefix: for `.abc-123.4`, `.abc-` is removed & compared, leaving `123.4`.

    let left_prefix = take_prefix(left, |c| c.is_ascii_digit());
    let right_prefix = take_prefix(right, |c| c.is_ascii_digit());

    // Override a leading dot to have a high priority, e.g. `.abc` > `abc`

    let left_is_dot = left_prefix.chars().next() == Some('.');
    let right_is_dot = right_prefix.chars().next() == Some('.');

    if left_is_dot || right_is_dot {
        let dot_cmp = left_is_dot.cmp(&right_is_dot);
        if !dot_cmp.is_eq() {
            return dot_cmp;
        }
    }

    // Compare lexicographically

    left_prefix.cmp(&right_prefix)
}

/// Removes the digit-only prefix from the parameters, then compares those prefixes.
fn num_cmp(left: &mut &str, right: &mut &str) -> Ordering {
    // Start by removing the prefix: for `4-beta.1`, `4` is removed and compared, leaving `-beta.1`.

    let left_prefix = take_prefix(left, |c| !c.is_ascii_digit());
    let right_prefix = take_prefix(right, |c| !c.is_ascii_digit());

    let left_num = left_prefix.parse().unwrap_or(0);
    let right_num = right_prefix.parse().unwrap_or(0);

    left_num.cmp(&right_num)
}

fn take_prefix<'a>(buf: &mut &'a str, pat: impl FnMut(char) -> bool) -> &'a str {
    if let Some((prefix, rest)) = buf.split_once(pat) {
        *buf = rest;
        return prefix;
    }

    return "";
}
