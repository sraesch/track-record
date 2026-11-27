//! MCP server on the machine port (`/mcp`, ADR 0008).
//!
//! Streamable HTTP in stateless mode with plain JSON responses: every POST is
//! answered by a fresh [`McpServer`], so nothing is kept between requests
//! (NFR-1). The route sits behind the same token middleware as `/api`; the
//! token's scope decides which tools exist:
//!
//! - every scope gets the read tools,
//! - `annotate` also gets the tag and annotation tools.
//!
//! There are no tools for uploads, invalidation or deletion. Every tool calls
//! `service::*`, so aliases, defaults (invalid runs and warmup samples
//! excluded) and validation are the same as in the REST API.

use std::sync::Arc;

use axum::http::request::Parts;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, RoleServer, ServerHandler, tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::AppState;
use crate::auth::Authenticated;
use crate::db::TokenScope;
use crate::error::ApiError;
use crate::service::analysis::compare::{self, CompareEvent, CompareRequest};
use crate::service::analysis::{self as analysis, AggregatesRequest, TrendRequest};
use crate::service::annotations::{self, AnnotationQuery, CreateAnnotation, UpdateAnnotation};
use crate::service::catalog::{self, TestCaseQuery};
use crate::service::derived::{self, EvaluateRequest, ValidateExpression};
use crate::service::metrics;
use crate::service::runs::{self, CaseExecutionListQuery, RunFilter};

const INSTRUCTIONS: &str = "\
track-record stores benchmark results. A run (one harness invocation) has case \
executions: a test case (scenario version + assets + parameters) on a client device, \
with samples (repetitions) that hold measurements and series. Start with list_runs or \
the list_* catalogue tools, then use aggregates, trend and compare for analysis. \
Metrics in analysis tools are DSL expressions: an uploaded metric name (`load_time`), \
a derived metric (`derived.throughput`) or an expression; check one with \
validate_derived_metric. Invalid runs and warmup samples are excluded by default. \
Devices and parameter values are reported after aliases.";

/// The tools of one request. Holds the shared routers and the app state;
/// cheap to create per request.
#[derive(Clone)]
pub struct McpServer {
    state: AppState,
    tools: Arc<Tools>,
}

struct Tools {
    read: ToolRouter<McpServer>,
    annotate: ToolRouter<McpServer>,
}

/// The `/mcp` service. Host validation is off when `allowed_hosts` is empty:
/// the bearer token already keeps browsers (DNS rebinding) out.
pub fn service(state: AppState) -> StreamableHttpService<McpServer, NeverSessionManager> {
    let tools = Arc::new(Tools {
        read: McpServer::read_tools(),
        annotate: McpServer::annotate_tools(),
    });
    let mut config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None)
        .with_max_request_body_bytes(state.max_body_bytes.max(1024 * 1024))
        .disable_allowed_origins();
    config = if state.mcp_allowed_hosts.is_empty() {
        config.disable_allowed_hosts()
    } else {
        config.with_allowed_hosts(state.mcp_allowed_hosts.iter().cloned())
    };
    StreamableHttpService::new(
        move || {
            Ok(McpServer {
                state: state.clone(),
                tools: tools.clone(),
            })
        },
        Arc::new(NeverSessionManager::default()),
        config,
    )
}

/// The scope of the request's token, set by the token middleware.
fn scope(context: &RequestContext<RoleServer>) -> Result<TokenScope, ErrorData> {
    context
        .extensions
        .get::<Parts>()
        .and_then(|parts| parts.extensions.get::<Authenticated>())
        .map(|auth| auth.scope)
        .ok_or_else(|| ErrorData::invalid_request("missing API token", None))
}

/// A successful result. Structured content must be an object, so lists are
/// wrapped in `{"items": [...]}`.
fn ok<T: Serialize>(value: &T) -> Result<CallToolResult, ErrorData> {
    let value = serde_json::to_value(value)
        .map_err(|e| ErrorData::internal_error(format!("cannot serialize result: {e}"), None))?;
    Ok(CallToolResult::structured(match value {
        Value::Object(_) => value,
        other => json!({ "items": other }),
    }))
}

