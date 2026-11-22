//! HTTP routers for the two listeners (ADR 0008).
//!
//! - **Machine port:** `/api/v1/...` with API tokens. Read endpoints accept
//!   any scope, tag/annotation endpoints need `annotate`, ingestion needs
//!   `write`. UI-only endpoints are not routed here at all. `/mcp` is the MCP
//!   server (any scope; the scope decides which tools exist).
//! - **UI port:** `/ui/api/v1/...` without tokens, behind the auth proxy:
//!   read, annotate and the UI-only endpoints (invalidate, delete, tokens,
//!   aliases). No ingestion.
//!
//! Both serve `/healthz` and `/readyz`, and their own OpenAPI document.

pub mod analysis;
pub mod annotate;
pub mod read;
pub mod ui;
pub mod write;

use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use axum::{Json, Router};
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::openapi::{OpenApi as OpenApiDoc, SecurityRequirement};
use utoipa::{OpenApi, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::{require_annotate, require_read, require_write};
use crate::error::Problem;

pub const MACHINE_PREFIX: &str = "/api/v1";
pub const UI_PREFIX: &str = "/ui/api/v1";

#[derive(OpenApi)]
#[openapi(
    info(
        title = "track-record",
        description = "Benchmark results store. See docs/requirements.md for the FR/NFR ids \
                       referenced in operation descriptions."
    ),
    components(schemas(Problem)),
    tags(
        (name = "ingestion", description = "Incremental, idempotent upload of results (FR-1 to FR-5)"),
        (name = "runs", description = "Runs and their results"),
        (name = "definitions", description = "Definitions and dimensions"),
        (name = "annotations", description = "Tags and annotations"),
        (name = "analysis", description = "Aggregates, trends, comparisons, series views and export (FR-12 to FR-20)"),
        (name = "ui", description = "Web UI only: never reachable with an API token"),
    )
)]
struct BaseDoc;

fn read_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(read::list_runs))
        .routes(routes!(read::get_run))
        .routes(routes!(read::list_case_executions))
        .routes(routes!(read::list_attachments))
        .routes(routes!(read::get_case_execution))
        .routes(routes!(read::get_series))
        .routes(routes!(read::get_series_points))
        .routes(routes!(read::get_attachment))
        .routes(routes!(read::download_attachment))
        .routes(routes!(read::list_suites))
        .routes(routes!(read::list_scenarios))
        .routes(routes!(read::list_metrics))
        .routes(routes!(read::list_units))
        .routes(routes!(read::list_client_devices))
        .routes(routes!(read::get_environment))
        .routes(routes!(read::list_sut_versions))
        .routes(routes!(read::list_assets))
        .routes(routes!(read::list_test_cases))
        .routes(routes!(read::get_test_case))
        .routes(routes!(read::list_aliases))
        .routes(routes!(read::list_annotations))
        .routes(routes!(read::get_annotation))
        .routes(routes!(analysis::list_derived_metrics))
        .routes(routes!(analysis::get_derived_metric))
        .routes(routes!(analysis::validate_expression))
        .routes(routes!(analysis::evaluate_expression))
        .routes(routes!(analysis::aggregates))
        .routes(routes!(analysis::trend))
        .routes(routes!(analysis::compare))
        .routes(routes!(analysis::series_overlay))
        .routes(routes!(analysis::series_aggregate))
        .routes(routes!(analysis::export))
}

fn annotate_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(annotate::add_run_tag, annotate::remove_run_tag))
        .routes(routes!(
            annotate::add_test_case_tag,
            annotate::remove_test_case_tag
        ))
        .routes(routes!(annotate::create_annotation))
        .routes(routes!(
            annotate::update_annotation,
            annotate::delete_annotation
        ))
}

fn write_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(write::create_run))
        .routes(routes!(write::close_run))
        .routes(routes!(write::create_case_execution))
        .routes(routes!(write::create_sample))
        .routes(routes!(write::create_series))
        .routes(routes!(write::upload_attachment))
}

