//! Composition of derived metrics (FR-17) and their evaluation per sample.
//!
//! A [`Library`] holds derived metrics by name. A derived metric has one
//! definition (ADR 0017): `derived.<name>` in an expression means that
//! definition, so changing a derived metric also changes the metrics built
//! on it. A [`Root`] is what is evaluated: a saved derived metric or an
//! ad-hoc expression.
//!
//! An [`Evaluator`] checks the root and everything it uses against the
//! units of each sample's inputs (cached per combination of units) and
//! evaluates it. The result is converted to the root's unit, so samples
//! whose metrics were uploaded in different units give comparable values.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use super::ast::{Expr, References};
use super::check::{Checked, Env, SeriesUnits, Signature, check};
use super::eval::{EvalError, Inputs, Value, eval};
use super::{Diagnostic, MAX_COMPOSITION_DEPTH, parse};
use crate::series::Series;
use crate::units::UnitCatalogue;

/// A saved derived metric.
#[derive(Debug, Clone)]
pub struct Definition {
    pub name: String,
    pub expression: String,
    /// The declared unit; every value is converted to it.
    pub unit: String,
}

#[derive(Debug, Clone)]
struct Entry {
    def: Definition,
    expr: Result<Expr, Diagnostic>,
}

/// Derived metrics by name.
#[derive(Debug, Clone, Default)]
pub struct Library {
    entries: HashMap<String, Entry>,
}

impl Library {
    pub fn new(defs: impl IntoIterator<Item = Definition>) -> Self {
        let mut library = Self::default();
        for def in defs {
            library.insert(def);
        }
        library
    }

    /// Adds a derived metric, replacing the one of the same name.
    pub fn insert(&mut self, def: Definition) {
        let expr = parse(&def.expression);
        self.entries.insert(def.name.clone(), Entry { def, expr });
    }

    /// Adds the derived metrics of `other`, replacing those of the same name.
    pub fn merge(&mut self, other: &Library) {
        self.entries
            .extend(other.entries.iter().map(|(k, v)| (k.clone(), v.clone())));
    }

    pub fn get(&self, name: &str) -> Option<&Definition> {
        self.entries.get(name).map(|e| &e.def)
    }

    fn entry(&self, name: &str) -> Result<&Entry, String> {
        self.entries
            .get(name)
            .ok_or_else(|| format!("unknown derived metric {name}"))
    }

    /// The derived metrics the given ones use, directly or through others,
    /// by name. Names that are not in the library are included, and so are
    /// the given names if something they use leads back to them.
    pub fn uses_closure(&self, names: impl IntoIterator<Item = String>) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack: Vec<String> = Vec::new();
        for name in names {
            stack.extend(self.direct_uses(&name));
        }
        while let Some(name) = stack.pop() {
            if seen.insert(name.clone()) {
                stack.extend(self.direct_uses(&name));
            }
        }
        seen
    }

    fn direct_uses(&self, name: &str) -> BTreeSet<String> {
        match self.entries.get(name) {
            Some(Entry { expr: Ok(expr), .. }) => expr.references().derived,
            _ => BTreeSet::new(),
        }
    }
}

/// What to evaluate.
#[derive(Debug, Clone)]
pub struct Root {
    pub expr: Expr,
    /// Output unit; `None` keeps the unit the expression has.
    pub unit: Option<String>,
}

impl Root {
    /// A saved derived metric, in its declared unit.
    pub fn definition(library: &Library, name: &str) -> Result<Root, String> {
        let entry = library.entry(name)?;
        let expr = entry
            .expr
            .clone()
            .map_err(|d| format!("derived metric {name} does not parse: {}", d.message))?;
        Ok(Root {
            expr,
            unit: Some(entry.def.unit.clone()),
        })
    }

    /// The raw inputs the root needs, including through the derived metrics
    /// it uses, and the names of those derived metrics.
    pub fn closure(&self, library: &Library) -> Result<(References, BTreeSet<String>), String> {
        let mut raw = References::default();
        let mut used = BTreeSet::new();
        let mut path = Vec::new();
        visit(&self.expr, library, &mut raw, &mut used, &mut path)?;
        Ok((raw, used))
    }
}

