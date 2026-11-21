//! Composition of derived metrics (FR-17) and their evaluation per sample.
//!
//! A [`Library`] holds derived metric versions. Each version pins the exact
//! versions of the derived metrics it uses (`uses`), so evaluating it never
//! depends on later versions. A [`Root`] is what is evaluated: a saved
//! version, or an ad-hoc expression whose `derived.<name>` references point
//! at the versions given in its own `uses` (usually the latest ones).
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

/// A saved derived metric version.
#[derive(Debug, Clone)]
pub struct Definition {
    pub version_id: i64,
    pub name: String,
    pub version: i32,
    pub expression: String,
    pub unit: String,
    /// Derived metric name -> the version id it is pinned to.
    pub uses: HashMap<String, i64>,
}

#[derive(Debug, Clone)]
struct Entry {
    def: Definition,
    expr: Result<Expr, Diagnostic>,
}

/// Derived metric versions by id.
#[derive(Debug, Clone, Default)]
pub struct Library {
    entries: HashMap<i64, Entry>,
}

impl Library {
    pub fn new(defs: impl IntoIterator<Item = Definition>) -> Self {
        Self {
            entries: defs
                .into_iter()
                .map(|def| {
                    let expr = parse(&def.expression);
                    (def.version_id, Entry { def, expr })
                })
                .collect(),
        }
    }

    pub fn get(&self, version_id: i64) -> Option<&Definition> {
        self.entries.get(&version_id).map(|e| &e.def)
    }

    fn entry(&self, version_id: i64) -> Result<&Entry, String> {
        self.entries
            .get(&version_id)
            .ok_or_else(|| format!("derived metric version {version_id} is not loaded"))
    }
}

/// What to evaluate.
#[derive(Debug, Clone)]
pub struct Root {
    pub expr: Expr,
    /// Derived metric name -> version id.
    pub uses: HashMap<String, i64>,
    /// Output unit; `None` keeps the unit the expression has.
    pub unit: Option<String>,
}

impl Root {
    /// A saved version, in its declared unit.
    pub fn definition(library: &Library, version_id: i64) -> Result<Root, String> {
        let entry = library.entry(version_id)?;
        let expr = entry.expr.clone().map_err(|d| {
            format!(
                "derived metric {} version {} does not parse: {}",
                entry.def.name, entry.def.version, d.message
            )
        })?;
        Ok(Root {
            expr,
            uses: entry.def.uses.clone(),
            unit: Some(entry.def.unit.clone()),
        })
    }

    /// The raw inputs the root needs, including through the derived metrics
    /// it uses, and the derived metric versions involved.
    pub fn closure(&self, library: &Library) -> Result<(References, BTreeSet<i64>), String> {
        let mut raw = References::default();
        let mut versions = BTreeSet::new();
        visit(&self.expr, &self.uses, library, &mut raw, &mut versions, 0)?;
        Ok((raw, versions))
    }
}