fn ui_only_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(ui::invalidate_run))
        .routes(routes!(ui::validate_run))
        .routes(routes!(ui::delete_run))
        .routes(routes!(ui::list_tokens, ui::create_token))
        .routes(routes!(ui::revoke_token))
        .routes(routes!(ui::set_alias))
        .routes(routes!(ui::save_derived_metric, ui::delete_derived_metric))
        .routes(routes!(ui::rename_derived_metric))
        .routes(routes!(ui::save_metric, ui::delete_metric))
        .routes(routes!(ui::rename_metric))
}

#[derive(serde::Serialize, ToSchema)]
struct Health {
    status: &'static str,
}

async fn healthz() -> Json<Health> {
    Json(Health { status: "ok" })
}

async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<Health>) {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => (StatusCode::OK, Json(Health { status: "ok" })),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(Health {
                status: "database unavailable",
            }),
        ),
    }
}

/// The router routes and OpenAPI document of the machine port.
pub fn machine_api(state: &AppState) -> (Router<AppState>, OpenApiDoc) {
    let v1 = read_routes()
        .route_layer(from_fn_with_state(state.clone(), require_read))
        .merge(annotate_routes().route_layer(from_fn_with_state(state.clone(), require_annotate)))
        .merge(
            write_routes()
                .route_layer(from_fn_with_state(state.clone(), require_write))
                .layer(DefaultBodyLimit::max(state.max_body_bytes)),
        );
    let (router, mut doc) = OpenApiRouter::with_openapi(BaseDoc::openapi())
        .nest(MACHINE_PREFIX, v1)
        .split_for_parts();
    doc.info.title = "track-record machine API".to_owned();
    let components = doc.components.get_or_insert_with(Default::default);
    components.add_security_scheme(
        "bearer",
        SecurityScheme::Http(
            HttpBuilder::new()
                .scheme(HttpAuthScheme::Bearer)
                .description(Some("API token (trk_...) created in the web UI"))
                .build(),
        ),
    );
    doc.security = Some(vec![SecurityRequirement::new(
        "bearer",
        Vec::<String>::new(),
    )]);
    doc.tags
        .get_or_insert_with(Vec::new)
        .retain(|t| t.name != "ui");
    (router, doc)
}

/// The router routes and OpenAPI document of the UI port.
pub fn ui_api(state: &AppState) -> (Router<AppState>, OpenApiDoc) {
    let v1 = read_routes()
        .merge(annotate_routes())
        .merge(ui_only_routes())
        .layer(DefaultBodyLimit::max(state.max_body_bytes));
    let (router, mut doc) = OpenApiRouter::with_openapi(BaseDoc::openapi())
        .nest(UI_PREFIX, v1)
        .split_for_parts();
    doc.info.title = "track-record UI API".to_owned();
    doc.tags
        .get_or_insert_with(Vec::new)
        .retain(|t| t.name != "ingestion");
    (router, doc)
}

/// The machine listener's app.
pub fn machine_router(state: AppState) -> Router {
    let (router, doc) = machine_api(&state);
    let mcp = Router::new()
        .route_service("/mcp", crate::mcp::service(state.clone()))
        .layer(from_fn_with_state(state.clone(), require_read));
    router
        .merge(mcp)
        .route("/api/openapi.json", get(move || async move { Json(doc) }))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .fallback(not_found)
        .with_state(state)
}

/// The UI listener's app.
pub fn ui_router(state: AppState) -> Router {
    let (router, doc) = ui_api(&state);
    router
        .route(
            "/ui/api/openapi.json",
            get(move || async move { Json(doc) }),
        )
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .fallback(not_found)
        .with_state(state)
}

async fn not_found() -> crate::error::ApiError {
    crate::error::ApiError::not_found("no such endpoint")
}
