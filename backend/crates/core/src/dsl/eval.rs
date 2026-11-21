//! Evaluation of a checked expression on the inputs of one sample.
//!
//! Values are `Option<f64>`: `None` is "no value" (FR-12a) and propagates
//! through every operation. A non-finite scalar result (e.g. division by
//! zero) is no value too. Inputs that a sample does not have are not "no
//! value" but missing: evaluation stops with [`EvalError::Missing`].

use super::ast::BinOp;
use super::check::{Conv, Func, Node, Reduction};
use crate::series::{PointReduction, Series};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Scalar(Option<f64>),
    Series(Option<Series>),
}

impl Value {
    pub fn as_scalar(&self) -> Option<f64> {
        match self {
            Value::Scalar(v) => *v,
            Value::Series(_) => None,
        }
    }

    /// Multiplies the value (y for a series) by a factor.
    pub fn scale(self, factor: f64) -> Value {
        if factor == 1.0 {
            return self;
        }
        match self {
            Value::Scalar(v) => Value::Scalar(finite(v.map(|v| v * factor))),
            Value::Series(s) => Value::Series(s.map(|s| s.map(|y| y * factor))),
        }
    }

    fn scale_x(self, factor: f64) -> Value {
        match self {
            Value::Series(Some(s)) if factor != 1.0 => Value::Series(Some(s.scale_x(factor))),
            other => other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// The sample has no value for this input (it was not measured).
    Missing(String),
    /// The inputs do not fit the expression, e.g. series over different x.
    Data(String),
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::Missing(name) => write!(f, "the sample has no {name}"),
            EvalError::Data(message) => f.write_str(message),
        }
    }
}

/// Inputs of one sample. Values are in the units the expression was checked
/// with.
pub trait Inputs {
    fn metric(&mut self, name: &str) -> Result<f64, EvalError>;
    fn series(&mut self, name: &str) -> Result<Series, EvalError>;
    /// The value of a derived metric, in its declared unit.
    fn derived(&mut self, name: &str) -> Result<Value, EvalError>;
}

fn finite(v: Option<f64>) -> Option<f64> {
    v.filter(|v| v.is_finite())
}

type R = Result<Value, EvalError>;

