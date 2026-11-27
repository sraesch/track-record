//! Recursive descent parser.
//!
//! ```text
//! expr     = additive
//! additive = product (("+" | "-") product)*
//! product  = unary (("*" | "/") unary)*
//! unary    = "-" unary | power
//! power    = primary ("^" unary)?          right-associative; -2^2 = -(2^2)
//! primary  = NUMBER UNIT? | name | "series" "." name | "derived" "." name
//!          | IDENT "(" args? ")" | "(" expr ")"
//! args     = arg ("," arg)*                positional before named
//! arg      = IDENT "=" expr | expr
//! name     = IDENT | `QUOTED`
//! ```
//!
//! [`parse_attributes`] reads the expressions of a chart that compute with
//! attributes instead of metrics (`param.width * param.height`, ADR 0022):
//! `primary` then takes `IDENT ("." name)+` as an attribute, and only the
//! scalar functions. Metric expressions keep rejecting attribute names.

use super::ast::{BinOp, Expr, ExprKind, NamedArg};
use super::lexer::{Token, tokenize};
use super::{Diagnostic, MAX_DEPTH, MAX_SOURCE_LEN, Span};

/// Parses an expression.
pub fn parse(src: &str) -> Result<Expr, Diagnostic> {
    parse_with(src, false)
}

/// The functions an expression over attributes can call: those that take
/// and give scalars.
pub const SCALAR_FUNCTIONS: &[&str] = &["abs", "log10", "sqrt", "min", "max", "clamp"];

/// Parses an expression over attributes (`param.width * param.height`): its
/// names are attributes, not metrics, and it calls only
/// [`SCALAR_FUNCTIONS`].
pub fn parse_attributes(src: &str) -> Result<Expr, Diagnostic> {
    parse_with(src, true)
}

fn parse_with(src: &str, attributes: bool) -> Result<Expr, Diagnostic> {
    if src.len() > MAX_SOURCE_LEN {
        return Err(Diagnostic::new(
            Span::new(0, src.len()),
            format!("expression is longer than {MAX_SOURCE_LEN} bytes"),
        ));
    }
    if src.trim().is_empty() {
        return Err(Diagnostic::new(
            Span::new(0, src.len()),
            "expression is empty",
        ));
    }
    let tokens = tokenize(src)?;
    let mut p = Parser {
        tokens,
        pos: 0,
        depth: 0,
        attributes,
    };
    let expr = p.expr()?;
    let (token, span) = p.peek();
    if *token != Token::Eof {
        return Err(Diagnostic::new(
            *span,
            format!("unexpected {} after the expression", token.describe()),
        ));
    }
    // Long chains such as a+b+c+... are deep without nesting; checking and
    // evaluation recurse, so bound the depth of the tree too.
    if tree_depth(&expr) > MAX_TREE_DEPTH {
        return Err(Diagnostic::new(
            expr.span,
            format!("expression has more than {MAX_TREE_DEPTH} levels of operations"),
        ));
    }
    Ok(expr)
}

const MAX_TREE_DEPTH: usize = 256;

fn tree_depth(e: &Expr) -> usize {
    1 + match &e.kind {
        ExprKind::Number { .. }
        | ExprKind::Metric(_)
        | ExprKind::Series(_)
        | ExprKind::Derived(_)
        | ExprKind::Attribute(_) => 0,
        ExprKind::Neg(inner) => tree_depth(inner),
        ExprKind::Binary { lhs, rhs, .. } => tree_depth(lhs).max(tree_depth(rhs)),
        ExprKind::Call { args, named, .. } => args
            .iter()
            .chain(named.iter().map(|n| &n.value))
            .map(tree_depth)
            .max()
            .unwrap_or(0),
    }
}

struct Parser {
    tokens: Vec<(Token, Span)>,
    pos: usize,
    depth: usize,
    /// Names are attributes, not metrics ([`parse_attributes`]).
    attributes: bool,
}

/// A metric name as written in backquotes, without the spaces around it.
/// (A parameter key is read verbatim, spaces included.)
fn metric_name(quoted: &str, span: Span) -> Result<String, Diagnostic> {
    let name = quoted.trim();
    if name.is_empty() {
        return Err(Diagnostic::new(span, "empty metric name"));
    }
    Ok(name.to_owned())
}

impl Parser {
    fn peek(&self) -> &(Token, Span) {
        &self.tokens[self.pos]
    }

    fn peek_at(&self, offset: usize) -> &Token {
        &self.tokens[(self.pos + offset).min(self.tokens.len() - 1)].0
    }