fn visit(
    expr: &Expr,
    uses: &HashMap<String, i64>,
    library: &Library,
    raw: &mut References,
    versions: &mut BTreeSet<i64>,
    depth: usize,
) -> Result<(), String> {
    if depth > MAX_COMPOSITION_DEPTH {
        return Err(format!(
            "derived metrics are nested deeper than {MAX_COMPOSITION_DEPTH} levels"
        ));
    }
    let refs = expr.references();
    raw.metrics.extend(refs.metrics);
    raw.series.extend(refs.series);
    for name in refs.derived {
        let id = *uses
            .get(&name)
            .ok_or_else(|| format!("unknown derived metric {name}"))?;
        if !versions.insert(id) {
            continue;
        }
        let entry = library.entry(id)?;
        let Ok(inner) = &entry.expr else {
            return Err(format!("derived metric {name} does not parse"));
        };
        visit(inner, &entry.def.uses, library, raw, versions, depth + 1)?;
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
    root_uses: HashMap<String, i64>,
    deps: HashMap<i64, (Checked, HashMap<String, i64>)>,
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
    deps: HashMap<i64, Result<(Checked, HashMap<String, i64>), String>>,
}

struct CompileEnv<'s, 'a> {
    state: &'s mut CompileState<'a>,
    uses: &'s HashMap<String, i64>,
    depth: usize,
}

impl Env for CompileEnv<'_, '_> {
    fn metric(&self, name: &str) -> Option<String> {
        self.state.source.metric_unit(name)
    }

    fn series(&self, name: &str) -> Option<SeriesUnits> {
        self.state.source.series_units(name)
    }

    fn derived(&mut self, name: &str) -> Result<Signature, String> {
        let id = *self
            .uses
            .get(name)
            .ok_or_else(|| format!("unknown derived metric {name}"))?;
        compile_dep(self.state, id, self.depth + 1).map(|c| c.signature)
    }
}

fn compile_dep(state: &mut CompileState<'_>, id: i64, depth: usize) -> Result<Checked, String> {
    if let Some(done) = state.deps.get(&id) {
        return done.clone().map(|(c, _)| c);
    }
    if depth > MAX_COMPOSITION_DEPTH {
        return Err(format!(
            "derived metrics are nested deeper than {MAX_COMPOSITION_DEPTH} levels"
        ));
    }
    let entry = state.library.entry(id)?.clone();
    let label = format!("derived.{} (version {})", entry.def.name, entry.def.version);
    let result = match &entry.expr {
        Err(d) => Err(format!("{label} does not parse: {}", d.message)),
        Ok(expr) => {
            let catalogue = state.catalogue;
            let mut env = CompileEnv {
                state: &mut *state,
                uses: &entry.def.uses,
                depth,
            };
            check(expr, &mut env, catalogue, Some(&entry.def.unit))
                .map_err(|d| format!("{label} does not apply: {}", d[0].message))
        }
    };
    let result = result.map(|c| (c, entry.def.uses.clone()));
    state.deps.insert(id, result.clone());
    result.map(|(c, _)| c)
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
    };
    let checked = {
        let mut env = CompileEnv {
            state: &mut state,
            uses: &root.uses,
            depth: 0,
        };
        check(&root.expr, &mut env, catalogue, root.unit.as_deref())?
    };
    let deps = state
        .deps
        .into_iter()
        .filter_map(|(id, r)| r.ok().map(|c| (id, c)))
        .collect();
    Ok(Compiled {
        root: checked,
        root_uses: root.uses.clone(),
        deps,
    })
}

