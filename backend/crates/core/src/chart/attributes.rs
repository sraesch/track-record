//! Attributes: the dimensions of the domain model that frames and points
//! carry, under fixed names (ADR 0022, "Attributes").

use serde::Serialize;

/// The family of an attribute. A family name stands for all its attributes
/// in a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Run,
    Case,
    Device,
    Sample,
    Source,
    /// `attr.<name>`, added by a derive node. Not a family name in keys.
    Derived,
}

impl Family {
    /// The families that can stand for their attributes in a key.
    pub const KEYS: [Family; 5] = [
        Family::Run,
        Family::Case,
        Family::Device,
        Family::Sample,
        Family::Source,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Family::Run => "run",
            Family::Case => "case",
            Family::Device => "device",
            Family::Sample => "sample",
            Family::Source => "source",
            Family::Derived => "derived",
        }
    }

    /// The family a key names, e.g. `case`.
    pub fn from_key(name: &str) -> Option<Family> {
        Family::KEYS.into_iter().find(|f| f.as_str() == name)
    }
}

/// The type of an attribute's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttrType {
    Number,
    Time,
    Text,
    Bool,
    /// A list of texts (`run.tags`, `sut.tag`).
    List,
    /// The JSON type of the value: number, text or bool (parameters).
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttrInfo {
    pub family: Family,
    pub ty: AttrType,
}

use AttrType as T;
use Family as F;

/// The attributes with a fixed name.
pub const FIXED: &[(&str, Family, AttrType)] = &[
    ("run.id", F::Run, T::Text),
    ("run.started_at", F::Run, T::Time),
    ("run.ended_at", F::Run, T::Time),
    ("run.status", F::Run, T::Text),
    ("run.trigger", F::Run, T::Text),
    ("run.valid", F::Run, T::Bool),
    ("run.tags", F::Run, T::List),
    ("suite.name", F::Run, T::Text),
    ("suite.version", F::Run, T::Number),
    ("sut.id", F::Run, T::Number),
    ("sut.commit", F::Run, T::Text),
    ("sut.branch", F::Run, T::Text),
    ("sut.build_config", F::Run, T::Text),
    ("sut.tag", F::Run, T::List),
    ("environment.id", F::Run, T::Number),
    ("scenario.name", F::Case, T::Text),
    ("scenario.version", F::Case, T::Number),
    ("device.name", F::Device, T::Text),
    ("device.details", F::Device, T::Number),
    ("case_execution.id", F::Sample, T::Text),
    ("sample.id", F::Sample, T::Text),
    ("sample.repetition", F::Sample, T::Number),
    ("sample.warmup", F::Sample, T::Bool),
    ("source", F::Source, T::Text),
    ("metric", F::Source, T::Text),
];

/// The attributes whose names come from the data or the graph:
/// `<prefix><name>`.
pub const PREFIXED: &[(&str, Family, AttrType)] = &[
    ("asset.", F::Case, T::Text),
    ("param.", F::Case, T::Any),
    ("attr.", F::Derived, T::Number),
];

/// Family and type of the attribute `name`, or `None` if there is no such
/// attribute. Names after a prefix (`param.width`) are not checked: they
/// come from the data.
pub fn lookup(name: &str) -> Option<AttrInfo> {
    if let Some((_, family, ty)) = FIXED.iter().find(|(n, ..)| *n == name) {
        return Some(AttrInfo {
            family: *family,
            ty: *ty,
        });
    }
    PREFIXED
        .iter()
        .find(|(p, ..)| name.len() > p.len() && name.starts_with(p))
        .map(|(_, family, ty)| AttrInfo {
            family: *family,
            ty: *ty,
        })
}

/// True if attribute names start with `name` and a dot (`run`, `sut`,
/// `param`, ...). Match inputs cannot have such names: after a combine,
/// `b.sut.id` has to say which input it comes from.
pub fn is_prefix(name: &str) -> bool {
    FIXED
        .iter()
        .any(|(n, ..)| n.split_once('.').is_some_and(|(p, _)| p == name))
        || PREFIXED
            .iter()
            .any(|(p, ..)| p.strip_suffix('.') == Some(name))
}