/// A tool error the caller sees, with the same `code` as the REST problem
/// details.
fn tool_error(err: ApiError) -> CallToolResult {
    let detail = match &err {
        ApiError::Problem { detail, .. } => detail.clone(),
        ApiError::Internal(e) => {
            tracing::error!(error = ?e, "internal error in MCP tool");
            "internal server error".to_owned()
        }
    };
    CallToolResult::structured_error(json!({
        "status": err.status().as_u16(),
        "code": err.code(),
        "detail": detail,
    }))
}

fn respond<T: Serialize>(result: Result<T, ApiError>) -> Result<CallToolResult, ErrorData> {
    match result {
        Ok(value) => ok(&value),
        Err(err) => Ok(tool_error(err)),
    }
}

// Parameters of tools without a service request type --------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunId {
    pub run_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListCaseExecutions {
    pub run_id: Uuid,
    /// Include case executions replaced by a correction.
    #[serde(default)]
    pub include_superseded: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaseExecutionId {
    pub case_execution_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunTag {
    pub run_id: Uuid,
    pub tag: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TestCaseTag {
    pub test_case_id: i64,
    pub tag: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditAnnotation {
    pub annotation_id: Uuid,
    pub body: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnotationId {
    pub annotation_id: Uuid,
}

/// Runs a comparison and collects its stream into one result: the `start`
/// fields, all rows and the `done` fields. MCP has no streamed tool results;
/// the REST endpoint streams the same events (NFR-7).
async fn collect_comparison(
    state: &AppState,
    req: CompareRequest,
) -> Result<CallToolResult, ErrorData> {
    let mut rx = match compare::compare(state.pool.clone(), req).await {
        Ok(rx) => rx,
        Err(err) => return Ok(tool_error(err)),
    };
    let mut result = json!({});
    let mut rows = Vec::new();
    while let Some(event) = rx.recv().await {
        match event {
            CompareEvent::Progress { .. } => {}
            CompareEvent::Start {
                metric,
                dimension,
                values,
                reference,
                total_samples,
            } => {
                result = json!({
                    "metric": metric,
                    "dimension": dimension,
                    "values": values,
                    "reference": reference,
                    "total_samples": total_samples,
                });
            }
            CompareEvent::Row(row) => rows.push(row),
            CompareEvent::Done { excluded, .. } => {
                result["rows"] = serde_json::to_value(&rows).expect("rows serialize");
                result["excluded"] = serde_json::to_value(&excluded).expect("excluded serializes");
                return Ok(CallToolResult::structured(result));
            }
            CompareEvent::Error { code, detail } => {
                return Ok(CallToolResult::structured_error(json!({
                    "code": code,
                    "detail": detail,
                })));
            }
        }
    }
    Err(ErrorData::internal_error(
        "comparison ended without a done event",
        None,
    ))
}

// Read tools -------------------------------------------------------------------------

#[tool_router(router = read_tools, vis = "pub(crate)")]
impl McpServer {
    /// List runs, newest measured start first (FR-11).
    #[tool(
        description = "List runs, newest measured start first. Filters: suite, SUT \
                       version fields, measured start time range, relative selection \
                       (`started_within_days`, `latest_runs`, `since_tag`, \
                       `since_sut_tag`), status, trigger, client \
                       device (after aliases), scenario, asset, parameters (`name=value`), \
                       metric and tags. Only valid runs unless `validity` says otherwise. \
                       Pages with `limit` and `cursor` (the previous `next_cursor`).",
        annotations(read_only_hint = true)
    )]
    async fn list_runs(
        &self,
        Parameters(filter): Parameters<RunFilter>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(runs::list_runs(&self.state.pool, &filter).await)
    }

    #[tool(
        description = "A run with its suite, SUT version, shared environment, tags, \
                       annotations and attachments.",
        annotations(read_only_hint = true)
    )]
    async fn get_run(&self, Parameters(p): Parameters<RunId>) -> Result<CallToolResult, ErrorData> {
        respond(runs::get_run(&self.state.pool, p.run_id).await)
    }

    #[tool(
        description = "Case executions of a run: test case, client device and sample \
                       counts. Corrected (superseded) ones are hidden by default.",
        annotations(read_only_hint = true)
    )]
    async fn list_case_executions(
        &self,
        Parameters(p): Parameters<ListCaseExecutions>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = CaseExecutionListQuery {
            include_superseded: p.include_superseded,
        };
        respond(runs::list_case_executions(&self.state.pool, p.run_id, &query).await)
    }

    #[tool(
        description = "A case execution with its samples, measurements and series \
                       metadata (no series points).",
        annotations(read_only_hint = true)
    )]
    async fn get_case_execution(
        &self,
        Parameters(p): Parameters<CaseExecutionId>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(runs::get_case_execution(&self.state.pool, p.case_execution_id).await)
    }

    #[tool(
        description = "Suites with all their versions.",
        annotations(read_only_hint = true)
    )]
    async fn list_suites(&self) -> Result<CallToolResult, ErrorData> {
        respond(catalog::suites(&self.state.pool).await)
    }

    #[tool(
        description = "Scenarios with all their versions. A changed scenario definition \
                       is a new version and a different case.",
        annotations(read_only_hint = true)
    )]
    async fn list_scenarios(&self) -> Result<CallToolResult, ErrorData> {
        respond(catalog::scenarios(&self.state.pool).await)
    }

    #[tool(
        description = "Test cases (scenario version, assets, parameters with their units) \
                       with their tags, optionally of one scenario.",
        annotations(read_only_hint = true)
    )]
    async fn list_test_cases(
        &self,
        Parameters(query): Parameters<TestCaseQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(catalog::test_cases(&self.state.pool, &query).await)
    }

    #[tool(
        description = "Uploaded metrics: unit, value type, direction and the derived \
                       metrics that use them. A metric has one definition, which can change.",
        annotations(read_only_hint = true)
    )]
    async fn list_metrics(&self) -> Result<CallToolResult, ErrorData> {
        respond(metrics::list(&self.state.pool).await)
    }

    #[tool(
        description = "Client devices with their names after aliases.",
        annotations(read_only_hint = true)
    )]
    async fn list_client_devices(&self) -> Result<CallToolResult, ErrorData> {
        respond(catalog::client_devices(&self.state.pool).await)
    }

    #[tool(
        description = "Derived metrics: expression, unit and the derived metrics they \
                       use. A derived metric has one definition, which can change.",
        annotations(read_only_hint = true)
    )]
    async fn list_derived_metrics(&self) -> Result<CallToolResult, ErrorData> {
        respond(derived::list(&self.state.pool).await)
    }

    #[tool(
        description = "Annotations, filtered by run, test case or a time range of \
                       point-in-time annotations.",
        annotations(read_only_hint = true)
    )]
    async fn list_annotations(
        &self,
        Parameters(query): Parameters<AnnotationQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(annotations::list(&self.state.pool, &query).await)
    }

    #[tool(
        description = "Statistics across samples per run, case and client device: n, \
                       no-value count, mean, median, percentiles, stddev, min, max and a \
                       confidence interval (FR-12). `metric` is a DSL expression; `unit` \
                       converts the values.",
        annotations(read_only_hint = true)
    )]
    async fn aggregates(
        &self,
        Parameters(req): Parameters<AggregatesRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(analysis::aggregates(&self.state.pool, &req).await)
    }

    #[tool(
        description = "Trend of one case (identified by any of its test cases), metric \
                       and client device across runs, one aggregate per run, with markers \
                       for device detail changes, shared environment changes and \
                       annotations (FR-18). Optionally normalized to a reference run.",
        annotations(read_only_hint = true)
    )]
    async fn trend(
        &self,
        Parameters(req): Parameters<TrendRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(analysis::trend(&self.state.pool, &req).await)
    }

    #[tool(
        description = "Compare side by side along a dimension: runs, client devices or \
                       case parameters (FR-16). Everything else is the matching key. Each \
                       cell has one aggregate per run (samples of different runs are \
                       never pooled); differences use the latest run against the \
                       reference, and cells with another SUT version or shared environment \
                       than the reference are marked.",
        annotations(read_only_hint = true)
    )]
    async fn compare(
        &self,
        Parameters(req): Parameters<CompareRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        collect_comparison(&self.state, req).await
    }

    #[tool(
        description = "Check a derived metric DSL expression: syntax, referenced metrics \
                       and unit consistency (FR-17a). Returns diagnostics with byte \
                       offsets and the inferred unit.",
        annotations(read_only_hint = true)
    )]
    async fn validate_derived_metric(
        &self,
        Parameters(req): Parameters<ValidateExpression>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(derived::validate(&self.state.pool, &req).await)
    }

    #[tool(
        description = "Evaluate a DSL expression or a saved derived metric on every \
                       sample of a case execution. A result can have no value, e.g. a \
                       threshold that was never reached (FR-12a).",
        annotations(read_only_hint = true)
    )]
    async fn evaluate_derived_metric(
        &self,
        Parameters(req): Parameters<EvaluateRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(derived::evaluate(&self.state.pool, &req).await)
    }
}

