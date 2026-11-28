//! Running a chart (ADR 0022, "Evaluation", FR-20, FR-22, FR-23, NFR-7).
//!
//! Before anything streams, a run reads the spec, binds the values of its
//! variables, validates it and selects the samples of every source it
//! needs; errors there are problem details. Then it streams NDJSON events
//! (`start`, `progress`, one `result`, then `done` or `error`), or, with
//! `format`, a file of the data that reaches the visualization.
//!
//! All reads happen in one read-only `REPEATABLE READ` transaction
//! ([`super::snapshot`]), which ends once the sources are loaded; the nodes
//! are evaluated after that, off the async runtime.

use std::collections::BTreeMap;

use axum::http::StatusCode;
use bytes::Bytes;
use sqlx::{PgPool, Postgres, Transaction};
use tokio::sync::mpsc;
use track_record_core::chart::display;
use track_record_core::chart::spec::LineChart;
use track_record_core::chart::{self, Graph, NodeKind, Problem, Setting, Spec, Tree, eval};
use track_record_core::units::UnitCatalogue;

use super::env::ChartEnv;
use super::export;
use super::model::{ChartEvent, RunChart, SourceSamples};
use super::source::{Budget, Limits, Selected, select};
use crate::error::{ApiError, ApiResult};
use crate::service::analysis::Excluded;
use crate::service::table::ExportFormat;

/// Points per line for display unless the request says otherwise.
pub const DEFAULT_MAX_POINTS: usize = 2000;

/// A stream of file bytes. An error after the start aborts the response.
pub type FileStream = mpsc::Receiver<Result<Bytes, std::io::Error>>;

/// What a run streams.
pub enum RunOutput {
    Events(mpsc::Receiver<ChartEvent>),
    File {
        stream: FileStream,
        format: ExportFormat,
    },
}

/// A spec that cannot run: 422 `invalid_chart` with its problems.
fn invalid_chart(problems: &[Problem]) -> ApiError {
    let detail = problems
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ");
    ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_chart",
        format!("the chart cannot run: {detail}"),
    )
}

/// Why a run failed after it started.
enum Failure {
    Api(ApiError),
    Node(Problem),
}

impl From<ApiError> for Failure {
    fn from(e: ApiError) -> Self {
        Failure::Api(e)
    }
}

impl From<sqlx::Error> for Failure {
    fn from(e: sqlx::Error) -> Self {
        Failure::Api(e.into())
    }
}

impl Failure {
    fn event(self) -> ChartEvent {
        match self {
            Failure::Node(p) => ChartEvent::Error {
                code: "chart_failed".into(),
                detail: p.to_string(),
                node: p.node,
                setting: p.setting,
            },
            Failure::Api(e) => {
                let detail = match &e {
                    ApiError::Problem { detail, .. } => detail.clone(),
                    ApiError::Internal(err) => {
                        tracing::error!(error = ?err, "chart run failed");
                        "internal server error".to_owned()
                    }
                };
                ChartEvent::Error {
                    code: e.code().to_owned(),
                    detail,
                    node: None,
                    setting: None,
                }
            }
        }
    }

    fn io(self) -> std::io::Error {
        let detail = match self {
            Failure::Node(p) => p.to_string(),
            Failure::Api(ApiError::Problem { detail, .. }) => detail,
            Failure::Api(ApiError::Internal(err)) => {
                tracing::error!(error = ?err, "chart export failed");
                "internal server error".to_owned()
            }
        };
        std::io::Error::other(detail)
    }
}

/// A run that passed the checks: the bound spec, the node it stops at and
/// the selected samples of the sources it reads, in a snapshot.
struct Prepared {
    spec: Spec,
    graph: Graph,
    target: usize,
    partial: bool,
    sources: Vec<(String, Selected)>,
    catalogue: UnitCatalogue,
    snapshot: Transaction<'static, Postgres>,
    limits: Limits,
}

impl Prepared {
    fn node(&self) -> String {
        self.spec.nodes[self.target].id.clone()
    }

    fn total_samples(&self) -> usize {
        self.sources.iter().map(|(_, s)| s.samples()).sum()
    }
}

/// Starts a run. Errors in the request, the spec or the selection of the
/// sources are returned before streaming; later errors end the stream.
pub async fn run(pool: &PgPool, limits: Limits, req: RunChart) -> ApiResult<RunOutput> {
    if req.format.is_some() && (req.until.is_some() || req.preview || req.max_points.is_some()) {
        return Err(ApiError::bad_request(
            "format exports the data that reaches the visualization; \
             it does not go with until, preview or max_points",
        ));
    }
    if req.max_points.is_some_and(|m| m < 2) {
        return Err(ApiError::bad_request("max_points must be at least 2"));
    }
    let prepared = prepare(pool, limits, &req).await?;
    match req.format {
        Some(format) => {
            let (tx, rx) = mpsc::channel(8);
            tokio::spawn(async move {
                let result = match evaluate(prepared, &mut |_| {}).await {
                    Ok((tree, _)) => export::write(&tree, format, &tx)
                        .await
                        .map_err(|e| Failure::Api(ApiError::Internal(e))),
                    Err(f) => Err(f),
                };
                if let Err(f) = result {
                    let _ = tx.send(Err(f.io())).await;
                }
            });
            Ok(RunOutput::File { stream: rx, format })
        }
        None => {
            let (tx, rx) = mpsc::channel(64);
            let max_points = req.max_points.unwrap_or(DEFAULT_MAX_POINTS);
            tokio::spawn(stream(prepared, max_points, tx));
            Ok(RunOutput::Events(rx))
        }
    }
}