/// `path` is the chain of derived metrics being visited: a name that comes
/// up again in it is a cycle. The database rules cycles out on save; this
/// keeps evaluation safe if one exists anyway.
fn visit(
    expr: &Expr,
    library: &Library,
    raw: &mut References,
    used: &mut BTreeSet<String>,
    path: &mut Vec<String>,
) -> Result<(), String> {
    if path.len() > MAX_COMPOSITION_DEPTH {
        return Err(format!(
            "derived metrics are nested deeper than {MAX_COMPOSITION_DEPTH} levels"
        ));
    }
    let refs = expr.references();
    raw.metrics.extend(refs.metrics);
    raw.series.extend(refs.series);
    for name in refs.derived {
        if path.contains(&name) {
            return Err(format!("derived metric {name} uses itself"));
        }
        if !used.insert(name.clone()) {
            continue;
        }
        let entry = library.entry(&name)?;
        let Ok(inner) = &entry.expr else {
            return Err(format!("derived metric {name} does not parse"));
        };
        path.push(name);
        visit(inner, library, raw, used, path)?;
        path.pop();
    }
    Ok(())
}

/// Units of the raw inputs, for checking.
pub trait UnitSource {
    fn metric_unit(&self, name: &str) -> Option<String>;
    fn series_units(&self, name: &str) -> Option<SeriesUnits>;
}

/// The raw inputs of one sample, each with the unit it was uploaded in.
#[derive(Debug, Clone, Default)]
pub struct SampleData {
    pub measurements: HashMap<String, (f64, String)>,
    pub series: HashMap<String, (Series, SeriesUnits)>,
}

impl UnitSource for SampleData {
    fn metric_unit(&self, name: &str) -> Option<String> {
        self.measurements.get(name).map(|(_, u)| u.clone())
    }

    fn series_units(&self, name: &str) -> Option<SeriesUnits> {
        self.series.get(name).map(|(_, u)| u.clone())
    }
}

/// A root and the derived metrics it uses, checked for one combination of
/// input units.
#[derive(Debug, Clone)]
pub struct Compiled {
    pub root: Checked,
    deps: HashMap<String, Checked>,
}

impl Compiled {
    pub fn signature(&self) -> &Signature {
        &self.root.signature
    }
}

struct CompileState<'a> {
    library: &'a Library,
    catalogue: &'a UnitCatalogue,
    source: &'a dyn UnitSource,
    deps: HashMap<String, Result<Checked, String>>,
    /// The derived metrics being checked, outermost first.
    path: Vec<String>,
}

struct CompileEnv<'s, 'a> {
    state: &'s mut CompileState<'a>,
}

impl Env for CompileEnv<'_, '_> {
    fn metric(&self, name: &str) -> Option<String> {
        self.state.source.metric_unit(name)
    }

    fn series(&self, name: &str) -> Option<SeriesUnits> {
        self.state.source.series_units(name)
    }

    fn derived(&mut self, name: &str) -> Result<Signature, String> {
        compile_dep(self.state, name).map(|c| c.signature)
    }
}

fn compile_dep(state: &mut CompileState<'_>, name: &str) -> Result<Checked, String> {
    if let Some(done) = state.deps.get(name) {
        return done.clone();
    }
    if state.path.iter().any(|n| n == name) {
        return Err(format!("derived.{name} uses itself"));
    }
    if state.path.len() >= MAX_COMPOSITION_DEPTH {
        return Err(format!(
            "derived metrics are nested deeper than {MAX_COMPOSITION_DEPTH} levels"
        ));
    }
    let entry = state.library.entry(name)?.clone();
    let result = match &entry.expr {
        Err(d) => Err(format!("derived.{name} does not parse: {}", d.message)),
        Ok(expr) => {
            let catalogue = state.catalogue;
            state.path.push(name.to_owned());
            let checked = {
                let mut env = CompileEnv { state: &mut *state };
                check(expr, &mut env, catalogue, Some(&entry.def.unit))
            };
            state.path.pop();
            checked.map_err(|d| format!("derived.{name} does not apply: {}", d[0].message))
        }
    };
    state.deps.insert(name.to_owned(), result.clone());
    result
}

