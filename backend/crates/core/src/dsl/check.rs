//! Type and unit check (FR-17a): compiles a syntax tree into a typed tree
//! with the unit conversions it needs.
//!
//! Types are scalar and series. Units follow the dimension algebra of
//! [`crate::units`]:
//! - `+`, `-`, `min`, `max`, `clamp` need convertible units; the other
//!   operands are converted to the unit of the first one that has a unit.
//! - `*` and `/` combine units, `^` needs a constant integer exponent for a
//!   value with a unit.
//! - A number without a unit takes the unit its context requires: in
//!   `load_time + 1` it is in the unit of `load_time`, in `bytes / 1000` it
//!   is a plain factor. `1[ms]` gives a number a fixed unit.
//! - `rate` gives y/x, `auc` y·x, `time_to` the unit of x.
//!
//! The units of the inputs come from an [`Env`]. When evaluating, they are
//! the units of the measurements of one sample, so a metric whose unit
//! changed between versions (s, later ms) still gives consistent results:
//! the result is converted to the declared unit.

use super::ast::{BinOp, Expr, ExprKind, NamedArg};
use super::{Diagnostic, Span};
use crate::series::Interp;
use crate::units::{ResolvedUnit, UnitCatalogue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Scalar,
    Series,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Scalar => "scalar",
            Kind::Series => "series",
        }
    }
}

/// Units of an uploaded series.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SeriesUnits {
    /// Unit of y (the metric's unit).
    pub unit: String,
    pub x_unit: String,
    pub x_dimension: String,
}

/// What an expression produces.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Signature {
    pub kind: Kind,
    /// Unit of the value (of y for a series).
    pub unit: String,
    /// Unit and dimension of x; empty for a scalar.
    pub x_unit: String,
    pub x_dimension: String,
}

/// Units of the inputs an expression may reference.
pub trait Env {
    /// Unit of an uploaded scalar metric, or `None` if there is none.
    fn metric(&self, name: &str) -> Option<String>;
    /// Units of an uploaded series, or `None` if there is none.
    fn series(&self, name: &str) -> Option<SeriesUnits>;
    /// Signature of a derived metric; `Err` explains why it is unusable.
    fn derived(&mut self, name: &str) -> Result<Signature, String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reduction {
    Min,
    Max,
    Mean,
    Auc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    Abs,
    Log10,
    Sqrt,
}

/// An operand with the factor that converts it to the unit its context
/// needs, and for a series the factor that converts its x to the x of the
/// result.
#[derive(Debug, Clone, PartialEq)]
pub struct Conv {
    pub node: Box<Node>,
    pub factor: f64,
    pub x_factor: f64,
}

/// The checked tree that [`super::eval`] evaluates.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Const(f64),
    Metric(String),
    Series(String),
    Derived(String),
    Neg(Box<Node>),
    Binary {
        op: BinOp,
        lhs: Conv,
        rhs: Conv,
    },
    Func(Func, Box<Node>),
    /// Point by point min or max of two or more operands.
    Extreme {
        max: bool,
        args: Vec<Conv>,
    },
    Clamp {
        value: Conv,
        lo: Conv,
        hi: Conv,
    },
    Reduce {
        op: Reduction,
        series: Box<Node>,
        from: Option<Conv>,
        to: Option<Conv>,
        interp: Interp,
    },
    TimeTo {
        series: Box<Node>,
        threshold: Conv,
        from: Option<Conv>,
        interp: Interp,
    },
    Rate(Box<Node>),
    Cumsum(Box<Node>),
    MovingAvg {
        series: Box<Node>,
        window: Conv,
    },
    Log10X(Box<Node>),
    Rekey {
        series: Box<Node>,
        /// `by` with its x converted to the x of `series`.
        by: Conv,
        interp: Interp,
    },
}

/// A checked expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    pub node: Node,
    /// Kind, output unit (the declared unit if one was given) and x.
    pub signature: Signature,
    /// The unit the expression has by itself, before conversion to the
    /// declared unit.
    pub inferred_unit: String,
    /// Converts the value from the inferred to the output unit.
    pub output_factor: f64,
}