struct SampleInputs<'a> {
    compiled: &'a Compiled,
    sample: &'a SampleData,
    uses: &'a HashMap<String, i64>,
    memo: &'a mut HashMap<i64, Result<Value, EvalError>>,
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
        let id = *self
            .uses
            .get(name)
            .ok_or_else(|| EvalError::Data(format!("unknown derived metric {name}")))?;
        if let Some(v) = self.memo.get(&id) {
            return v.clone();
        }
        let (checked, uses) = self.compiled.deps.get(&id).ok_or_else(|| {
            EvalError::Data(format!("derived.{name} does not apply to this sample"))
        })?;
        let mut inner = SampleInputs {
            compiled: self.compiled,
            sample: self.sample,
            uses,
            memo: &mut *self.memo,
        };
        let value = eval(&checked.node, &mut inner).map(|v| v.scale(checked.output_factor));
        self.memo.insert(id, value.clone());
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
        uses: &compiled.root_uses,
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
    cache: HashMap<String, Result<Arc<Compiled>, String>>,
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
        let mut key = String::new();
        for name in &self.raw.metrics {
            let unit = sample
                .metric_unit(name)
                .ok_or_else(|| EvalError::Missing(name.clone()))?;
            key.push_str(&format!("m{}:{name}={unit};", name.len()));
        }
        for name in &self.raw.series {
            let u = sample
                .series_units(name)
                .ok_or_else(|| EvalError::Missing(format!("series.{name}")))?;
            key.push_str(&format!(
                "s{}:{name}={}|{}|{};",
                name.len(),
                u.unit,
                u.x_unit,
                u.x_dimension
            ));
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

    fn def(id: i64, name: &str, expression: &str, unit: &str, uses: &[(&str, i64)]) -> Definition {
        Definition {
            version_id: id,
            name: name.into(),
            version: 1,
            expression: expression.into(),
            unit: unit.into(),
            uses: uses.iter().map(|(n, i)| ((*n).into(), *i)).collect(),
        }
    }

    fn library() -> Library {
        Library::new([
            def(1, "throughput", "bytes / time", "MB/s", &[]),
            def(
                2,
                "bits",
                "derived.throughput",
                "Mbit/s",
                &[("throughput", 1)],
            ),
            def(3, "throughput", "bytes / time / 2", "MB/s", &[]),
            def(4, "first_frame", "time_to(series.fps, 1)", "ms", &[]),
            def(5, "smooth", "moving_avg(series.fps, 1)", "fps", &[]),
            def(
                6,
                "smooth_max",
                "max(derived.smooth)",
                "fps",
                &[("smooth", 5)],
            ),
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

    fn root(src: &str, uses: &[(&str, i64)], unit: Option<&str>) -> Root {
        Root {
            expr: parse(src).unwrap(),
            uses: uses.iter().map(|(n, i)| ((*n).into(), *i)).collect(),
            unit: unit.map(Into::into),
        }
    }

    #[test]
    fn pinned_versions_are_used() {
        let lib = library();
        let cat = catalogue();
        let mut e = Evaluator::new(&lib, &cat, Root::definition(&lib, 2).unwrap()).unwrap();
        // 8 MB / 2 s = 4 MB/s, converted to 32 Mbit/s; version 3 of
        // throughput is ignored.
        let out = e.evaluate(&sample("s", 2.0)).unwrap();
        assert_eq!(out.value, Value::Scalar(Some(32.0)));
        assert_eq!(out.signature.unit, "Mbit/s");
        let (raw, versions) = Root::definition(&lib, 2).unwrap().closure(&lib).unwrap();
        assert_eq!(versions.into_iter().collect::<Vec<_>>(), vec![1]);
        assert!(raw.metrics.contains("bytes") && raw.metrics.contains("time"));
    }

    #[test]
    fn inputs_in_other_units_convert() {
        let lib = library();
        let cat = catalogue();
        let mut e = Evaluator::new(
            &lib,
            &cat,
            root("derived.throughput", &[("throughput", 1)], None),
        )
        .unwrap();
        assert_eq!(
            e.evaluate(&sample("s", 2.0)).unwrap().value,
            Value::Scalar(Some(4.0))
        );
        assert_eq!(
            e.evaluate(&sample("ms", 2000.0)).unwrap().value,
            Value::Scalar(Some(4.0))
        );
        // An ad-hoc expression in a requested unit.
        let mut e = Evaluator::new(&lib, &cat, root("bytes / time", &[], Some("kB/s"))).unwrap();
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
    fn missing_inputs_and_no_value() {
        let lib = library();
        let cat = catalogue();
        let mut e = Evaluator::new(&lib, &cat, Root::definition(&lib, 4).unwrap()).unwrap();
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
        let mut e =
            Evaluator::new(&lib, &cat, root("time_to(series.fps, 100)", &[], None)).unwrap();
        assert_eq!(
            e.evaluate(&sample("s", 1.0)).unwrap().value,
            Value::Scalar(None)
        );
    }

    #[test]
    fn series_composition() {
        let lib = library();
        let cat = catalogue();
        let mut e = Evaluator::new(&lib, &cat, Root::definition(&lib, 6).unwrap()).unwrap();
        assert_eq!(
            e.evaluate(&sample("s", 1.0)).unwrap().value,
            Value::Scalar(Some(30.0))
        );
        let mut e =
            Evaluator::new(&lib, &cat, root("derived.smooth", &[("smooth", 5)], None)).unwrap();
        let out = e.evaluate(&sample("s", 1.0)).unwrap();
        assert_eq!(out.signature.kind, super::super::Kind::Series);
        assert_eq!(out.signature.x_unit, "s");
    }

    #[test]
    fn unknown_references() {
        let lib = library();
        assert!(root("derived.nope", &[], None).closure(&lib).is_err());
        assert!(root("derived.x", &[("x", 99)], None).closure(&lib).is_err());
        let cat = catalogue();
        let d = compile(
            &lib,
            &cat,
            &root("derived.throughput + 1[s]", &[("throughput", 1)], None),
            &sample("s", 1.0),
        )
        .unwrap_err();
        assert!(d[0].message.contains("compatible"));
    }
}