    fn bump(&mut self) -> (Token, Span) {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, token: Token, what: &str) -> Result<Span, Diagnostic> {
        let (t, span) = self.peek().clone();
        if t == token {
            self.bump();
            Ok(span)
        } else {
            Err(Diagnostic::new(
                span,
                format!("expected {what}, found {}", t.describe()),
            ))
        }
    }

    fn enter(&mut self) -> Result<(), Diagnostic> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Diagnostic::new(
                self.peek().1,
                format!("expression is nested deeper than {MAX_DEPTH} levels"),
            ));
        }
        Ok(())
    }

    fn expr(&mut self) -> Result<Expr, Diagnostic> {
        self.enter()?;
        let mut lhs = self.product()?;
        loop {
            let op = match self.peek().0 {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.product()?;
            lhs = binary(op, lhs, rhs);
        }
        self.depth -= 1;
        Ok(lhs)
    }

    fn product(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.unary()?;
        loop {
            let op = match self.peek().0 {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => break,
            };
            self.bump();
            let rhs = self.unary()?;
            lhs = binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.peek().0 == Token::Minus {
            self.enter()?;
            let (_, span) = self.bump();
            let inner = self.unary()?;
            self.depth -= 1;
            let span = span.to(inner.span);
            return Ok(Expr {
                kind: ExprKind::Neg(Box::new(inner)),
                span,
            });
        }
        self.power()
    }

    fn power(&mut self) -> Result<Expr, Diagnostic> {
        let base = self.primary()?;
        if self.peek().0 == Token::Caret {
            self.enter()?;
            self.bump();
            let exp = self.unary()?;
            self.depth -= 1;
            return Ok(binary(BinOp::Pow, base, exp));
        }
        Ok(base)
    }

    fn name(&mut self, what: &str) -> Result<(String, Span), Diagnostic> {
        match self.bump() {
            (Token::Ident(s) | Token::Quoted(s), span) => Ok((s, span)),
            (t, span) => Err(Diagnostic::new(
                span,
                format!("expected {what}, found {}", t.describe()),
            )),
        }
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let (token, span) = self.peek().clone();
        match token {
            Token::Number(value) => {
                self.bump();
                if let (Token::Unit(unit), unit_span) = self.peek().clone() {
                    self.bump();
                    return Ok(Expr {
                        kind: ExprKind::Number {
                            value,
                            unit: Some(unit),
                        },
                        span: span.to(unit_span),
                    });
                }
                Ok(Expr {
                    kind: ExprKind::Number { value, unit: None },
                    span,
                })
            }
            Token::Quoted(name) if self.attributes => Err(not_an_attribute(&name, span)),
            Token::Quoted(name) => {
                self.bump();
                Ok(Expr {
                    kind: ExprKind::Metric(metric_name(&name, span)?),
                    span,
                })
            }
            Token::Ident(name) if self.attributes && self.peek_at(1) != &Token::LParen => {
                self.bump();
                self.attribute(name, span)
            }
            Token::Ident(name) => {
                self.bump();
                match self.peek().0 {
                    Token::LParen => self.call(name, span),
                    Token::Dot => {
                        self.bump();
                        let (inner, inner_span) = self.name("a metric name after '.'")?;
                        let inner = metric_name(&inner, inner_span)?;
                        let kind = match name.as_str() {
                            "series" => ExprKind::Series(inner),
                            "derived" => ExprKind::Derived(inner),
                            _ => {
                                return Err(Diagnostic::new(
                                    span,
                                    format!(
                                        "unknown prefix {name:?}: use series.<name> or \
                                         derived.<name>, or quote names with dots in backticks"
                                    ),
                                ));
                            }
                        };
                        Ok(Expr {
                            kind,
                            span: span.to(inner_span),
                        })
                    }
                    _ => Ok(Expr {
                        kind: ExprKind::Metric(name),
                        span,
                    }),
                }
            }
            Token::LParen => {
                self.bump();
                let inner = self.expr()?;
                self.expect(Token::RParen, "')'")?;
                Ok(inner)
            }
            Token::Unit(_) => Err(Diagnostic::new(
                span,
                "a unit in brackets must follow a number, e.g. 1.5[s]",
            )),
            other => Err(Diagnostic::new(
                span,
                format!(
                    "expected a number, a metric or '(', found {}",
                    other.describe()
                ),
            )),
        }
    }

    /// An attribute name after its first part: `param.width`, `run.id`.
    fn attribute(&mut self, first: String, span: Span) -> Result<Expr, Diagnostic> {
        if self.peek().0 != Token::Dot {
            return Err(not_an_attribute(&first, span));
        }
        if first == "series" || first == "derived" {
            return Err(Diagnostic::new(
                span,
                format!(
                    "{first}.<name> is a metric; this expression computes with attributes such as \
                     param.width"
                ),
            ));
        }
        let mut name = first;
        let mut end = span;
        while self.peek().0 == Token::Dot {
            self.bump();
            let (part, part_span) = self.name("an attribute name after '.'")?;
            name.push('.');
            name.push_str(&part);
            end = part_span;
        }
        Ok(Expr {
            kind: ExprKind::Attribute(name),
            span: span.to(end),
        })
    }

    fn call(&mut self, name: String, name_span: Span) -> Result<Expr, Diagnostic> {
        if self.attributes && !SCALAR_FUNCTIONS.contains(&name.as_str()) {
            return Err(Diagnostic::new(
                name_span,
                format!(
                    "{name} is not a function of attributes; use {}",
                    SCALAR_FUNCTIONS.join(", ")
                ),
            ));
        }
        self.enter()?;
        self.expect(Token::LParen, "'('")?;
        let mut args = Vec::new();
        let mut named: Vec<NamedArg> = Vec::new();
        if self.peek().0 != Token::RParen {
            loop {
                if let (Token::Ident(arg), arg_span) = self.peek().clone()
                    && *self.peek_at(1) == Token::Eq
                {
                    if self.attributes {
                        return Err(Diagnostic::new(
                            arg_span,
                            format!("{name} takes no named arguments here"),
                        ));
                    }
                    self.bump();
                    self.bump();
                    let value = self.expr()?;
                    if named.iter().any(|n| n.name == arg) {
                        return Err(Diagnostic::new(
                            arg_span,
                            format!("argument {arg} is given twice"),
                        ));
                    }
                    named.push(NamedArg {
                        name: arg,
                        span: arg_span.to(value.span),
                        value,
                    });
                } else {
                    let value = self.expr()?;
                    if !named.is_empty() {
                        return Err(Diagnostic::new(
                            value.span,
                            "positional arguments must come before named ones",
                        ));
                    }
                    args.push(value);
                }
                if self.peek().0 == Token::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        let close = self.expect(Token::RParen, "',' or ')'")?;
        self.depth -= 1;
        // The checker counts arguments too, but an expression over
        // attributes may be checked only when it runs, with the units of a
        // frame; count them here so that validation reports a wrong count.
        if self.attributes {
            let (lo, hi) = match name.as_str() {
                "abs" | "log10" | "sqrt" => (1, 1),
                // With one argument they reduce a series; attributes are
                // scalars.
                "min" | "max" => (2, 16),
                _ => (3, 3),
            };
            if !(lo..=hi).contains(&args.len()) {
                let expected = match (lo, hi) {
                    (1, 1) => "1 argument".to_owned(),
                    (lo, hi) if lo == hi => format!("{lo} arguments"),
                    (lo, hi) => format!("{lo} to {hi} arguments"),
                };
                return Err(Diagnostic::new(
                    name_span.to(close),
                    format!("{name} takes {expected}, not {}", args.len()),
                ));
            }
        }
        Ok(Expr {
            kind: ExprKind::Call {
                name,
                name_span,
                args,
                named,
            },
            span: name_span.to(close),
        })
    }
}

fn not_an_attribute(name: &str, span: Span) -> Diagnostic {
    Diagnostic::new(
        span,
        format!(
            "{name} is not an attribute; this expression computes with attributes such as \
             param.width, not with metrics"
        ),
    )
}

fn binary(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    let span = lhs.span.to(rhs.span);
    Expr {
        kind: ExprKind::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        span,
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    /// Renders the tree with explicit parentheses.
    fn show(e: &Expr) -> String {
        match &e.kind {
            ExprKind::Number { value, unit: None } => format!("{value}"),
            ExprKind::Number {
                value,
                unit: Some(u),
            } => format!("{value}[{u}]"),
            ExprKind::Metric(n) => n.clone(),
            ExprKind::Series(n) => format!("series.{n}"),
            ExprKind::Derived(n) => format!("derived.{n}"),
            ExprKind::Attribute(n) => format!("@{n}"),
            ExprKind::Neg(e) => format!("(-{})", show(e)),
            ExprKind::Binary { op, lhs, rhs } => {
                format!("({} {} {})", show(lhs), op.symbol(), show(rhs))
            }
            ExprKind::Call {
                name, args, named, ..
            } => {
                let mut parts: Vec<String> = args.iter().map(show).collect();
                parts.extend(
                    named
                        .iter()
                        .map(|n| format!("{}={}", n.name, show(&n.value))),
                );
                format!("{name}({})", parts.join(", "))
            }
        }
    }

    fn p(src: &str) -> String {
        show(&parse(src).unwrap())
    }

    #[test]
    fn precedence_and_associativity() {
        assert_eq!(p("a + b * c"), "(a + (b * c))");
        assert_eq!(p("a - b - c"), "((a - b) - c)");
        assert_eq!(p("a / b / c"), "((a / b) / c)");
        assert_eq!(p("-2^2"), "(-(2 ^ 2))");
        assert_eq!(p("2^3^2"), "(2 ^ (3 ^ 2))");
        assert_eq!(p("2^-1"), "(2 ^ (-1))");
        assert_eq!(p("(a + b) * c"), "((a + b) * c)");
        assert_eq!(p("--a"), "(-(-a))");
    }

    #[test]
    fn references_and_calls() {
        assert_eq!(p("bytes / time"), "(bytes / time)");
        assert_eq!(p("max(series.fps)"), "max(series.fps)");
        assert_eq!(
            p("time_to(series.loaded, 0.9, from=t0, interp=step)"),
            "time_to(series.loaded, 0.9, from=t0, interp=step)"
        );
        assert_eq!(p("derived.throughput * 8"), "(derived.throughput * 8)");
        assert_eq!(p("`load-time` + 1[ms]"), "(load-time + 1[ms])");
        assert_eq!(p("series.`frame time`"), "series.frame time");
        assert_eq!(p("f()"), "f()");

        let e = parse("rekey(series.q, series.bytes, interp=linear) + b").unwrap();
        let refs = e.references();
        assert_eq!(refs.series.len(), 2);
        assert_eq!(refs.metrics.into_iter().collect::<Vec<_>>(), vec!["b"]);
    }

    #[test]
    fn errors_point_at_the_problem() {
        let err = |src: &str| parse(src).unwrap_err();
        assert_eq!(err("a +").span, Span::new(3, 3));
        assert_eq!(err("a b").span, Span::new(2, 3));
        assert_eq!(err("foo.bar").span, Span::new(0, 3));
        assert!(err("max(a, from=1, b)").message.contains("positional"));
        assert!(err("max(s, from=1, from=2)").message.contains("twice"));
        assert!(err("(a + b").message.contains("')'"));
        assert!(err("[s]").message.contains("must follow a number"));
        assert!(err("").message.contains("empty"));
        let deep = format!("{}a{}", "(".repeat(100), ")".repeat(100));
        assert!(err(&deep).message.contains("nested deeper"));
        let long = "a+".repeat(3000) + "a";
        assert!(err(&long).message.contains("longer than"));
        let chain = "a+".repeat(300) + "a";
        assert!(err(&chain).message.contains("levels of operations"));
    }

    #[test]
    fn attribute_expressions_read_attributes_not_metrics() {
        let a = |src: &str| show(&parse_attributes(src).unwrap());
        assert_eq!(
            a("param.width * param.height"),
            "(@param.width * @param.height)"
        );
        assert_eq!(a("abs(sut.id - 1)"), "abs((@sut.id - 1))");
        assert_eq!(
            a("param.`frame size` / 2[px]"),
            "(@param.frame size / 2[px])"
        );
        assert_eq!(a("b.param.width"), "@b.param.width");
        // Parameter keys are read verbatim; metric names are trimmed.
        assert_eq!(a("param.` width`"), "@param. width");
        assert_eq!(p("series.` fps `"), "series.fps");
        assert_eq!(p("` load time `"), "load time");
        assert_eq!(a("param.` `"), "@param. ");
        assert!(parse("` `").is_err());
        assert!(parse("series.` `").is_err());
        let e = parse_attributes("max(param.w, param.h) + param.w").unwrap();
        let names: Vec<String> = e.attributes().into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["param.w", "param.h", "param.w"]);
        assert!(e.references().metrics.is_empty());

        let err = |src: &str| parse_attributes(src).unwrap_err();
        assert!(err("load_time * 2").message.contains("not an attribute"));
        assert_eq!(err("2 * `load time`").span, Span::new(4, 15));
        assert!(err("series.fps").message.contains("is a metric"));
        assert!(err("derived.x + 1").message.contains("is a metric"));
        assert!(
            err("rate(param.w)")
                .message
                .contains("not a function of attributes")
        );
        assert!(
            err("max(param.w, from=1)")
                .message
                .contains("no named arguments")
        );

        // Metric expressions keep rejecting attribute names.
        assert!(
            parse("param.width * 2")
                .unwrap_err()
                .message
                .contains("unknown prefix")
        );
    }

    proptest! {
        #[test]
        fn parser_never_panics(src in "[a-z0-9_ .,()+*/^=`\\[\\]-]{0,80}") {
            let _ = parse(&src);
            let _ = parse_attributes(&src);
        }

        #[test]
        fn parser_never_panics_on_unicode(src in "\\PC{0,40}") {
            let _ = parse(&src);
        }
    }
}
