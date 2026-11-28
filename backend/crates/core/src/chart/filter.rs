//! The run filter of a source (FR-11, ADR 0022, "Sources").
//!
//! The server's `RunFilter` (`service::runs`) cannot live in this crate, so
//! a source keeps its filter as JSON fields, and this table says which
//! fields it accepts and what they take ([`check`] for the values beyond
//! the type). The server turns a bound filter into its `RunFilter`; a test
//! there checks that it takes every field and value this module accepts.
//! Paging (`limit`, `cursor`) is not part of a source.

use serde_json::Value;

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

/// The values of `status`.
pub const STATUSES: &[&str] = &["running", "completed", "failed", "aborted"];
/// The values of `triggered_by`.
pub const TRIGGERS: &[&str] = &["nightly", "manual", "ci"];
/// The values of `validity`.
pub const VALIDITIES: &[&str] = &["valid", "invalid", "all"];

/// What the filter field `name` takes, or `None` if there is no such field.
pub fn field(name: &str) -> Option<Expect> {
    FIELDS
        .iter()
        .find(|(n, ..)| *n == name)
        .map(|(n, ..)| Expect::Field(n))
}

/// The type of the filter field `name`; text for a name the table does not
/// have, which [`field`] never gives.
pub fn type_of(name: &str) -> Expect {
    FIELDS
        .iter()
        .find(|(n, ..)| *n == name)
        .map_or(Expect::one(Text), |(_, ty, list)| Expect::Type {
            ty: *ty,
            list: *list,
        })
}

/// Checks what the server's run filter takes beyond the type of a field:
/// whole numbers of at least 1, run ids, parameters as `name=value`, and
/// the values of enumerations.
/// The value has the field's type already.
pub fn check(name: &str, value: &Value) -> Result<(), String> {
    let one_of = |allowed: &[&str], v: &Value| match v.as_str() {
        Some(s) if allowed.contains(&s) => Ok(()),
        _ => Err(format!("{v} is not one of {}", allowed.join(", "))),
    };
    let items = || value.as_array().map(Vec::as_slice).unwrap_or_default();
    match name {
        "started_within_days" | "latest_runs" => match value.as_i64() {
            Some(n) if n >= 1 => Ok(()),
            _ => Err(format!("{value} is not a whole number of at least 1")),
        },
        "shared_environment_id" => match value.as_i64() {
            Some(_) => Ok(()),
            None => Err(format!("{value} is not an id: a whole number")),
        },
        "run_id" => items().iter().try_for_each(|v| match v.as_str() {
            Some(s) if is_uuid(s) => Ok(()),
            _ => Err(format!("{v} is not a run id (a UUID)")),
        }),
        "status" => items().iter().try_for_each(|v| one_of(STATUSES, v)),
        "param" => items().iter().try_for_each(|v| match v.as_str() {
            Some(s) if s.split_once('=').is_some_and(|(n, _)| !n.trim().is_empty()) => Ok(()),
            _ => Err(format!("{v} is not a parameter condition: name=value")),
        }),
        "triggered_by" => one_of(TRIGGERS, value),
        "validity" => one_of(VALIDITIES, value),
        _ => Ok(()),
    }
}

/// True for a UUID in its hyphenated form.
fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::chart::variables::fits;

    #[test]
    fn fields_have_types_and_paging_is_not_one() {
        assert_eq!(field("sut_branch"), Some(Expect::Field("sut_branch")));
        assert_eq!(type_of("sut_branch"), Expect::one(Text));
        assert_eq!(type_of("status"), Expect::list(Text));
        assert_eq!(type_of("started_after"), Expect::one(Time));
        assert_eq!(field("limit"), None);
        assert_eq!(field("cursor"), None);
    }

    #[test]
    fn values_must_be_ones_the_server_takes() {
        let ok = |name: &str, v: Value| fits(field(name).unwrap(), &v).is_ok();
        assert!(ok("latest_runs", json!(5)));
        assert!(!ok("latest_runs", json!(1.5)));
        assert!(!ok("latest_runs", json!(0)));
        assert!(!ok("started_within_days", json!(-3)));
        assert!(ok("shared_environment_id", json!(7)));
        assert!(!ok("shared_environment_id", json!(7.5)));
        assert!(ok(
            "run_id",
            json!(["7c6c5a3e-0c4b-4d4e-9c39-6f1e6c3b8a10"])
        ));
        assert!(!ok("run_id", json!(["not-a-uuid"])));
        assert!(ok("param", json!(["codec=h264", "size_mb=10"])));
        assert!(!ok("param", json!(["codec"])));
        assert!(!ok("param", json!([" =h264"])));
        assert!(!ok(
            "run_id",
            json!(["7c6c5a3e-0c4b-4d4e-9c39-6f1e6c3b8a1z"])
        ));
        assert!(ok("status", json!(["completed", "aborted"])));
        assert!(!ok("status", json!(["unknown"])));
        assert!(ok("triggered_by", json!("ci")));
        assert!(!ok("triggered_by", json!("cron")));
        assert!(ok("validity", json!("all")));
        assert!(!ok("validity", json!("both")));
        assert!(ok("sut_branch", json!("main")));
        assert!(!ok("sut_branch", json!(5)));
    }
}
