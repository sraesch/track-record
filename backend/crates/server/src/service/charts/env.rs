//! The units that validation checks a spec against (ADR 0022, "Format 1",
//! "Validation"): the metrics and derived metrics that the metric
//! expressions of its sources read, with the latest units, as
//! `service::derived` validates an expression.

use std::collections::{BTreeSet, HashMap};

use sqlx::PgConnection;
use track_record_core::chart::{NodeKind, Setting, Spec};
use track_record_core::dsl::check::{Env, SeriesUnits, Signature};
use track_record_core::dsl::program::{Library, Root, UnitSource, compile};
use track_record_core::dsl::{derived_reference, parse};
use track_record_core::units::UnitCatalogue;

use crate::error::ApiResult;
use crate::service::catalog;
use crate::service::derived::{LatestUnits, load_library};

/// [`Env`] over the definitions and units loaded for one spec.
pub struct ChartEnv {
    pub catalogue: UnitCatalogue,
    library: Library,
    latest: LatestUnits,
    derived: HashMap<String, Result<Signature, String>>,
}

impl ChartEnv {
    /// Loads what the metric expressions of `spec`'s sources read: the
    /// derived metrics they use, directly or not, and the units of the
    /// uploaded metrics behind them. Settings that are variables count with
    /// their defaults. Without a spec (one that does not parse), only the
    /// catalogue.
    pub async fn load(conn: &mut PgConnection, spec: Option<&Spec>) -> ApiResult<Self> {
        let catalogue = catalog::unit_catalogue(&mut *conn).await?;
        let mut metrics = BTreeSet::new();
        let mut derived = BTreeSet::new();
        if let Some(spec) = spec {
            for node in &spec.bind_defaults().nodes {
                if let NodeKind::Source(source) = &node.kind
                    && let Setting::Value(metric) = &source.metric
                    && let Ok(expr) = parse(metric)
                {
                    let refs = expr.references();
                    metrics.extend(refs.metrics);
                    metrics.extend(refs.series);
                    derived.extend(refs.derived);
                }
            }
        }
        // PostgreSQL text cannot hold NUL; such a name exists nowhere, and
        // the check reports it as unknown.
        let storable = |n: &String| !n.contains('\0');
        derived.retain(storable);
        let library = load_library(&mut *conn, &derived).await?;
        // The raw inputs of the derived metrics, through those they use.
        for name in library
            .uses_closure(derived.iter().cloned())
            .iter()
            .chain(&derived)
        {
            if let Some(d) = library.get(name)
                && let Ok(e) = parse(&d.expression)
            {
                let r = e.references();
                metrics.extend(r.metrics);
                metrics.extend(r.series);
            }
        }
        metrics.retain(storable);
        let latest = LatestUnits::load(conn, &metrics).await?;
        Ok(Self {
            catalogue,
            library,
            latest,
            derived: HashMap::new(),
        })
    }
}

impl Env for ChartEnv {
    fn metric(&self, name: &str) -> Option<String> {
        self.latest.metric_unit(name)
    }

    fn series(&self, name: &str) -> Option<SeriesUnits> {
        self.latest.series_units(name)
    }

    fn derived(&mut self, name: &str) -> Result<Signature, String> {
        if let Some(done) = self.derived.get(name) {
            return done.clone();
        }
        // Compiled as a source compiles `derived.<name>`, so that the
        // reference counts as a level of nesting there too.
        let signature = parse(&derived_reference(name))
            .map_err(|d| d.message)
            .and_then(|expr| {
                let root = Root { expr, unit: None };
                compile(&self.library, &self.catalogue, &root, &self.latest)
                    .map(|c| c.signature().clone())
                    .map_err(|d| d[0].message.clone())
            });
        self.derived.insert(name.to_owned(), signature.clone());
        signature
    }
}