/// Checks an expression. With `declared_unit`, the result must convert to
/// it and is converted on evaluation.
pub fn check(
    expr: &Expr,
    env: &mut dyn Env,
    catalogue: &UnitCatalogue,
    declared_unit: Option<&str>,
) -> Result<Checked, Vec<Diagnostic>> {
    let mut c = Checker {
        env,
        cat: catalogue,
        diags: Vec::new(),
    };
    let typed = c.expr(expr);
    let (Some(t), true) = (typed, c.diags.is_empty()) else {
        return Err(c.diags);
    };
    let inferred_unit = match &t.unit {
        U::Any => String::new(),
        U::Known(u) => u.symbol.clone(),
    };
    let (unit, output_factor) = match (declared_unit, &t.unit) {
        (None, _) => (inferred_unit.clone(), 1.0),
        (Some(d), U::Any) => (d.to_owned(), 1.0),
        (Some(d), U::Known(u)) => match u.conversion_factor(&catalogue.resolve(d)) {
            Some(f) => (d.to_owned(), f),
            None => {
                return Err(vec![Diagnostic::new(
                    expr.span,
                    format!(
                        "the expression is in {}, which cannot be converted to the declared \
                         unit {}",
                        show_unit(&u.symbol),
                        show_unit(d)
                    ),
                )]);
            }
        },
    };
    Ok(Checked {
        node: t.node,
        signature: Signature {
            kind: t.kind,
            unit,
            x_unit: t.x.as_ref().map(|x| x.symbol.clone()).unwrap_or_default(),
            x_dimension: t.x_dimension,
        },
        inferred_unit,
        output_factor,
    })
}

fn show_unit(symbol: &str) -> String {
    if symbol.is_empty() {
        "no unit (dimensionless)".to_owned()
    } else {
        format!("\"{symbol}\"")
    }
}

/// Unit of a typed node. A number without a unit is `Any`.
#[derive(Debug, Clone)]
enum U {
    Any,
    Known(ResolvedUnit),
}

impl U {
    fn describe(&self) -> String {
        match self {
            U::Any => "a number".to_owned(),
            U::Known(u) => show_unit(&u.symbol),
        }
    }
}

#[derive(Debug, Clone)]
struct T {
    node: Node,
    kind: Kind,
    unit: U,
    /// Unit of x for a series.
    x: Option<ResolvedUnit>,
    x_dimension: String,
}

impl T {
    fn scalar(node: Node, unit: U) -> T {
        T {
            node,
            kind: Kind::Scalar,
            unit,
            x: None,
            x_dimension: String::new(),
        }
    }

    fn with_node(&self, node: Node, unit: U) -> T {
        T {
            node,
            kind: self.kind,
            unit,
            x: self.x.clone(),
            x_dimension: self.x_dimension.clone(),
        }
    }

    fn conv(self, factor: f64) -> Conv {
        Conv {
            node: Box::new(self.node),
            factor,
            x_factor: 1.0,
        }
    }
}

struct Checker<'a> {
    env: &'a mut dyn Env,
    cat: &'a UnitCatalogue,
    diags: Vec<Diagnostic>,
}