/// Checks a root against the units of `source`. Errors in derived metrics
/// it uses are reported at the reference.
pub fn compile(
    library: &Library,
    catalogue: &UnitCatalogue,
    root: &Root,
    source: &dyn UnitSource,
) -> Result<Compiled, Vec<Diagnostic>> {
    let mut state = CompileState {
        library,
        catalogue,
        source,
        deps: HashMap::new(),
        path: Vec::new(),
    };
    let checked = {
        let mut env = CompileEnv { state: &mut state };
        check(&root.expr, &mut env, catalogue, root.unit.as_deref())?
    };
    let deps = state
        .deps
        .into_iter()
        .filter_map(|(name, r)| r.ok().map(|c| (name, c)))
        .collect();
    Ok(Compiled {
        root: checked,
        deps,
    })
}

struct SampleInputs<'a> {
    compiled: &'a Compiled,
    sample: &'a SampleData,
    memo: &'a mut HashMap<String, Result<Value, EvalError>>,
}

impl Inputs for SampleInputs<'_> {
    fn metric(&mut self, name: &str) -> Result<f64, EvalError> {
        self.sample
            .measurements
            .get(name)
            .map(|(v, _)| *v)
            .ok_or_else(|| EvalError::Missing(name.to_owned()))
    }

    fn series(&mut self, name: &str) -> Result<Series, EvalError> {
        self.sample
            .series
            .get(name)
            .map(|(s, _)| s.clone())
            .ok_or_else(|| EvalError::Missing(format!("series.{name}")))
    }

    fn derived(&mut self, name: &str) -> Result<Value, EvalError> {
        if let Some(v) = self.memo.get(name) {
            return v.clone();
        }
        let checked = self.compiled.deps.get(name).ok_or_else(|| {
            EvalError::Data(format!("derived.{name} does not apply to this sample"))
        })?;
        let mut inner = SampleInputs {
            compiled: self.compiled,
            sample: self.sample,
            memo: &mut *self.memo,
        };
        let value = eval(&checked.node, &mut inner).map(|v| v.scale(checked.output_factor));
        self.memo.insert(name.to_owned(), value.clone());
        value
    }
}

/// Evaluates a compiled root on a sample whose units it was compiled for.
/// Non-finite points are dropped from a series result.
pub fn evaluate(compiled: &Compiled, sample: &SampleData) -> Result<Value, EvalError> {
    let mut memo = HashMap::new();
    let mut inputs = SampleInputs {
        compiled,
        sample,
        memo: &mut memo,
    };
    let value = eval(&compiled.root.node, &mut inputs)?.scale(compiled.root.output_factor);
    Ok(match value {
        Value::Series(Some(s)) => Value::Series(Some(s.finite())),
        other => other,
    })
}

/// The result for one sample.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub value: Value,
    /// Unit and x of the value. With a root unit, `unit` is that unit; x is
    /// the sample's own x unit.
    pub signature: Signature,
}

/// Evaluates a root on many samples, checking it once per combination of
/// input units.
pub struct Evaluator<'a> {
    library: &'a Library,
    catalogue: &'a UnitCatalogue,
    root: Root,
    raw: References,
    /// By the units of the inputs, in the order of `raw` (metric units, then
    /// unit, x unit and x dimension of each series), so that no two
    /// combinations share a key whatever their units contain.
    cache: HashMap<Vec<String>, Result<Arc<Compiled>, String>>,
}

impl<'a> Evaluator<'a> {
    pub fn new(
        library: &'a Library,
        catalogue: &'a UnitCatalogue,
        root: Root,
    ) -> Result<Self, String> {
        let (raw, _) = root.closure(library)?;
        Ok(Self {
            library,
            catalogue,
            root,
            raw,
            cache: HashMap::new(),
        })
    }

    /// The raw metrics and series the root needs.
    pub fn inputs(&self) -> &References {
        &self.raw
    }

