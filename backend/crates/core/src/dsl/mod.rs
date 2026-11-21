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