// Tag and annotation tools (scope `annotate`) ----------------------------------------

#[tool_router(router = annotate_tools, vis = "pub(crate)")]
impl McpServer {
    #[tool(
        description = "Add a tag to a run. Adding an existing tag does nothing.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true
        )
    )]
    async fn add_run_tag(
        &self,
        Parameters(p): Parameters<RunTag>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(
            annotations::add_run_tag(&self.state.pool, p.run_id, &p.tag)
                .await
                .map(|()| json!({ "run_id": p.run_id, "tag": p.tag })),
        )
    }

    #[tool(
        description = "Remove a tag from a run.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true
        )
    )]
    async fn remove_run_tag(
        &self,
        Parameters(p): Parameters<RunTag>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(
            annotations::remove_run_tag(&self.state.pool, p.run_id, &p.tag)
                .await
                .map(|()| json!({ "run_id": p.run_id, "removed": p.tag })),
        )
    }

    #[tool(
        description = "Add a tag to a test case. Adding an existing tag does nothing.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true
        )
    )]
    async fn add_test_case_tag(
        &self,
        Parameters(p): Parameters<TestCaseTag>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(
            annotations::add_test_case_tag(&self.state.pool, p.test_case_id, &p.tag)
                .await
                .map(|()| json!({ "test_case_id": p.test_case_id, "tag": p.tag })),
        )
    }

    #[tool(
        description = "Remove a tag from a test case.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true
        )
    )]
    async fn remove_test_case_tag(
        &self,
        Parameters(p): Parameters<TestCaseTag>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(
            annotations::remove_test_case_tag(&self.state.pool, p.test_case_id, &p.tag)
                .await
                .map(|()| json!({ "test_case_id": p.test_case_id, "removed": p.tag })),
        )
    }

    #[tool(
        description = "Create an annotation on a run, a test case or a point in time \
                       (e.g. \"switched to new compression\").",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    async fn create_annotation(
        &self,
        Parameters(req): Parameters<CreateAnnotation>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(annotations::create(&self.state.pool, &req).await)
    }

    #[tool(
        description = "Replace an annotation's text.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true
        )
    )]
    async fn update_annotation(
        &self,
        Parameters(p): Parameters<EditAnnotation>,
    ) -> Result<CallToolResult, ErrorData> {
        let req = UpdateAnnotation { body: p.body };
        respond(annotations::update(&self.state.pool, p.annotation_id, &req).await)
    }

    #[tool(
        description = "Delete an annotation.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false
        )
    )]
    async fn delete_annotation(
        &self,
        Parameters(p): Parameters<AnnotationId>,
    ) -> Result<CallToolResult, ErrorData> {
        respond(
            annotations::delete(&self.state.pool, p.annotation_id)
                .await
                .map(|()| json!({ "deleted": p.annotation_id })),
        )
    }
}

impl McpServer {
    /// The tools a token with `scope` may use, sorted by name.
    fn visible_tools(&self, scope: TokenScope) -> Vec<Tool> {
        let mut tools = self.tools.read.list_all();
        if scope.allows(TokenScope::Annotate) {
            tools.extend(self.tools.annotate.list_all());
        }
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        tools
    }
}

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "track-record",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS.to_owned())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let scope = scope(&context)?;
        Ok(ListToolsResult {
            tools: self.visible_tools(scope),
            ..Default::default()
        })
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools
            .read
            .get(name)
            .or_else(|| self.tools.annotate.get(name))
            .cloned()
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let scope = scope(&context)?;
        let name = request.name.clone();
        if self.tools.read.has_route(&name) {
            let call = ToolCallContext::new(self, request, context);
            return self.tools.read.call(call).await;
        }
        if self.tools.annotate.has_route(&name) && scope.allows(TokenScope::Annotate) {
            let call = ToolCallContext::new(self, request, context);
            return self.tools.annotate.call(call).await;
        }
        // Tools the token may not use do not exist for it.
        Err(ErrorData::invalid_params(
            format!("tool {name} not found"),
            None,
        ))
    }
}
