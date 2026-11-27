//! The run filter of a source (FR-11, ADR 0022, "Sources").
//!
//! The server's `RunFilter` (`service::runs`) cannot live in this crate, so
//! a source keeps its filter as JSON fields, and this table says which
//! fields it accepts and what they take. The server turns a bound filter
//! into its `RunFilter`; a test there checks that it knows every field of
//! this table. Paging (`limit`, `cursor`) is not part of a source.

use super::variables::{Expect, VarType};

use VarType::{Number, Text, Time};

/// The fields of a source's run filter: name, type, and whether the field
/// takes a list.
pub const FIELDS: &[(&str, VarType, bool)] = &[
    ("run_id", Text, true),
    ("suite", Text, false),
    ("sut_commit", Text, false),
    ("sut_branch", Text, false),
    ("sut_tag", Text, false),
    ("sut_build_config", Text, false),
    ("started_after", Time, false),
    ("started_before", Time, false),
    ("started_within_days", Number, false),
    ("latest_runs", Number, false),
    ("since_tag", Text, false),
    ("since_sut_tag", Text, false),
    ("status", Text, true),
    ("triggered_by", Text, false),
    ("shared_environment_id", Number, false),
    ("client_device", Text, false),
    ("scenario", Text, false),
    ("asset", Text, false),
    ("param", Text, true),
    ("metric", Text, false),
    ("tag", Text, true),
    ("validity", Text, false),
];

/// What the filter field `name` takes, or `None` if there is no such field.
pub fn field(name: &str) -> Option<Expect> {
    FIELDS
        .iter()
        .find(|(n, ..)| *n == name)
        .map(|(_, ty, list)| Expect::Type {
            ty: *ty,
            list: *list,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_have_types_and_paging_is_not_one() {
        assert_eq!(field("sut_branch"), Some(Expect::one(Text)));
        assert_eq!(field("status"), Some(Expect::list(Text)));
        assert_eq!(field("started_after"), Some(Expect::one(Time)));
        assert_eq!(field("limit"), None);
        assert_eq!(field("cursor"), None);
    }
}
