//! The derived metric DSL (FR-17, FR-17a, ADR 0009).
//!
//! An expression is parsed ([`parse`]), type- and unit-checked against the
//! units of its inputs ([`check`]) and evaluated on the inputs of one sample
//! ([`eval`]). [`program`] ties derived metrics that use other derived
//! metrics together and evaluates them per sample.
//!
//! The language is deliberately small: numbers, references, `+ - * / ^`,
//! unary minus, parentheses and a fixed set of functions. There are no
//! loops, no variables and no I/O, and inputs are limited in size, so an
//! expression is safe to evaluate on the server.

pub mod ast;
pub mod check;
pub mod eval;
pub mod lexer;
pub mod parser;
pub mod program;

use std::fmt;

pub use ast::{Expr, ExprKind, References};
pub use check::{Checked, Kind, Signature};
pub use parser::parse;

/// Longest accepted expression, in bytes.
pub const MAX_SOURCE_LEN: usize = 4096;
/// Deepest accepted nesting of the syntax tree.
pub const MAX_DEPTH: usize = 64;
/// Most derived metrics in one chain of composition.
pub const MAX_COMPOSITION_DEPTH: usize = 32;

/// Byte range in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

/// How an expression refers to the derived metric `name`: `derived.<name>`,
/// with the name in backticks unless it is a plain identifier.
pub fn derived_reference(name: &str) -> String {
    let plain = name.chars().next().is_some_and(lexer::is_ident_start)
        && name.chars().all(lexer::is_ident_char);
    if plain {
        format!("derived.{name}")
    } else {
        format!("derived.`{name}`")
    }
}

/// `source` with every reference to the derived metric `old` replaced by
/// one to `new`, for renaming a derived metric. The rest of the text is
/// kept as it is. `None` if the expression does not refer to `old`.
pub fn rename_derived(source: &str, old: &str, new: &str) -> Result<Option<String>, Diagnostic> {
    let spans = parse(source)?.derived_spans(old);
    if spans.is_empty() {
        return Ok(None);
    }
    let reference = derived_reference(new);
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for span in spans {
        out.push_str(&source[at..span.start]);
        out.push_str(&reference);
        at = span.end;
    }
    out.push_str(&source[at..]);
    Ok(Some(out))
}

/// An error in an expression, with the part of the source it refers to.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (at {}..{})",
            self.message, self.span.start, self.span.end
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_are_quoted_when_needed() {
        assert_eq!(derived_reference("time_to_90"), "derived.time_to_90");
        assert_eq!(derived_reference("_x1"), "derived._x1");
        assert_eq!(derived_reference("90th"), "derived.`90th`");
        assert_eq!(
            derived_reference("load time (cold)"),
            "derived.`load time (cold)`"
        );
    }

    #[test]
    fn renaming_rewrites_only_references_to_the_metric() {
        let renamed = |src: &str, new: &str| rename_derived(src, "a", new).unwrap();
        assert_eq!(
            renamed("derived.a * 2 + derived.ab - a + series.a", "b"),
            Some("derived.b * 2 + derived.ab - a + series.a".into())
        );
        // Every reference, quoted or spaced, in calls and named arguments;
        // the rest of the text stays as it was.
        assert_eq!(
            renamed(
                "mean(series.fps, from=derived.`a`)  /  max(derived . a, 1)",
                "load time"
            ),
            Some(
                "mean(series.fps, from=derived.`load time`)  /  max(derived.`load time`, 1)".into()
            )
        );
        assert_eq!(renamed("derived.b + 1", "c"), None);
        assert!(rename_derived("derived.a +", "a", "b").is_err());
        let out = renamed("derived.a / derived.a", "x y").unwrap();
        assert_eq!(
            parse(&out).unwrap().references().derived,
            ["x y".to_owned()].into()
        );
    }
}