impl Checker<'_> {
    fn error(&mut self, span: Span, message: impl Into<String>) -> Option<T> {
        self.diags.push(Diagnostic::new(span, message));
        None
    }

    fn expr(&mut self, e: &Expr) -> Option<T> {
        match &e.kind {
            ExprKind::Number { value, unit } => Some(T::scalar(
                Node::Const(*value),
                match unit {
                    Some(u) => U::Known(self.cat.resolve(u)),
                    None => U::Any,
                },
            )),
            ExprKind::Metric(name) => match self.env.metric(name) {
                Some(unit) => Some(T::scalar(
                    Node::Metric(name.clone()),
                    U::Known(self.cat.resolve(&unit)),
                )),
                None if self.env.series(name).is_some() => {
                    self.error(e.span, format!("{name} is a series; write series.{name}"))
                }
                None if ["linear", "step"].contains(&name.as_str()) => {
                    self.error(e.span, format!("{name} is only valid as interp={name}"))
                }
                None => self.error(e.span, format!("unknown metric {name}")),
            },
            ExprKind::Series(name) => match self.env.series(name) {
                Some(s) => Some(T {
                    node: Node::Series(name.clone()),
                    kind: Kind::Series,
                    unit: U::Known(self.cat.resolve(&s.unit)),
                    x: Some(self.cat.resolve(&s.x_unit)),
                    x_dimension: s.x_dimension,
                }),
                None if self.env.metric(name).is_some() => self.error(
                    e.span,
                    format!("{name} has no series, only scalar values; write {name}"),
                ),
                None => self.error(e.span, format!("no series of metric {name}")),
            },
            ExprKind::Derived(name) => match self.env.derived(name) {
                Ok(sig) => Some(T {
                    node: Node::Derived(name.clone()),
                    kind: sig.kind,
                    unit: U::Known(self.cat.resolve(&sig.unit)),
                    x: (sig.kind == Kind::Series).then(|| self.cat.resolve(&sig.x_unit)),
                    x_dimension: sig.x_dimension,
                }),
                Err(message) => self.error(e.span, message),
            },
            ExprKind::Neg(inner) => {
                let t = self.expr(inner)?;
                let unit = t.unit.clone();
                let node = Node::Neg(Box::new(t.node.clone()));
                Some(t.with_node(node, unit))
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let l = self.expr(lhs);
                let r = self.expr(rhs);
                self.binary(*op, l?, lhs.span, r?, rhs)
            }
            ExprKind::Call {
                name,
                name_span,
                args,
                named,
            } => self.call(name, *name_span, args, named, e.span),
        }
    }

    /// x of the result of combining operands point by point, and each
    /// operand's x factor relative to it.
    fn pointwise_x(
        &mut self,
        items: &[(&T, Span)],
    ) -> Option<(Option<ResolvedUnit>, String, Vec<f64>)> {
        let reference = items
            .iter()
            .find(|(t, _)| t.kind == Kind::Series)
            .map(|(t, _)| (t.x.clone().expect("series has x"), t.x_dimension.clone()));
        let Some((x, dim)) = reference else {
            return Some((None, String::new(), vec![1.0; items.len()]));
        };
        let mut factors = Vec::new();
        for (t, span) in items.iter() {
            match (&t.kind, &t.x) {
                (Kind::Series, Some(tx)) => match tx.conversion_factor(&x) {
                    Some(f) => factors.push(f),
                    None => {
                        self.error(
                            *span,
                            format!(
                                "x is in {} here but in {} in the other series; point by point \
                                 math needs series over the same x",
                                show_unit(&tx.symbol),
                                show_unit(&x.symbol)
                            ),
                        );
                        return None;
                    }
                },
                _ => factors.push(1.0),
            }
        }
        Some((Some(x), dim, factors))
    }

    /// Unit all operands are converted to (the first known one) and each
    /// operand's factor.
    fn unify(&mut self, items: &[(&T, Span)], what: &str) -> Option<(U, Vec<f64>)> {
        let target = items.iter().find_map(|(t, _)| match &t.unit {
            U::Known(u) => Some(u.clone()),
            U::Any => None,
        });
        let Some(target) = target else {
            return Some((U::Any, vec![1.0; items.len()]));
        };
        let mut factors = Vec::new();
        let mut ok = true;
        for (t, span) in items {
            match &t.unit {
                U::Any => factors.push(1.0),
                U::Known(u) => match u.conversion_factor(&target) {
                    Some(f) => factors.push(f),
                    None => {
                        ok = false;
                        self.diags.push(Diagnostic::new(
                            *span,
                            format!(
                                "{what} needs compatible units, but this is in {} and the first \
                                 operand in {}",
                                show_unit(&u.symbol),
                                show_unit(&target.symbol)
                            ),
                        ));
                    }
                },
            }
        }
        ok.then_some((U::Known(target), factors))
    }

    fn binary(&mut self, op: BinOp, l: T, lspan: Span, r: T, rhs: &Expr) -> Option<T> {
        let rspan = rhs.span;
        let (x, x_dimension, xf) = self.pointwise_x(&[(&l, lspan), (&r, rspan)])?;
        let kind = if x.is_some() {
            Kind::Series
        } else {
            Kind::Scalar
        };
        let (unit, lf, rf) = match op {
            BinOp::Add | BinOp::Sub => {
                let (unit, f) =
                    self.unify(&[(&l, lspan), (&r, rspan)], &format!("'{}'", op.symbol()))?;
                (unit, f[0], f[1])
            }
            BinOp::Mul | BinOp::Div => {
                let unit = match (&l.unit, &r.unit, op) {
                    (U::Any, U::Any, _) => U::Any,
                    (U::Known(a), U::Any, _) => U::Known(a.clone()),
                    (U::Any, U::Known(b), BinOp::Mul) => U::Known(b.clone()),
                    (U::Any, U::Known(b), _) => U::Known(ResolvedUnit::dimensionless().div(b)),
                    (U::Known(a), U::Known(b), BinOp::Mul) => U::Known(a.mul(b)),
                    (U::Known(a), U::Known(b), _) => U::Known(a.div(b)),
                };
                (unit, 1.0, 1.0)
            }
            BinOp::Pow => {
                let rf = match &r.unit {
                    U::Any => 1.0,
                    U::Known(u) if u.dimension.is_dimensionless() => u.to_base,
                    U::Known(u) => {
                        return self.error(
                            rspan,
                            format!(
                                "an exponent cannot have a unit, but this is in {}",
                                show_unit(&u.symbol)
                            ),
                        );
                    }
                };
                match &l.unit {
                    U::Any => (U::Any, 1.0, rf),
                    U::Known(u) => match rhs.constant() {
                        Some(e)
                            if e.fract() == 0.0 && e.abs() <= 64.0 && r.kind == Kind::Scalar =>
                        {
                            (U::Known(u.powi(e as i32)), 1.0, rf)
                        }
                        _ if u.dimension.is_dimensionless() => {
                            (U::Known(ResolvedUnit::dimensionless()), u.to_base, rf)
                        }
                        _ => {
                            return self.error(
                                rspan,
                                format!(
                                    "a value in {} can only be raised to a constant integer power",
                                    show_unit(&u.symbol)
                                ),
                            );
                        }
                    },
                }
            }
        };
        Some(T {
            node: Node::Binary {
                op,
                lhs: Conv {
                    node: Box::new(l.node),
                    factor: lf,
                    x_factor: xf[0],
                },
                rhs: Conv {
                    node: Box::new(r.node),
                    factor: rf,
                    x_factor: xf[1],
                },
            },
            kind,
            unit,
            x,
            x_dimension,
        })
    }

    fn call(
        &mut self,
        name: &str,
        name_span: Span,
        args: &[Expr],
        named: &[NamedArg],
        span: Span,
    ) -> Option<T> {
        let (allowed, min_args, max_args): (&[&str], usize, usize) = match name {
            "abs" | "log10" | "sqrt" => (&[], 1, 1),
            "min" | "max" => (&["from", "to"], 1, 16),
            "clamp" => (&[], 3, 3),
            "mean" => (&["from", "to"], 1, 1),
            "auc" => (&["from", "to", "interp"], 1, 1),
            "time_to" => (&["from", "interp"], 2, 2),
            "rate" | "cumsum" | "log10_x" => (&[], 1, 1),
            "moving_avg" => (&[], 2, 2),
            "rekey" => (&["interp"], 2, 2),
            _ => {
                return self.error(
                    name_span,
                    format!(
                        "unknown function {name}; available: abs, log10, sqrt, min, max, clamp, \
                         mean, auc, time_to, rate, cumsum, moving_avg, log10_x, rekey"
                    ),
                );
            }
        };
        if args.len() < min_args || args.len() > max_args {
            let expected = if min_args == max_args {
                format!("{min_args}")
            } else {
                format!("{min_args} to {max_args}")
            };
            return self.error(
                span,
                format!(
                    "{name} takes {expected} positional argument(s), got {}",
                    args.len()
                ),
            );
        }
        let mut bad = false;
        for n in named {
            if !allowed.contains(&n.name.as_str()) {
                bad = true;
                let hint = if allowed.is_empty() {
                    "none".to_owned()
                } else {
                    allowed.join(", ")
                };
                self.diags.push(Diagnostic::new(
                    n.span,
                    format!("{name} has no argument {}; named arguments: {hint}", n.name),
                ));
            }
        }
        let interp = self.interp(named);
        let typed: Vec<Option<T>> = args.iter().map(|a| self.expr(a)).collect();
        if bad || interp.is_none() {
            return None;
        }
        let interp = interp?;
        let mut typed: Vec<T> = typed.into_iter().collect::<Option<_>>()?;
        let spans: Vec<Span> = args.iter().map(|a| a.span).collect();
        let named_expr = |key: &str| named.iter().find(|n| n.name == key);

        match name {
            "abs" | "log10" | "sqrt" => {
                let t = typed.remove(0);
                let (func, unit) = match name {
                    "abs" => (Func::Abs, t.unit.clone()),
                    "log10" => (Func::Log10, U::Known(ResolvedUnit::dimensionless())),
                    _ => match &t.unit {
                        U::Any => (Func::Sqrt, U::Any),
                        U::Known(u) => match u.sqrt() {
                            Some(root) => (Func::Sqrt, U::Known(root)),
                            None => {
                                return self.error(
                                    spans[0],
                                    format!("{} has no square root", show_unit(&u.symbol)),
                                );
                            }
                        },
                    },
                };
                let node = Node::Func(func, Box::new(t.node.clone()));
                Some(t.with_node(node, unit))
            }
            "min" | "max" if typed.len() == 1 => {
                let op = if name == "min" {
                    Reduction::Min
                } else {
                    Reduction::Max
                };
                let s = typed.remove(0);
                self.reduce(
                    op,
                    s,
                    spans[0],
                    named_expr("from"),
                    named_expr("to"),
                    interp,
                    name,
                )
            }
            "min" | "max" => {
                if let Some(n) = named.first() {
                    return self.error(
                        n.span,
                        format!("{} applies to the single-series form {name}(s)", n.name),
                    );
                }
                let items: Vec<(&T, Span)> = typed.iter().zip(spans.iter().copied()).collect();
                let (unit, factors) = self.unify(&items, name)?;
                let (x, x_dimension, xf) = self.pointwise_x(&items)?;
                let kind = if x.is_some() {
                    Kind::Series
                } else {
                    Kind::Scalar
                };
                let args = typed
                    .into_iter()
                    .zip(factors)
                    .zip(xf)
                    .map(|((t, f), xf)| Conv {
                        node: Box::new(t.node),
                        factor: f,
                        x_factor: xf,
                    })
                    .collect();
                Some(T {
                    node: Node::Extreme {
                        max: name == "max",
                        args,
                    },
                    kind,
                    unit,
                    x,
                    x_dimension,
                })
            }
            "clamp" => {
                let items: Vec<(&T, Span)> = typed.iter().zip(spans.iter().copied()).collect();
                let (unit, f) = self.unify(&items, "clamp")?;
                let (x, x_dimension, xf) = self.pointwise_x(&items)?;
                let kind = if x.is_some() {
                    Kind::Series
                } else {
                    Kind::Scalar
                };
                let mut convs = typed.into_iter().zip(f).zip(xf).map(|((t, f), xf)| Conv {
                    node: Box::new(t.node),
                    factor: f,
                    x_factor: xf,
                });
                let (value, lo, hi) = (convs.next()?, convs.next()?, convs.next()?);
                Some(T {
                    node: Node::Clamp { value, lo, hi },
                    kind,
                    unit,
                    x,
                    x_dimension,
                })
            }
            "mean" | "auc" => {
                let op = if name == "mean" {
                    Reduction::Mean
                } else {
                    Reduction::Auc
                };
                let s = typed.remove(0);
                self.reduce(
                    op,
                    s,
                    spans[0],
                    named_expr("from"),
                    named_expr("to"),
                    interp,
                    name,
                )
            }
            "time_to" => {
                let threshold = typed.pop()?;
                let s = typed.pop()?;
                let (sx, y) = self.series_arg(&s, spans[0], name)?;
                let threshold = self.scalar_in(threshold, spans[1], &y, "the threshold")?;
                let from = self.bound(named_expr("from"), &sx)?;
                Some(T::scalar(
                    Node::TimeTo {
                        series: Box::new(s.node),
                        threshold,
                        from,
                        interp,
                    },
                    U::Known(sx),
                ))
            }
            "rate" | "cumsum" | "log10_x" => {
                let s = typed.remove(0);
                let (x, y) = self.series_arg(&s, spans[0], name)?;
                let inner = Box::new(s.node.clone());
                Some(match name {
                    "rate" => s.with_node(Node::Rate(inner), U::Known(y.div(&x))),
                    "cumsum" => s.with_node(Node::Cumsum(inner), U::Known(y)),
                    _ => T {
                        node: Node::Log10X(inner),
                        kind: Kind::Series,
                        unit: U::Known(y),
                        x: Some(ResolvedUnit::dimensionless()),
                        x_dimension: format!("log10({})", s.x_dimension),
                    },
                })
            }
            "moving_avg" => {
                let window = typed.pop()?;
                let s = typed.pop()?;
                let (x, _) = self.series_arg(&s, spans[0], name)?;
                let window = self.scalar_in(window, spans[1], &x, "the window")?;
                let node = Node::MovingAvg {
                    series: Box::new(s.node.clone()),
                    window,
                };
                let unit = s.unit.clone();
                Some(s.with_node(node, unit))
            }
            "rekey" => {
                let by = typed.pop()?;
                let s = typed.pop()?;
                let (sx, _) = self.series_arg(&s, spans[0], name)?;
                let (bx, by_y) = self.series_arg(&by, spans[1], name)?;
                let Some(xf) = bx.conversion_factor(&sx) else {
                    return self.error(
                        spans[1],
                        format!(
                            "rekey needs series over the same x, but x is in {} here and in {} \
                             in the first series",
                            show_unit(&bx.symbol),
                            show_unit(&sx.symbol)
                        ),
                    );
                };
                let x_dimension = match &args[1].kind {
                    ExprKind::Series(n) | ExprKind::Derived(n) | ExprKind::Metric(n) => n.clone(),
                    _ => "rekeyed".to_owned(),
                };
                Some(T {
                    node: Node::Rekey {
                        series: Box::new(s.node),
                        by: Conv {
                            node: Box::new(by.node),
                            factor: 1.0,
                            x_factor: xf,
                        },
                        interp,
                    },
                    kind: Kind::Series,
                    unit: s.unit,
                    x: Some(by_y),
                    x_dimension,
                })
            }
            _ => unreachable!("function names are checked above"),
        }
    }

    fn interp(&mut self, named: &[NamedArg]) -> Option<Interp> {
        let Some(n) = named.iter().find(|n| n.name == "interp") else {
            return Some(Interp::Linear);
        };
        if let ExprKind::Metric(v) = &n.value.kind
            && let Some(i) = Interp::parse(v)
        {
            return Some(i);
        }
        self.error(n.value.span, "interp must be linear or step");
        None
    }

    /// Requires a series; returns its x and y units.
    fn series_arg(
        &mut self,
        t: &T,
        span: Span,
        func: &str,
    ) -> Option<(ResolvedUnit, ResolvedUnit)> {
        if t.kind != Kind::Series {
            self.error(
                span,
                format!("{func} needs a series here, e.g. series.<name>"),
            );
            return None;
        }
        let y = match &t.unit {
            U::Known(u) => u.clone(),
            U::Any => ResolvedUnit::dimensionless(),
        };
        Some((t.x.clone().expect("series has x"), y))
    }

    /// Requires a scalar convertible to `unit`.
    fn scalar_in(&mut self, t: T, span: Span, unit: &ResolvedUnit, what: &str) -> Option<Conv> {
        if t.kind != Kind::Scalar {
            self.error(span, format!("{what} must be a scalar, not a series"));
            return None;
        }
        let factor = match &t.unit {
            U::Any => 1.0,
            U::Known(u) => match u.conversion_factor(unit) {
                Some(f) => f,
                None => {
                    self.error(
                        span,
                        format!(
                            "{what} is in {} but must be in a unit like {}",
                            t.unit.describe(),
                            show_unit(&unit.symbol)
                        ),
                    );
                    return None;
                }
            },
        };
        Some(t.conv(factor))
    }

    /// A `from=`/`to=` bound in the unit of x.
    fn bound(&mut self, arg: Option<&NamedArg>, x: &ResolvedUnit) -> Option<Option<Conv>> {
        let Some(arg) = arg else {
            return Some(None);
        };
        let t = self.expr(&arg.value)?;
        let what = format!("{}=", arg.name);
        Some(Some(self.scalar_in(t, arg.value.span, x, &what)?))
    }

    #[allow(clippy::too_many_arguments)]
    fn reduce(
        &mut self,
        op: Reduction,
        s: T,
        span: Span,
        from: Option<&NamedArg>,
        to: Option<&NamedArg>,
        interp: Interp,
        name: &str,
    ) -> Option<T> {
        let (x, y) = self.series_arg(&s, span, name)?;
        let from = self.bound(from, &x);
        let to = self.bound(to, &x);
        let unit = match op {
            Reduction::Auc => U::Known(y.mul(&x)),
            _ => s.unit.clone(),
        };
        Some(T::scalar(
            Node::Reduce {
                op,
                series: Box::new(s.node),
                from: from?,
                to: to?,
                interp,
            },
            unit,
        ))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::dsl::parse;
    use crate::units::tests::catalogue;

    /// A test environment: scalar metrics, series and derived signatures.
    #[derive(Default)]
    pub(crate) struct TestEnv {
        pub metrics: HashMap<String, String>,
        pub series: HashMap<String, SeriesUnits>,
        pub derived: HashMap<String, Signature>,
    }

    impl Env for TestEnv {
        fn metric(&self, name: &str) -> Option<String> {
            self.metrics.get(name).cloned()
        }
        fn series(&self, name: &str) -> Option<SeriesUnits> {
            self.series.get(name).cloned()
        }
        fn derived(&mut self, name: &str) -> Result<Signature, String> {
            self.derived
                .get(name)
                .cloned()
                .ok_or_else(|| format!("unknown derived metric {name}"))
        }
    }

    pub(crate) fn env() -> TestEnv {
        let mut e = TestEnv::default();
        for (n, u) in [
            ("bytes", "B"),
            ("time", "s"),
            ("load_ms", "ms"),
            ("score", ""),
            ("t0", "s"),
        ] {
            e.metrics.insert(n.into(), u.into());
        }
        for (n, u, x) in [
            ("fps", "fps", "s"),
            ("loaded", "%", "s"),
            ("received", "B", "s"),
            ("quality", "", "s"),
            ("late", "", "ms"),
            ("by_size", "", "B"),
        ] {
            e.series.insert(
                n.into(),
                SeriesUnits {
                    unit: u.into(),
                    x_unit: x.into(),
                    x_dimension: if x == "B" { "size" } else { "time" }.into(),
                },
            );
        }
        e.derived.insert(
            "throughput".into(),
            Signature {
                kind: Kind::Scalar,
                unit: "MB/s".into(),
                x_unit: String::new(),
                x_dimension: String::new(),
            },
        );
        e
    }

    fn ok(src: &str, declared: Option<&str>) -> Checked {
        let expr = parse(src).unwrap();
        check(&expr, &mut env(), &catalogue(), declared).unwrap_or_else(|d| panic!("{src}: {d:?}"))
    }

    fn err(src: &str, declared: Option<&str>) -> Vec<Diagnostic> {
        let expr = parse(src).unwrap();
        check(&expr, &mut env(), &catalogue(), declared).expect_err(src)
    }

    fn conv_to(c: &Checked, unit: &str) -> Option<f64> {
        catalogue().conversion_factor(&c.inferred_unit, unit)
    }

    #[test]
    fn scalar_units() {
        let c = ok("bytes / time", None);
        assert_eq!(c.signature.kind, Kind::Scalar);
        assert_eq!(c.inferred_unit, "B/s");
        let c = ok("bytes / time", Some("MB/s"));
        assert_eq!(c.output_factor, 1e-6);
        assert_eq!(c.signature.unit, "MB/s");
        // Mixed time units convert.
        let c = ok("time + load_ms", Some("ms"));
        assert_eq!(c.output_factor, 1000.0);
        let Node::Binary { rhs, .. } = &c.node else {
            panic!()
        };
        assert_eq!(rhs.factor, 1e-3);
        // Numbers adopt units; annotated numbers have their own.
        assert_eq!(ok("time + 1", None).inferred_unit, "s");
        let Node::Binary { rhs, .. } = ok("time + 500[ms]", None).node else {
            panic!()
        };
        assert_eq!(rhs.factor, 1e-3);
        assert_eq!(ok("1000 / time", None).inferred_unit, "1/s");
        assert_eq!(conv_to(&ok("1 / time", None), "Hz"), Some(1.0));
        assert_eq!(ok("time^2", None).inferred_unit, "(s)^2");
        assert_eq!(ok("sqrt(time^2)", None).signature.kind, Kind::Scalar);
        assert_eq!(ok("score^0.5", None).inferred_unit, "");
        assert_eq!(ok("log10(bytes)", None).inferred_unit, "");
        assert_eq!(ok("clamp(score, 0, 1)", None).inferred_unit, "");
        assert_eq!(ok("max(time, load_ms, 1)", Some("s")).output_factor, 1.0);
        assert_eq!(ok("derived.throughput * 2", None).inferred_unit, "MB/s");
        assert_eq!(ok("2 + 3", Some("ms")).output_factor, 1.0);
    }

    #[test]
    fn unit_errors() {
        assert!(
            err("bytes + time", None)[0]
                .message
                .contains("compatible units")
        );
        assert!(
            err("bytes / time", Some("s"))[0]
                .message
                .contains("declared unit")
        );
        assert!(
            err("time ^ score", None)[0]
                .message
                .contains("constant integer")
        );
        assert!(
            err("score ^ time", None)[0]
                .message
                .contains("exponent cannot have a unit")
        );
        assert!(err("sqrt(time)", None)[0].message.contains("square root"));
        assert!(
            err("clamp(time, 0[B], 1)", None)[0]
                .message
                .contains("compatible")
        );
    }

    #[test]
    fn series_functions() {
        let c = ok("max(series.fps)", None);
        assert_eq!(
            (c.signature.kind, c.inferred_unit.as_str()),
            (Kind::Scalar, "fps")
        );
        let c = ok("time_to(series.loaded, 90, from=t0)", Some("ms"));
        assert_eq!(c.output_factor, 1000.0);
        // 0.9 adopts % of y; 0.9[] would be a ratio.
        let Node::TimeTo { threshold, .. } = ok("time_to(series.loaded, 0.9[])", None).node else {
            panic!()
        };
        assert_eq!(threshold.factor, 100.0);
        assert_eq!(
            conv_to(&ok("auc(series.fps, from=1, to=2[min])", None), "fps*s"),
            Some(1.0)
        );
        let c = ok("rate(series.received)", Some("MB/s"));
        assert_eq!((c.signature.kind, c.output_factor), (Kind::Series, 1e-6));
        assert_eq!(c.signature.x_unit, "s");
        let c = ok("rekey(series.quality, series.received)", None);
        assert_eq!(
            (
                c.signature.x_unit.as_str(),
                c.signature.x_dimension.as_str()
            ),
            ("B", "received")
        );
        let c = ok("moving_avg(series.fps, 500[ms])", None);
        let Node::MovingAvg { window, .. } = c.node else {
            panic!()
        };
        assert_eq!(window.factor, 1e-3);
        assert_eq!(
            ok("log10_x(series.fps)", None).signature.x_dimension,
            "log10(time)"
        );
        // Pointwise: series and scalar, two series over the same x.
        assert_eq!(ok("series.fps * 2", None).signature.kind, Kind::Series);
        let c = ok("(series.quality + series.late) / 2", None);
        assert_eq!(c.signature.kind, Kind::Series);
        let Node::Binary { lhs, .. } = c.node else {
            panic!()
        };
        let Node::Binary { rhs, .. } = *lhs.node else {
            panic!()
        };
        assert_eq!(rhs.x_factor, 1e-3, "ms converted to s");
        assert_eq!(
            ok("max(series.quality, 0.5)", None).signature.kind,
            Kind::Series
        );
        assert_eq!(
            ok("mean(abs(series.quality - 1))", None).signature.kind,
            Kind::Scalar
        );
    }

    #[test]
    fn series_errors() {
        let first = |src: &str| err(src, None).remove(0).message;
        assert!(first("max(time)").contains("needs a series"));
        assert!(first("series.quality + series.by_size").contains("same x"));
        assert!(first("time_to(series.fps, 1[B])").contains("threshold"));
        assert!(first("mean(series.fps, from=1[B])").contains("from="));
        assert!(first("time_to(series.fps, series.fps)").contains("scalar"));
        assert!(first("max(series.fps, interp=step)").contains("no argument interp"));
        assert!(first("auc(series.fps, interp=cubic)").contains("linear or step"));
        assert!(first("rekey(series.fps, series.by_size)").contains("same x"));
        assert!(first("fps").contains("write series.fps"));
        assert!(first("series.bytes").contains("write bytes"));
        assert!(first("series.nothing").contains("no series"));
        assert!(first("nothing").contains("unknown metric"));
        assert!(first("derived.nothing").contains("unknown derived"));
        assert!(first("frobnicate(1)").contains("unknown function"));
        assert!(first("abs(1, 2)").contains("takes 1"));
        assert!(first("min(time, t0, from=1)").contains("single-series"));
    }

    #[test]
    fn collects_several_errors() {
        let d = err("nothing + other * series.none", None);
        assert_eq!(d.len(), 3);
        assert_eq!(d[0].span, Span::new(0, 7));
    }
}