pub fn eval(node: &Node, inputs: &mut dyn Inputs) -> R {
    match node {
        Node::Const(v) => Ok(Value::Scalar(Some(*v))),
        Node::Metric(name) => Ok(Value::Scalar(finite(Some(inputs.metric(name)?)))),
        Node::Series(name) => Ok(Value::Series(Some(inputs.series(name)?))),
        Node::Derived(name) => inputs.derived(name),
        Node::Neg(inner) => Ok(map(eval(inner, inputs)?, |v| -v)),
        Node::Binary { op, lhs, rhs } => {
            let a = conv(lhs, inputs)?;
            let b = conv(rhs, inputs)?;
            let f: fn(f64, f64) -> f64 = match op {
                BinOp::Add => |a, b| a + b,
                BinOp::Sub => |a, b| a - b,
                BinOp::Mul => |a, b| a * b,
                BinOp::Div => |a, b| a / b,
                BinOp::Pow => f64::powf,
            };
            pointwise(a, b, f)
        }
        Node::Func(func, inner) => {
            let f: fn(f64) -> f64 = match func {
                Func::Abs => f64::abs,
                Func::Log10 => f64::log10,
                Func::Sqrt => f64::sqrt,
            };
            Ok(map(eval(inner, inputs)?, f))
        }
        Node::Extreme { max, args } => {
            let f: fn(f64, f64) -> f64 = if *max { max_nan } else { min_nan };
            let mut acc: Option<Value> = None;
            for arg in args {
                let v = conv(arg, inputs)?;
                acc = Some(match acc {
                    None => v,
                    Some(a) => pointwise(a, v, f)?,
                });
            }
            Ok(acc.expect("at least two arguments"))
        }
        Node::Clamp { value, lo, hi } => {
            let v = conv(value, inputs)?;
            let lo = conv(lo, inputs)?;
            let hi = conv(hi, inputs)?;
            pointwise(pointwise(v, lo, max_nan)?, hi, min_nan)
        }
        Node::Reduce {
            op,
            series,
            from,
            to,
            interp,
        } => {
            let s = series_value(series, inputs)?;
            let from = bound(from, inputs)?;
            let to = bound(to, inputs)?;
            let (Some(s), Some(from), Some(to)) = (s, from, to) else {
                return Ok(Value::Scalar(None));
            };
            let v = match op {
                Reduction::Min => s.reduce(PointReduction::Min, from, to),
                Reduction::Max => s.reduce(PointReduction::Max, from, to),
                Reduction::Mean => s.reduce(PointReduction::Mean, from, to),
                Reduction::Auc => s.auc(from, to, *interp),
            };
            Ok(Value::Scalar(finite(v)))
        }
        Node::TimeTo {
            series,
            threshold,
            from,
            interp,
        } => {
            let s = series_value(series, inputs)?;
            let threshold = conv(threshold, inputs)?.as_scalar();
            let from = bound(from, inputs)?;
            let (Some(s), Some(threshold), Some(from)) = (s, threshold, from) else {
                return Ok(Value::Scalar(None));
            };
            Ok(Value::Scalar(finite(s.time_to(threshold, from, *interp))))
        }
        Node::Rate(inner) => series_map(inner, inputs, |s| Ok(s.rate())),
        Node::Cumsum(inner) => series_map(inner, inputs, |s| Ok(s.cumsum())),
        Node::Log10X(inner) => series_map(inner, inputs, |s| Ok(s.log10_x())),
        Node::MovingAvg { series, window } => {
            let window = conv(window, inputs)?.as_scalar();
            let Some(window) = window else {
                return Ok(Value::Series(None));
            };
            series_map(series, inputs, |s| {
                s.moving_avg(window).ok_or_else(|| {
                    EvalError::Data(format!(
                        "the moving average window {window} is not positive"
                    ))
                })
            })
        }
        Node::Rekey { series, by, interp } => {
            let s = series_value(series, inputs)?;
            let by = match conv(by, inputs)? {
                Value::Series(s) => s,
                Value::Scalar(_) => unreachable!("checked as a series"),
            };
            let (Some(s), Some(by)) = (s, by) else {
                return Ok(Value::Series(None));
            };
            s.rekey(&by, *interp)
                .map(|s| Value::Series(Some(s)))
                .map_err(EvalError::Data)
        }
    }
}

fn conv(c: &Conv, inputs: &mut dyn Inputs) -> R {
    Ok(eval(&c.node, inputs)?.scale(c.factor).scale_x(c.x_factor))
}

fn bound(c: &Option<Conv>, inputs: &mut dyn Inputs) -> Result<Option<Option<f64>>, EvalError> {
    match c {
        None => Ok(Some(None)),
        Some(c) => Ok(conv(c, inputs)?.as_scalar().map(Some)),
    }
}

fn series_value(node: &Node, inputs: &mut dyn Inputs) -> Result<Option<Series>, EvalError> {
    match eval(node, inputs)? {
        Value::Series(s) => Ok(s),
        Value::Scalar(_) => unreachable!("checked as a series"),
    }
}

fn series_map(
    node: &Node,
    inputs: &mut dyn Inputs,
    f: impl FnOnce(&Series) -> Result<Series, EvalError>,
) -> R {
    Ok(Value::Series(match series_value(node, inputs)? {
        Some(s) => Some(f(&s)?),
        None => None,
    }))
}

fn map(v: Value, f: impl Fn(f64) -> f64) -> Value {
    match v {
        Value::Scalar(v) => Value::Scalar(finite(v.map(f))),
        Value::Series(s) => Value::Series(s.map(|s| s.map(f))),
    }
}