async fn prepare(pool: &PgPool, limits: Limits, req: &RunChart) -> ApiResult<Prepared> {
    let spec = Spec::from_json(&req.spec).map_err(|p| invalid_chart(&p))?;
    let graph = Graph::new(&spec).map_err(|p| invalid_chart(&p))?;
    let target = match &req.until {
        None => graph.visualization(),
        Some(id) => graph.index(id).ok_or_else(|| {
            invalid_chart(&[Problem::spec_setting(
                "until",
                format!("there is no node {id:?}"),
            )])
        })?,
    };
    let needed = graph.upstream(target);
    // A whole run needs a valid spec. A run that stops at a node (`until`)
    // needs the nodes it evaluates to be valid, and the spec itself.
    let blocks = |p: &Problem| {
        req.until.is_none()
            || p.node
                .as_ref()
                .and_then(|id| graph.index(id))
                .is_none_or(|i| needed.contains(&i))
    };
    let (spec, mut problems) = spec.bind_partly(&req.variables);
    problems.retain(|p| blocks(p));
    if !problems.is_empty() {
        return Err(invalid_chart(&problems));
    }

    let mut snapshot = super::snapshot(pool).await?;
    let mut env = ChartEnv::load(&mut snapshot, Some(&spec)).await?;
    let catalogue = env.catalogue.clone();
    let mut problems = chart::validate(&spec, &graph, &mut env, &catalogue).problems;
    problems.retain(|p| blocks(p));
    if !problems.is_empty() {
        return Err(invalid_chart(&problems));
    }

    let mut sources = Vec::new();
    for &i in &needed {
        let node = &spec.nodes[i];
        if let NodeKind::Source(src) = &node.kind {
            let selected = select(&mut snapshot, &node.id, src, &limits, req.preview).await?;
            sources.push((node.id.clone(), selected));
        }
    }
    Ok(Prepared {
        partial: req.preview || target != graph.visualization(),
        spec,
        graph,
        target,
        sources,
        catalogue,
        snapshot,
        limits,
    })
}

/// Loads the sources of a prepared run, then ends its snapshot and
/// evaluates the nodes. `progress` gets the samples done over all sources.
async fn evaluate(
    prepared: Prepared,
    progress: &mut (dyn FnMut(usize) + Send),
) -> Result<(Tree, BTreeMap<String, Excluded>), Failure> {
    let Prepared {
        spec,
        graph,
        target,
        sources,
        catalogue,
        mut snapshot,
        limits,
        ..
    } = prepared;
    let mut budget = Budget::new(limits.max_points);
    let mut trees = BTreeMap::new();
    let mut excluded = BTreeMap::new();
    let mut before = 0;
    for (id, selected) in sources {
        let samples = selected.samples();
        let loaded = selected
            .load(&mut snapshot, &limits, &mut budget, &mut |done| {
                progress(before + done)
            })
            .await?;
        before += samples;
        trees.insert(id.clone(), loaded.tree);
        excluded.insert(id, loaded.excluded);
    }
    // Everything is read: end the snapshot before the evaluation.
    snapshot.rollback().await?;

    let until = spec.nodes[target].id.clone();
    let tree = tokio::task::spawn_blocking(move || {
        eval::run(
            &spec,
            &graph,
            trees,
            Some(&until),
            &catalogue,
            limits.max_points,
        )
    })
    .await
    .map_err(|e| Failure::Api(ApiError::Internal(e.into())))?
    .map_err(Failure::Node)?;
    Ok((tree, excluded))
}

/// The NDJSON events of a run.
async fn stream(prepared: Prepared, max_points: usize, tx: mpsc::Sender<ChartEvent>) {
    let node = prepared.node();
    let partial = prepared.partial;
    let total = prepared.total_samples();
    let start = ChartEvent::Start {
        node: node.clone(),
        partial,
        sources: prepared
            .sources
            .iter()
            .map(|(id, s)| SourceSamples {
                id: id.clone(),
                samples: s.samples(),
            })
            .collect(),
        total_samples: total,
    };
    if tx.send(start).await.is_err() {
        return;
    }
    let line = match &prepared.spec.nodes[prepared.target].kind {
        NodeKind::Line(LineChart {
            y: Setting::Value(y),
            ..
        }) => Some(y.clone()),
        _ => None,
    };
    let progress_tx = tx.clone();
    let result = evaluate(prepared, &mut |done| {
        // Progress is best effort: a slow reader skips some.
        let _ = progress_tx.try_send(ChartEvent::Progress { done, total });
    })
    .await;
    let event = match result {
        Ok((mut tree, excluded)) => {
            if let Some(y) = &line {
                for_each_frame(&mut tree, &mut |f| display::downsample(f, y, max_points));
            }
            ChartEvent::Result {
                node,
                partial,
                tree: (&tree).into(),
                excluded,
            }
        }
        Err(f) => {
            let _ = tx.send(f.event()).await;
            return;
        }
    };
    if tx.send(event).await.is_ok() {
        let _ = tx.send(ChartEvent::Done).await;
    }
}

fn for_each_frame(tree: &mut Tree, f: &mut dyn FnMut(&mut chart::Frame)) {
    match tree {
        Tree::Frames(frames) => frames.iter_mut().for_each(f),
        Tree::Groups(groups) => groups
            .iter_mut()
            .for_each(|g| for_each_frame(&mut g.tree, f)),
    }
}
