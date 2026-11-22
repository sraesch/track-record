//! Syntax tree of the derived metric DSL.

use std::collections::BTreeSet;

use super::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Pow => "^",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedArg {
    pub name: String,
    pub span: Span,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// A number, optionally with a unit (`1.5[s]`). Without a unit it takes
    /// the unit its context requires.
    Number {
        value: f64,
        unit: Option<String>,
    },
    /// An uploaded scalar metric of the same sample (a bare name).
    Metric(String),
    /// An uploaded series of the same sample (`series.<name>`).
    Series(String),
    /// A derived metric (`derived.<name>`).
    Derived(String),
    Neg(Box<Expr>),
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Call {
        name: String,
        name_span: Span,
        args: Vec<Expr>,
        named: Vec<NamedArg>,
    },
}

/// Named arguments whose value is a keyword, not an expression.
pub const KEYWORD_ARGS: &[&str] = &["interp"];

/// The inputs an expression refers to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct References {
    pub metrics: BTreeSet<String>,
    pub series: BTreeSet<String>,
    pub derived: BTreeSet<String>,
}

impl Expr {
    /// Where the expression refers to the derived metric `name`
    /// (`derived.<name>`), in source order.
    pub fn derived_spans(&self, name: &str) -> Vec<Span> {
        let mut out = Vec::new();
        self.collect_derived(name, &mut out);
        out.sort_by_key(|s| s.start);
        out
    }

    fn collect_derived(&self, name: &str, out: &mut Vec<Span>) {
        match &self.kind {
            ExprKind::Derived(n) if n == name => out.push(self.span),
            ExprKind::Number { .. }
            | ExprKind::Metric(_)
            | ExprKind::Series(_)
            | ExprKind::Derived(_) => {}
            ExprKind::Neg(e) => e.collect_derived(name, out),
            ExprKind::Binary { lhs, rhs, .. } => {
                lhs.collect_derived(name, out);
                rhs.collect_derived(name, out);
            }
            ExprKind::Call { args, named, .. } => {
                for a in args {
                    a.collect_derived(name, out);
                }
                for n in named {
                    n.value.collect_derived(name, out);
                }
            }
        }
    }

    pub fn references(&self) -> References {
        let mut refs = References::default();
        self.collect(&mut refs);
        refs
    }

    fn collect(&self, refs: &mut References) {
        match &self.kind {
            ExprKind::Number { .. } => {}
            ExprKind::Metric(n) => {
                refs.metrics.insert(n.clone());
            }
            ExprKind::Series(n) => {
                refs.series.insert(n.clone());
            }
            ExprKind::Derived(n) => {
                refs.derived.insert(n.clone());
            }
            ExprKind::Neg(e) => e.collect(refs),
            ExprKind::Binary { lhs, rhs, .. } => {
                lhs.collect(refs);
                rhs.collect(refs);
            }
            ExprKind::Call { args, named, .. } => {
                for a in args {
                    a.collect(refs);
                }
                for n in named {
                    if !KEYWORD_ARGS.contains(&n.name.as_str()) {
                        n.value.collect(refs);
                    }
                }
            }
        }
    }

    /// The value of a constant number, including a negated one.
    pub fn constant(&self) -> Option<f64> {
        match &self.kind {
            ExprKind::Number { value, unit: None } => Some(*value),
            ExprKind::Neg(e) => e.constant().map(|v| -v),
            _ => None,
        }
    }
}
