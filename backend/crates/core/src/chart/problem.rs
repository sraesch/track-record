//! Problems of a chart spec, reported per node and setting (ADR 0022,
//! "Spec").

use std::fmt;

use serde::Serialize;

use crate::dsl::{Diagnostic, Span};

/// One problem of a spec. `node` and `setting` say where it is; a problem
/// of the whole spec has neither.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Problem {
    /// Id of the node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// Path of the setting in the node, e.g. `filter.sut_branch`,
    /// `functions[1]` or `inputs.b`; for a problem of the spec, e.g.
    /// `variables[0].default`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setting: Option<String>,
    pub message: String,
    /// Byte range inside the expression of `setting`, as the DSL reports it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

impl Problem {
    /// A problem of the whole spec.
    pub fn spec(message: impl Into<String>) -> Self {
        Self {
            node: None,
            setting: None,
            message: message.into(),
            span: None,
        }
    }

    /// A problem of a setting of the spec itself (`format`, `variables`).
    pub fn spec_setting(setting: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            setting: Some(setting.into()),
            ..Self::spec(message)
        }
    }

    /// A problem of a node.
    pub fn node(node: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            node: Some(node.into()),
            ..Self::spec(message)
        }
    }

    /// A problem of a setting of a node.
    pub fn setting(
        node: impl Into<String>,
        setting: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            node: Some(node.into()),
            setting: Some(setting.into()),
            ..Self::spec(message)
        }
    }

    /// A problem inside the expression of a setting.
    pub fn expression(node: &str, setting: &str, diagnostic: &Diagnostic) -> Self {
        Self {
            span: Some(diagnostic.span),
            ..Self::setting(node, setting, diagnostic.message.clone())
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.node, &self.setting) {
            (Some(n), Some(s)) => write!(f, "node {n:?}, {s}: ")?,
            (Some(n), None) => write!(f, "node {n:?}: ")?,
            (None, Some(s)) => write!(f, "{s}: ")?,
            (None, None) => {}
        }
        f.write_str(&self.message)?;
        if let Some(span) = self.span {
            write!(f, " (at {}..{})", span.start, span.end)?;
        }
        Ok(())
    }
}