/// The family of the attribute `name`, if it is one.
pub fn family_of(name: &str) -> Option<Family> {
    lookup(name).map(|i| i.family)
}

/// The attributes every frame of a source has: the fixed ones and the
/// patterns `asset.*` and `param.*`.
pub fn source_attributes() -> impl Iterator<Item = String> {
    FIXED
        .iter()
        .map(|(n, ..)| (*n).to_owned())
        .chain(["asset.*".to_owned(), "param.*".to_owned()])
}

/// True if the key entry `entry` (an attribute or a family name) covers the
/// attribute `name`.
fn covers(entry: &str, name: &str) -> bool {
    entry == name || Family::from_key(entry).is_some_and(|f| family_of(name) == Some(f))
}

/// The key of a group node: the attributes of `by`, families expanded,
/// without those of `except`.
#[derive(Debug, Clone, Copy)]
pub struct Key<'a> {
    pub by: &'a [String],
    pub except: &'a [String],
}

impl Key<'_> {
    pub fn includes(&self, name: &str) -> bool {
        self.by.iter().any(|e| covers(e, name)) && !self.except.iter().any(|e| covers(e, name))
    }
}

/// The key of a match node: all attributes except the named ones, or only
/// the named ones. The family `source` is never part of it, so that frames
/// of two sources can match.
#[derive(Debug, Clone, Copy)]
pub enum MatchKey<'a> {
    Except(&'a [String]),
    Only(&'a [String]),
}

impl MatchKey<'_> {
    pub fn includes(&self, name: &str) -> bool {
        if family_of(name) == Some(Family::Source) {
            return false;
        }
        match self {
            MatchKey::Except(except) => !except.iter().any(|e| covers(e, name)),
            MatchKey::Only(only) => only.iter().any(|e| covers(e, name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn fixed_and_prefixed_names_are_attributes() {
        assert_eq!(lookup("run.started_at").unwrap().ty, AttrType::Time);
        assert_eq!(lookup("sut.id").unwrap().family, Family::Run);
        assert_eq!(lookup("param.width").unwrap().ty, AttrType::Any);
        assert_eq!(lookup("asset.model").unwrap().family, Family::Case);
        assert_eq!(lookup("attr.pixels").unwrap().family, Family::Derived);
        assert_eq!(lookup("param."), None);
        assert_eq!(lookup("run"), None);
        assert_eq!(lookup("load_time"), None);
        assert!(is_prefix("sut"));
        assert!(is_prefix("param"));
        assert!(is_prefix("case_execution"));
        assert!(!is_prefix("b"));
        assert!(!is_prefix("source"));
    }

    #[test]
    fn a_family_expands_and_except_removes() {
        let by = names(&["case", "device.name"]);
        let except = names(&["param.codec"]);
        let key = Key {
            by: &by,
            except: &except,
        };
        assert!(key.includes("scenario.version"));
        assert!(key.includes("scenario.name"));
        assert!(key.includes("param.width"));
        assert!(key.includes("asset.model"));
        assert!(key.includes("device.name"));
        assert!(!key.includes("param.codec"));
        assert!(!key.includes("device.details"));
        assert!(!key.includes("run.id"));

        let except = names(&["case"]);
        let key = Key {
            by: &names(&["case", "run"]),
            except: &except,
        };
        assert!(!key.includes("param.width"));
        assert!(key.includes("run.id"));
    }

    #[test]
    fn source_is_never_part_of_a_match_key() {
        let none: Vec<String> = Vec::new();
        let key = MatchKey::Except(&none);
        assert!(key.includes("run.id"));
        assert!(key.includes("param.width"));
        assert!(!key.includes("source"));
        assert!(!key.includes("metric"));

        let only = names(&["case", "source", "metric"]);
        let key = MatchKey::Only(&only);
        assert!(key.includes("scenario.version"));
        assert!(!key.includes("run.id"));
        assert!(!key.includes("source"));
        assert!(!key.includes("metric"));

        let except = names(&["run", "sample"]);
        let key = MatchKey::Except(&except);
        assert!(!key.includes("run.id"));
        assert!(!key.includes("sample.repetition"));
        assert!(key.includes("device.name"));
    }
}
