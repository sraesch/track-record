# Changelog

All notable changes to track-record are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and releases use
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Requirement
IDs (`FR-n`, `NFR-n`) refer to [docs/requirements.md](docs/requirements.md).

[frontend/APP_CHANGELOG.md](frontend/APP_CHANGELOG.md) is a shorter version
of this file for the people who use the web UI. Update it with every
change that they can see.

## [Unreleased]

## [0.1.0] - 2026-11-23

### Added

#### Database

- PostgreSQL schema for definitions, dimensions, results, attachments,
  annotations and API tokens, with pgTAP tests (`db/test.sh`)
  ([ADR 0004](docs/adr/0004-database-schema.md),
  [#1](https://github.com/sraesch/track-record/pull/1)). The schema enforces
  the core invariants itself:
  - Results use client-generated UUIDs, so retried uploads hit the existing
    row (FR-3).
  - Results are immutable through triggers (FR-6, FR-8). The only hard
    delete is `delete_run()` (FR-10).
  - Suites, scenarios and aliases are versioned and deduplicated by content
    through the `ensure_*` functions (FR-7).
  - Stale running runs are aborted by server time since the last upload,
    never by their measured start time (FR-2).
  - Only raw values are stored: finite measurements, one series per sample
    and metric, and x never decreases (ADR 0002).
  - A unit catalogue with conversion factors; numeric parameters can carry a
    unit (FR-13a).
  - Alias values keep their JSON type (FR-14).
- Database image `track-record-db` on PostgreSQL 18, which applies all
  migrations on first start ([ADR 0006](docs/adr/0006-database-image.md),
  [#2](https://github.com/sraesch/track-record/pull/2)). For upgrades it runs
  as the migration job `track-record-migrate`.
- `db/migrate.sh` applies pending migrations, one transaction each, and
  records them with a checksum. It refuses changed or missing migrations and
  is safe to run concurrently (#2).

#### Backend

- Rust backend with two ports
  ([ADR 0008](docs/adr/0008-backend-service.md),
  [#5](https://github.com/sraesch/track-record/pull/5)):
  - The machine port serves `/api/v1` with API tokens (`read`, `annotate`,
    `write` scopes).
  - The UI port serves `/ui/api/v1` without tokens and is the only place
    for invalidation, hard delete, tokens, aliases and changes to metric
    definitions (NFR-2).
- Incremental, idempotent ingestion with `PUT` on client UUIDs (FR-1 to
  FR-6): runs, case executions with inline samples and series, samples,
  series, attachments and closing a run. An identical retry answers 200,
  different content 409 (#5).
- Reading runs with the FR-11 filters and cursor paging; run and case
  execution details, series points, attachment downloads and definition
  lists. Aliases apply in one shared query layer (FR-14) (#5).
- Tags and annotations, invalidation (FR-9), hard delete with the run ID as
  confirmation (FR-10), API token management, and a job that aborts stale
  runs (FR-2) (#5).
- The derived metric DSL in `track-record-core::dsl`: parser, type and unit
  check, evaluation and composition (FR-17, FR-17a)
  ([ADR 0009](docs/adr/0009-derived-metric-dsl-and-analysis.md),
  [#6](https://github.com/sraesch/track-record/pull/6)). A derived metric
  can have no value, which is counted, not treated as missing (FR-12a).
- Statistics and series operations: mean, median, percentiles, standard
  deviation and confidence intervals; interpolation, reductions,
  resampling and downsampling (#6).
- Analysis endpoints that take a view spec with a run filter and a DSL
  expression as metric (FR-21) (#6):
  - aggregates per run, case and device (FR-12);
  - trends with device, environment and SUT version markers and annotations
    (FR-18);
  - comparisons along run, device or parameters, streamed as NDJSON and
    never pooling runs (FR-16, NFR-7);
  - series overlays and percentile bands (FR-19);
  - streamed CSV, NDJSON and Parquet export (FR-20).
- Series points can be downsampled with `?max_points=` (FR-19) (#6).
- MCP server at `/mcp` on the machine port (rmcp, stateless)
  ([#7](https://github.com/sraesch/track-record/pull/7)). Every token scope
  gets the read and analysis tools; only `annotate` tokens get the tag and
  annotation tools. There are no tools for uploads, invalidation, deletion,
  tokens or aliases.
- OpenAPI documents generated from code in `backend/openapi/`, checked by a
  test (#5).

#### Web UI

- The app shell: app bar, a side menu with every view, and light, dark and
  system color modes. On narrow screens the menu slides in over the page
  ([ADR 0010](docs/adr/0010-frontend-scaffold.md),
  [ADR 0011](docs/adr/0011-ui-shell.md),
  [#9](https://github.com/sraesch/track-record/pull/9)). Every view has a
  URL that reproduces it, filters included (FR-21).
- **Run list** with status, validity, SUT version, trigger and tags (FR-11)
  (#9, [#13](https://github.com/sraesch/track-record/pull/13)). It can be
  filtered by suite, status, trigger, branch, device, tags, date range,
  validity, commit, SUT tag, build configuration, scenario, asset, metric,
  parameters and shared environment. Only valid runs are shown by default
  (FR-9).
- **Run detail** ([#10](https://github.com/sraesch/track-record/pull/10)):
  - overview, tags, annotations, case executions and shared environment;
  - mark a run invalid and valid again (FR-9), and hard delete after typing
    the run ID (FR-10);
  - attachment previews in a dialog or on their own page: images, Markdown
    without its HTML, JSON and text
    ([ADR 0013](docs/adr/0013-attachment-previews.md)). Uploaded HTML is
    never rendered, and every file can be downloaded.
- **Case execution page**
  ([#12](https://github.com/sraesch/track-record/pull/12), #13, #17):
  - measurements per sample, with warmup samples dimmed (FR-12);
  - a chart per series metric, with a line per sample
    ([ADR 0015](docs/adr/0015-charting-library.md), Apache ECharts);
  - device details, attachments, and the link between a superseded upload
    and its correction (FR-6);
  - test case tags and annotations, which every execution of the case
    shows;
  - derived metric values, including no value and missing inputs (FR-12a).
- **Aliases page** for client device names and parameter values (FR-14)
  (25f0fa9). Aliases can be added, changed, removed and set again, and
  parameter values keep their JSON type.
- **Metrics page** with one definition per metric, the unit catalogue and
  the custom units the results use (FR-7, FR-13a)
  ([#16](https://github.com/sraesch/track-record/pull/16), #17). Metrics
  can be edited, renamed and deleted.
- **Derived metrics page**
  ([#17](https://github.com/sraesch/track-record/pull/17)):
  - lists each metric with what it uses and what uses it;
  - an editor that the server checks while you type (FR-17a), with a
    preview on a case execution
    ([ADR 0016](docs/adr/0016-expression-editor.md), CodeMirror 6);
  - create, change, rename and delete.
- **Suites & scenarios page** with the versions of each definition and what
  each one changed (FR-7)
  ([#18](https://github.com/sraesch/track-record/pull/18)). It also lists
  the test cases of a scenario version, with their tags and annotations.
- **API tokens page**: list, create with the secret shown once, and revoke
  (NFR-2) ([#19](https://github.com/sraesch/track-record/pull/19)).
- App icon as favicon and in the app bar.
- **About page** at `/about`, opened from the app bar, which shows
  [frontend/APP_CHANGELOG.md](frontend/APP_CHANGELOG.md) formatted as
  Markdown.
- `npm run seed` fills an empty database with test data for UI development
  through the public APIs (#9).

#### Deployment and CI

- GitHub Actions CI for the database, backend and frontend, running the
  same scripts as local development
  ([ADR 0005](docs/adr/0005-ci.md), #1, #5, #9).
- The images `track-record-db`, `track-record-server` and
  `track-record-frontend` are published to GHCR for amd64 and arm64 on
  pushes to `develop`, `main` and `v*` tags
  ([ADR 0007](docs/adr/0007-image-publishing.md),
  [ADR 0014](docs/adr/0014-server-and-frontend-images.md),
  [#3](https://github.com/sraesch/track-record/pull/3),
  [#11](https://github.com/sraesch/track-record/pull/11)). Build stages
  cross-compile instead of running under emulation.
- Old `sha-` images beyond the newest 10 are deleted after each publish
  ([#4](https://github.com/sraesch/track-record/pull/4)).
- The server image runs on `distroless/cc` as a non-root user
  ([#8](https://github.com/sraesch/track-record/pull/8)).
- The frontend image is nginx, serving the UI and proxying `/ui/api`
  without buffering, so streamed results pass through (NFR-7) (#9).
- `deploy/local/` runs the whole stack on Docker Desktop Kubernetes for
  trying the UI (#9). This is not the Helm chart.
- A Helm chart for the whole stack, `deploy/helm/track-record/`
  ([ADR 0021](docs/adr/0021-helm-chart.md), NFR-1, NFR-2): the published
  images, an optional in-cluster database (or an external PostgreSQL 18),
  `track-record-migrate` as a pre-upgrade hook (pre-install too with an
  external database), the server scaled horizontally on a `ReadWriteMany`
  attachment volume, and separate ingresses or Gateway API routes for
  machine clients (`/api`, `/mcp`) and the web UI. The UI port has its own
  cluster-internal Service, and NetworkPolicies let only the frontend reach
  it. Data volumes and the generated database password survive
  `helm uninstall` (FR-8). `deploy/helm/test.sh` (lint, template,
  kubeconform, access rules) and `deploy/helm/install-test.sh` (install,
  upload, upgrade on a real cluster) run in CI, and release tags publish
  the chart to `oci://ghcr.io/sraesch/charts/track-record`.
- A release process: `develop` is merged into `main` and tagged `vX.Y.Z`,
  and a GitHub Release carries the matching section of this file
  ([ADR 0020](docs/adr/0020-release-process.md)). The steps are the Claude
  Code skill `release` in `.claude/skills/release/`.

### Changed

These changes happened on `develop` before this release. They matter only
for databases that ran a `develop` image; the first one concerns only the
web UI.

- The web UI is built on the **Graphite** design system instead of MUI
  ([ADR 0019](docs/adr/0019-graphite-design-system.md)): denser pages,
  dark by default where the system is dark, Lucide icons and the Geist
  fonts bundled with the app (no requests to Google Fonts). On wide screens
  the side menu collapses to a rail of icons instead of disappearing.

- Derived metrics are no longer versioned: each has one definition, which
  can be changed (FR-7, FR-17,
  [ADR 0017](docs/adr/0017-derived-metrics-without-versions.md), #17).
  `derived.<name>` is resolved by name when it is evaluated, so a change
  reaches every analysis, historical runs included. Migration 0011 keeps
  the latest version of each derived metric and drops the older ones.
- Uploaded metrics are no longer versioned
  ([ADR 0018](docs/adr/0018-metrics-without-versions.md), #17). Results
  refer to the metric, so stored values are read in its current unit and
  are never rewritten. Migration 0013 keeps the latest version of each
  metric. From now on, an upload with another unit or value type for an
  existing metric is rejected (409 `metric_definition_conflict`).

### Fixed

- A dialog no longer moves the focus to its first field while it fades in,
  so text typed into another field during that time stays where it was
  typed ([#14](https://github.com/sraesch/track-record/pull/14)).
- Frontend tests stay within their timeouts on loaded machines
  ([#15](https://github.com/sraesch/track-record/pull/15)).

[Unreleased]: https://github.com/sraesch/track-record/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/sraesch/track-record/releases/tag/v0.1.0
