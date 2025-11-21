use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::{Display, Formatter},
};

use diesel::{
    Queryable, backend::Backend, deserialize::FromSql, expression::AsExpression, sql_types::Text,
};

#[derive(Debug, Clone, Eq, AsExpression, Hash)]
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

impl PartialEq for ModuleVersion<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl PartialOrd for ModuleVersion<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ModuleVersion<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        let epoch = self
            .epoch
            .unwrap_or_default()
            .cmp(&other.epoch.unwrap_or_default());
        if !epoch.is_eq() {
            return epoch;
        }

        let mut left = self.mod_version();
        let mut right = other.mod_version();

        if left == right {
            return Ordering::Equal;
        }

        // Split into pairs of strings and digits, then use a numerically-aware
        // comparison for each. e.g. 1.2.3a -> "", 1, ".", 2, ".", 3, "a", 0
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

        left.cmp(right)
    }
}

/// Removes the non-digit prefix from the parameters, then compares those
/// prefixes.
fn str_cmp(left: &mut &str, right: &mut &str) -> Ordering {
    // Start by removing the prefix: for `.abc-123.4`, `.abc-` is removed &
    // compared, leaving `123.4`.

    let left_prefix = take_prefix(left, |c| !c.is_ascii_digit());
    let right_prefix = take_prefix(right, |c| !c.is_ascii_digit());

    // This is a special case for dots that may represent a new subversion.

    let left_is_dot = left_prefix.starts_with('.');
    let right_is_dot = right_prefix.starts_with('.');

    if left_is_dot || right_is_dot {
        // If one side is adding another subversion but the other is just metadata,
        // then the side with the subversion wins.
        // e.g. `1[.]4` > `1[-beta]`
        let dot_cmp = left_is_dot.cmp(&right_is_dot);
        if !dot_cmp.is_eq() {
            return dot_cmp;
        }

        // If one of the dots is bare (maybe there is a number after), sort it
        // larger than the one with metadata.
        // e.g. `1[.]10` > `1[.beta]`

        if left_prefix.len() == 1 && right_prefix.len() > 1 {
            return Ordering::Greater; // `.` > `.x`
        }

        if left_prefix.len() > 1 && right_prefix.len() == 1 {
            return Ordering::Less; // `.x` < `.`
        }

        // Fall through to normal compare, e.g. `.beta` > `.alpha`
    }


    // Compare lexicographically.
    // e.g. `-beta.` > `-alpha.`

    left_prefix.cmp(right_prefix)
}

/// Removes the digit-only prefix from the parameters, then compares those
/// prefixes.
fn num_cmp(left: &mut &str, right: &mut &str) -> Ordering {
    // Start by removing the prefix: for `4-beta.1`, `4` is removed and compared,
    // leaving `-beta.1`.

    let left_prefix = take_prefix(left, |c| c.is_ascii_digit());
    let right_prefix = take_prefix(right, |c| c.is_ascii_digit());

    let left_num = left_prefix.parse().unwrap_or(0);
    let right_num = right_prefix.parse().unwrap_or(0);

    left_num.cmp(&right_num)
}

/// Returns the prefix of characters for which the given test function evaluates true.
fn take_prefix<'a>(buf: &mut &'a str, mut is_prefix: impl FnMut(char) -> bool) -> &'a str {
    let split_point = buf.find(|c| !is_prefix(c)).unwrap_or(buf.len());
    let (prefix, rest) = buf.split_at(split_point);
    *buf = rest;
    prefix
}

#[cfg(test)]
mod test {
    // Version test cases are mostly sourced from CKAN-core here.
    // https://github.com/KSP-CKAN/CKAN/blob/master/Tests/Core/Versioning/ModuleVersionTests.cs

    use super::*;

    #[test]
    fn different_epoch() {
        let v1 = ModuleVersion::from("1:alpha");
        let v2 = ModuleVersion::from("banana");
        assert!(v1 > v2);

        let v1 = ModuleVersion::from("0:alpha");
        let v2 = ModuleVersion::from("banana");
        assert!(v1 < v2);

        let v1 = ModuleVersion::from("3:alpha");
        let v2 = ModuleVersion::from("2:banana");
        assert!(v1 > v2);
    }

    #[test]
    fn alpha() {
        let v1 = ModuleVersion::from("alpha");
        let v2 = ModuleVersion::from("banana");

        assert!(v1 < v2);
    }

    #[test]
    fn basic() {
        let v0 = ModuleVersion::from("1.2.0");
        let v1 = ModuleVersion::from("1.2.0");
        let v2 = ModuleVersion::from("1.2.2");

        assert_eq!(v0, v1);
        assert!(v1 < v2);
        assert!(v2 > v1);
    }

    #[test]
    fn logical_equality() {
        // Should be aware of zero padding.
        let v1 = ModuleVersion::from("1.1");
        let v2 = ModuleVersion::from("1.01");

        assert_eq!(v1, v2);
    }