    pub fn evaluate(&mut self, sample: &SampleData) -> Result<Outcome, EvalError> {
        let mut key = Vec::with_capacity(self.raw.metrics.len() + 3 * self.raw.series.len());
        for name in &self.raw.metrics {
            let unit = sample
                .metric_unit(name)
                .ok_or_else(|| EvalError::Missing(name.clone()))?;
            key.push(unit);
        }
        for name in &self.raw.series {
            let u = sample
                .series_units(name)
                .ok_or_else(|| EvalError::Missing(format!("series.{name}")))?;
            key.extend([u.unit, u.x_unit, u.x_dimension]);
        }
        let compiled = self
            .cache
            .entry(key)
            .or_insert_with(|| {
                compile(self.library, self.catalogue, &self.root, sample)
                    .map(Arc::new)
                    .map_err(|d| d[0].message.clone())
            })
            .clone()
            .map_err(EvalError::Data)?;
        Ok(Outcome {
            value: evaluate(&compiled, sample)?,
            signature: compiled.signature().clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::tests::catalogue;

    fn def(name: &str, expression: &str, unit: &str) -> Definition {
        Definition {
            name: name.into(),
            expression: expression.into(),
            unit: unit.into(),
        }
    }

    fn library() -> Library {
        Library::new([
            def("throughput", "bytes / time", "MB/s"),
            def("bits", "derived.throughput", "Mbit/s"),
            def("first_frame", "time_to(series.fps, 1)", "ms"),
            def("smooth", "moving_avg(series.fps, 1)", "fps"),
            def("smooth_max", "max(derived.smooth)", "fps"),
        ])
    }

    fn sample(time_unit: &str, time: f64) -> SampleData {
        let mut s = SampleData::default();
        s.measurements.insert("bytes".into(), (8e6, "B".into()));
        s.measurements
            .insert("time".into(), (time, time_unit.into()));
        s.series.insert(
            "fps".into(),
            (
                Series::new(vec![0.0, 0.5, 1.0], vec![0.0, 0.0, 60.0]),
                SeriesUnits {
                    unit: "fps".into(),
                    x_unit: "s".into(),
                    x_dimension: "time".into(),
                },
            ),
        );
        s
    }

    fn root(src: &str, unit: Option<&str>) -> Root {
        Root {
            expr: parse(src).unwrap(),
            unit: unit.map(Into::into),
        }
    }

    #[test]
    fn a_changed_definition_changes_the_metrics_built_on_it() {
        let mut lib = library();
        let cat = catalogue();
        let bits = |lib: &Library| {
            let mut e = Evaluator::new(lib, &cat, Root::definition(lib, "bits").unwrap()).unwrap();
            e.evaluate(&sample("s", 2.0)).unwrap()
        };
        // 8 MB / 2 s = 4 MB/s, converted to 32 Mbit/s.
        let out = bits(&lib);
        assert_eq!(out.value, Value::Scalar(Some(32.0)));
        assert_eq!(out.signature.unit, "Mbit/s");
        let (raw, used) = Root::definition(&lib, "bits")
            .unwrap()
            .closure(&lib)
            .unwrap();
        assert_eq!(used.into_iter().collect::<Vec<_>>(), vec!["throughput"]);
        assert!(raw.metrics.contains("bytes") && raw.metrics.contains("time"));

        // There are no versions: bits follows the new throughput.
        lib.insert(def("throughput", "bytes / time / 2", "MB/s"));
        assert_eq!(bits(&lib).value, Value::Scalar(Some(16.0)));
    }

    #[test]
    fn inputs_in_other_units_convert() {
        let lib = library();
        let cat = catalogue();
        let mut e = Evaluator::new(&lib, &cat, root("derived.throughput", None)).unwrap();
        assert_eq!(
            e.evaluate(&sample("s", 2.0)).unwrap().value,
            Value::Scalar(Some(4.0))
        );
        assert_eq!(
            e.evaluate(&sample("ms", 2000.0)).unwrap().value,
            Value::Scalar(Some(4.0))
        );
        // An ad-hoc expression in a requested unit.
        let mut e = Evaluator::new(&lib, &cat, root("bytes / time", Some("kB/s"))).unwrap();
        assert_eq!(
            e.evaluate(&sample("ms", 2000.0)).unwrap().value,
            Value::Scalar(Some(4000.0))
        );
        // A unit that does not fit is a data error for that sample.
        assert!(matches!(
            e.evaluate(&sample("B", 1.0)),
            Err(EvalError::Data(_))
        ));
    }

    #[test]
    fn units_with_separators_do_not_share_a_check() {
        let lib = library();
        let cat = catalogue();
        let mut e = Evaluator::new(&lib, &cat, root("series.fps", None)).unwrap();
        let with = |x_unit: &str, x_dimension: &str| {
            let mut s = sample("s", 1.0);
            let units = &mut s.series.get_mut("fps").unwrap().1;
            units.x_unit = x_unit.into();
            units.x_dimension = x_dimension.into();
            s
        };
        let a = e.evaluate(&with("s", "time|ticks")).unwrap();
        let b = e.evaluate(&with("s|time", "ticks")).unwrap();
        assert_eq!(
            (a.signature.x_unit.as_str(), b.signature.x_unit.as_str()),
            ("s", "s|time")
        );
        assert_eq!(b.signature.x_dimension, "ticks");
    }

    #[test]
    fn missing_inputs_and_no_value() {
        let lib = library();
        let cat = catalogue();
        let mut e =
            Evaluator::new(&lib, &cat, Root::definition(&lib, "first_frame").unwrap()).unwrap();
        let v = e
            .evaluate(&sample("s", 1.0))
            .unwrap()
            .value
            .as_scalar()
            .unwrap();
        assert!((v - (0.5 + 0.5 / 60.0) * 1000.0).abs() < 1e-9, "{v}");
        let mut s = sample("s", 1.0);
        s.series.clear();
        assert_eq!(e.evaluate(&s), Err(EvalError::Missing("series.fps".into())));
        let mut e = Evaluator::new(&lib, &cat, root("time_to(series.fps, 100)", None)).unwrap();
        assert_eq!(
            e.evaluate(&sample("s", 1.0)).unwrap().value,
            Value::Scalar(None)
        );
    }

    #[test]
    fn series_composition() {
        let lib = library();
        let cat = catalogue();
        let mut e =
            Evaluator::new(&lib, &cat, Root::definition(&lib, "smooth_max").unwrap()).unwrap();
        assert_eq!(
            e.evaluate(&sample("s", 1.0)).unwrap().value,
            Value::Scalar(Some(30.0))
        );
        let mut e = Evaluator::new(&lib, &cat, root("derived.smooth", None)).unwrap();
        let out = e.evaluate(&sample("s", 1.0)).unwrap();
        assert_eq!(out.signature.kind, super::super::Kind::Series);
        assert_eq!(out.signature.x_unit, "s");
    }

    #[test]
    fn unknown_references() {
        let lib = library();
        assert!(root("derived.nope", None).closure(&lib).is_err());
        assert!(Root::definition(&lib, "nope").is_err());
        let cat = catalogue();
        let d = compile(
            &lib,
            &cat,
            &root("derived.throughput + 1[s]", None),
            &sample("s", 1.0),
        )
        .unwrap_err();
        assert!(d[0].message.contains("compatible"));
    }

    #[test]
    fn what_a_metric_uses() {
        let mut lib = library();
        lib.insert(def("top", "derived.bits + derived.gone", "Mbit/s"));
        let used = |name: &str| -> Vec<String> {
            lib.uses_closure([name.to_owned()]).into_iter().collect()
        };
        assert_eq!(used("top"), vec!["bits", "gone", "throughput"]);
        assert_eq!(used("throughput"), Vec::<String>::new());
        assert_eq!(used("unknown"), Vec::<String>::new());
    }

    #[test]
    fn cycles_are_errors_not_loops() {
        // The database rejects cycles on save; evaluation must still end.
        let lib = Library::new([
            def("a", "derived.b + 1", ""),
            def("b", "derived.a * 2", ""),
            def("me", "derived.me", ""),
        ]);
        let cat = catalogue();
        for name in ["a", "me"] {
            let root = Root::definition(&lib, name).unwrap();
            let err = root.closure(&lib).unwrap_err();
            assert!(err.contains("uses itself"), "{err}");
            let d = compile(&lib, &cat, &root, &sample("s", 1.0)).unwrap_err();
            assert!(d[0].message.contains("uses itself"), "{d:?}");
        }
        assert!(lib.uses_closure(["a".to_owned()]).contains("a"));
    }
}