/// min/max where a missing point (NaN) stays missing.
fn min_nan(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

fn max_nan(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

fn pointwise(a: Value, b: Value, f: fn(f64, f64) -> f64) -> R {
    Ok(match (a, b) {
        (Value::Scalar(a), Value::Scalar(b)) => Value::Scalar(match (a, b) {
            (Some(a), Some(b)) => finite(Some(f(a, b))),
            _ => None,
        }),
        (Value::Series(s), Value::Scalar(b)) => Value::Series(match (s, b) {
            (Some(s), Some(b)) => Some(s.map(|a| f(a, b))),
            _ => None,
        }),
        (Value::Scalar(a), Value::Series(s)) => Value::Series(match (a, s) {
            (Some(a), Some(s)) => Some(s.map(|b| f(a, b))),
            _ => None,
        }),
        (Value::Series(a), Value::Series(b)) => Value::Series(match (a, b) {
            (Some(a), Some(b)) => Some(a.zip(&b, f).ok_or_else(|| {
                EvalError::Data(
                    "point by point math needs series with the same x values".to_owned(),
                )
            })?),
            _ => None,
        }),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::dsl::check::check;
    use crate::dsl::check::tests::env;
    use crate::dsl::parse;
    use crate::units::tests::catalogue;

    #[derive(Default)]
    struct TestInputs {
        metrics: HashMap<String, f64>,
        series: HashMap<String, Series>,
        derived: HashMap<String, Value>,
    }

    impl Inputs for TestInputs {
        fn metric(&mut self, name: &str) -> Result<f64, EvalError> {
            self.metrics
                .get(name)
                .copied()
                .ok_or_else(|| EvalError::Missing(name.into()))
        }
        fn series(&mut self, name: &str) -> Result<Series, EvalError> {
            self.series
                .get(name)
                .cloned()
                .ok_or_else(|| EvalError::Missing(name.into()))
        }
        fn derived(&mut self, name: &str) -> Result<Value, EvalError> {
            self.derived
                .get(name)
                .cloned()
                .ok_or_else(|| EvalError::Missing(name.into()))
        }
    }

    fn inputs() -> TestInputs {
        let mut i = TestInputs::default();
        i.metrics.insert("bytes".into(), 4_000_000.0);
        i.metrics.insert("time".into(), 2.0);
        i.metrics.insert("load_ms".into(), 500.0);
        i.metrics.insert("score".into(), -0.5);
        i.metrics.insert("t0".into(), 1.0);
        let t = vec![0.0, 1.0, 2.0, 3.0];
        i.series.insert(
            "loaded".into(),
            Series::new(t.clone(), vec![0.0, 50.0, 80.0, 100.0]),
        );
        i.series.insert(
            "fps".into(),
            Series::new(t.clone(), vec![10.0, 30.0, 60.0, 50.0]),
        );
        i.series.insert(
            "quality".into(),
            Series::new(t.clone(), vec![0.1, 0.2, 0.4, 0.8]),
        );
        i.series.insert(
            "late".into(),
            Series::new(vec![0.0, 1000.0, 2000.0, 3000.0], vec![1.0, 1.0, 1.0, 1.0]),
        );
        i.series.insert(
            "received".into(),
            Series::new(t, vec![0.0, 100.0, 300.0, 600.0]),
        );
        i.derived
            .insert("throughput".into(), Value::Scalar(Some(2.0)));
        i
    }

    fn run(src: &str, declared: Option<&str>) -> Result<Value, EvalError> {
        let c = check(&parse(src).unwrap(), &mut env(), &catalogue(), declared).unwrap();
        Ok(eval(&c.node, &mut inputs())?.scale(c.output_factor))
    }

    fn scalar(src: &str, declared: Option<&str>) -> Option<f64> {
        match run(src, declared).unwrap() {
            Value::Scalar(v) => v,
            other => panic!("{src}: {other:?}"),
        }
    }

    fn series(src: &str) -> Series {
        match run(src, None).unwrap() {
            Value::Series(Some(s)) => s,
            other => panic!("{src}: {other:?}"),
        }
    }

    #[test]
    fn scalar_arithmetic() {
        assert_eq!(scalar("bytes / time", Some("MB/s")), Some(2.0));
        assert_eq!(scalar("time + load_ms", Some("ms")), Some(2500.0));
        assert_eq!(scalar("time + 500[ms]", None), Some(2.5));
        assert_eq!(scalar("-time^2 + 1", None), Some(-3.0));
        assert_eq!(scalar("clamp(score, 0, 1)", None), Some(0.0));
        assert_eq!(scalar("abs(score) * 2", None), Some(1.0));
        assert_eq!(scalar("log10(bytes)", None), Some(4_000_000f64.log10()));
        assert_eq!(scalar("max(time, load_ms, 1)", None), Some(2.0));
        assert_eq!(scalar("min(time, load_ms)", Some("ms")), Some(500.0));
        assert_eq!(scalar("derived.throughput * 2", None), Some(4.0));
    }

    #[test]
    fn no_value_propagates() {
        assert_eq!(scalar("time / 0", None), None, "division by zero");
        assert_eq!(scalar("log10(score)", None), None);
        assert_eq!(
            scalar("time_to(series.loaded, 101)", None),
            None,
            "never reached"
        );
        assert_eq!(scalar("time_to(series.loaded, 101) + 1", None), None);
        assert_eq!(scalar("max(time_to(series.loaded, 101), time)", None), None);
        assert_eq!(
            scalar("mean(series.fps, from=time_to(series.loaded, 101))", None),
            None
        );
    }

    #[test]
    fn missing_inputs_stop_evaluation() {
        let c = check(
            &parse("bytes / time").unwrap(),
            &mut env(),
            &catalogue(),
            None,
        )
        .unwrap();
        let mut i = inputs();
        i.metrics.remove("bytes");
        assert_eq!(
            eval(&c.node, &mut i),
            Err(EvalError::Missing("bytes".into()))
        );
    }

    #[test]
    fn reductions_and_time_to() {
        assert_eq!(scalar("max(series.fps)", None), Some(60.0));
        assert_eq!(scalar("min(series.fps, from=1)", None), Some(30.0));
        assert_eq!(scalar("mean(series.fps, to=1)", None), Some(20.0));
        assert_eq!(scalar("auc(series.fps)", None), Some(20.0 + 45.0 + 55.0));
        assert_eq!(
            scalar("auc(series.fps, interp=step, to=4)", None),
            Some(10.0 + 30.0 + 60.0 + 50.0)
        );
        assert_eq!(scalar("time_to(series.loaded, 90)", None), Some(2.5));
        assert_eq!(
            scalar("time_to(series.loaded, 90)", Some("ms")),
            Some(2500.0)
        );
        assert_eq!(scalar("time_to(series.loaded, 0.9[])", None), Some(2.5));
        assert_eq!(
            scalar("time_to(series.loaded, 90, interp=step)", None),
            Some(3.0)
        );
        assert_eq!(
            scalar("time_to(series.loaded, 10, from=t0)", None),
            Some(1.0)
        );
        assert_eq!(
            scalar("time_to(series.loaded, 10, from=1500[ms])", None),
            Some(1.5)
        );
    }

    #[test]
    fn transforms_and_pointwise() {
        assert_eq!(series("rate(series.received)").y, vec![100.0, 200.0, 300.0]);
        assert_eq!(
            series("cumsum(series.fps)").y,
            vec![10.0, 40.0, 100.0, 150.0]
        );
        assert_eq!(
            series("moving_avg(series.fps, 1500[ms])").y,
            vec![10.0, 20.0, 45.0, 55.0]
        );
        assert_eq!(series("series.fps / 10").y, vec![1.0, 3.0, 6.0, 5.0]);
        assert_eq!(series("1 - series.quality").y.len(), 4);
        // late is over ms; it is converted to s before matching x.
        assert_eq!(
            series("series.quality * series.late").y,
            vec![0.1, 0.2, 0.4, 0.8]
        );
        let r = series("rekey(series.quality, series.received)");
        assert_eq!(r.x, vec![0.0, 100.0, 300.0, 600.0]);
        assert_eq!(
            series("max(series.quality, 0.3)").y,
            vec![0.3, 0.3, 0.4, 0.8]
        );
        assert_eq!(
            scalar("max(rate(series.received))", Some("B/s")),
            Some(300.0)
        );
    }

    #[test]
    fn data_errors() {
        let c = check(
            &parse("series.quality + series.fps").unwrap(),
            &mut env(),
            &catalogue(),
            None,
        );
        assert!(c.is_err(), "fps and quality units differ");
        let c = check(
            &parse("series.quality * series.fps").unwrap(),
            &mut env(),
            &catalogue(),
            None,
        )
        .unwrap();
        let mut i = inputs();
        i.series
            .insert("fps".into(), Series::new(vec![0.0, 1.0], vec![1.0, 2.0]));
        assert!(matches!(eval(&c.node, &mut i), Err(EvalError::Data(_))));
        let c = check(
            &parse("moving_avg(series.fps, score)").unwrap(),
            &mut env(),
            &catalogue(),
            None,
        );
        assert!(
            c.is_err(),
            "score is dimensionless, the window must be in s"
        );
    }
}