    #[test]
    fn dot_has_sort_priority() {
        let v1 = ModuleVersion::from("1.0-beta");
        let v2 = ModuleVersion::from("1.0.1-beta");

        assert!(v2 > v1);

        let v1 = ModuleVersion::from("1.0_beta");
        let v2 = ModuleVersion::from("1.0.1_beta");

        assert!(v2 > v1);
    }

    #[test]
    fn dot_for_extra_data() {
        let v1 = ModuleVersion::from("1.0");
        let v2 = ModuleVersion::from("1.0.repackaged");
        let v3 = ModuleVersion::from("1.0.1");

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v2 > v1);
        assert!(v3 > v2);
    }

    #[test]
    fn subversion_over_metadata() {
        let v1 = ModuleVersion::from("1.4");
        let v2 = ModuleVersion::from("1.beta");
        assert!(v1 > v2);
    }

    #[test]
    fn dot_segments_compare_lexicographically() {
        let v1 = ModuleVersion::from("1.alpha");
        let v2 = ModuleVersion::from("1.beta");
        assert!(v1 < v2);
    }

    #[test]
    fn uneven_versioning() {
        let v1 = ModuleVersion::from("1.1.0.0");
        let v2 = ModuleVersion::from("1.1.1");
        assert!(v1 < v2);
        assert!(v2 > v1);
    }

    #[test]
    fn complex() {
        let v1 = ModuleVersion::from("v6a12");
        let v2 = ModuleVersion::from("v6a5");
        assert!(v1 > v2);
        assert!(v2 < v1);
    }

    #[test]
    fn take_prefix_letters() {
        let mut string = "abc123";
        let prefix = take_prefix(&mut string, |c| c.is_alphabetic());

        assert_eq!(prefix, "abc");
        assert_eq!(string, "123");
    }

    #[test]
    fn take_prefix_empty() {
        let mut string = "456";
        let prefix = take_prefix(&mut string, |c| c.is_alphabetic());

        assert_eq!(prefix, "");
        assert_eq!(string, "456");
    }

    #[test]
    fn take_prefix_all() {
        let mut string = "hello";
        let prefix = take_prefix(&mut string, |c| c.is_alphabetic());

        assert_eq!(prefix, "hello");
        assert_eq!(string, "");
    }

    #[test]
    fn str_cmp_simple() {
        let mut left = "alpha123";
        let mut right = "beta123";

        let cmp = str_cmp(&mut left, &mut right);

        assert_eq!(cmp, Ordering::Less);
        assert_eq!(left, "123");
        assert_eq!(right, "123");
    }

    #[test]
    fn str_cmp_different_suffix() {
        let mut left = "zeta5";
        let mut right = "ernest10";

        let cmp = str_cmp(&mut left, &mut right);

        assert_eq!(cmp, Ordering::Greater);
        assert_eq!(left, "5");
        assert_eq!(right, "10");
    }

    #[test]
    fn str_cmp_same_prefix() {
        let mut left = "kappa9";
        let mut right = "kappa15";

        let cmp = str_cmp(&mut left, &mut right);

        assert_eq!(cmp, Ordering::Equal);
        assert_eq!(left, "9");
        assert_eq!(right, "15");
    }

    #[test]
    fn str_cmp_no_prefix() {
        let mut left = "9kip";
        let mut right = "15omega";

        let cmp = str_cmp(&mut left, &mut right);

        assert_eq!(cmp, Ordering::Equal);
        assert_eq!(left, "9kip");
        assert_eq!(right, "15omega");
    }

    #[test]
    fn num_cmp_simple() {
        let mut left = "123alpha";
        let mut right = "124alpha";

        let cmp = num_cmp(&mut left, &mut right);

        assert_eq!(cmp, Ordering::Less);
        assert_eq!(left, "alpha");
        assert_eq!(right, "alpha");
    }

    #[test]
    fn num_cmp_different_suffix() {
        let mut left = "10zeta";
        let mut right = "5ernest";

        let cmp = num_cmp(&mut left, &mut right);

        assert_eq!(cmp, Ordering::Greater);
        assert_eq!(left, "zeta");
        assert_eq!(right, "ernest");
    }

    #[test]
    fn num_cmp_same_prefix() {
        let mut left = "9kappa";
        let mut right = "9ernest";

        let cmp = num_cmp(&mut left, &mut right);

        assert_eq!(cmp, Ordering::Equal);
        assert_eq!(left, "kappa");
        assert_eq!(right, "ernest");
    }

    #[test]
    fn num_cmp_no_prefix() {
        let mut left = "kip";
        let mut right = "omega15";

        let cmp = num_cmp(&mut left, &mut right);

        assert_eq!(cmp, Ordering::Equal);
        assert_eq!(left, "kip");
        assert_eq!(right, "omega15");
    }
}
