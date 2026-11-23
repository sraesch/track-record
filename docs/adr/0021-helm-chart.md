# ADR 0021: Helm chart

**Status:** proposed · **Date:** 2026-11-23

## Context

The requirements ask for a Kubernetes deployment with a Helm chart (NFR-1),
with a stateless backend that scales horizontally and a shared
`ReadWriteMany` volume for attachments ([ADR 0003](0003-attachment-storage.md)).
All three images are published to GHCR as `X.Y.Z`, `X.Y`, `develop`, `main`,
`latest` and `sha-<short>` ([ADR 0007](0007-image-publishing.md),
[ADR 0014](0014-server-and-frontend-images.md)). Migrations run with
`track-record-migrate` from the database image, and ADR 0006 planned it as
a hook of the chart. `deploy/local/` runs the stack on Docker Desktop with
plain manifests; it builds the images itself and is not meant for
production.

The access model matters most (NFR-2). The server listens on two ports
([ADR 0008](0008-backend-service.md)): the machine port (`/api`, `/mcp`)
checks API tokens, the UI port (`/ui/api`) has no authentication and is the
only place for invalidation, deletion and token management. The web UI sits
behind an auth proxy; machine clients bypass it. Nothing outside the
cluster may reach the UI port except through the frontend and its proxy.

[ADR 0020](0020-release-process.md) made one version for the whole system,
set in the backend crates, the frontend package and the OpenAPI documents.

## Decision

### Where and which version

- **The chart lives in `deploy/helm/track-record/`**, next to `deploy/local/`.
  Scripts beside it: `test.sh` (static checks), `install-test.sh` (install
  into a cluster) and `publish.sh` (package and push).
- **The chart version is the system version.** `version` and `appVersion`
  in `Chart.yaml` are both `X.Y.Z` and change in the `Prepare X.Y.Z release`
  commit together with the other version files. One number tells which
  images a chart deploys, and there are no chart-only releases: a change to
  the chart alone ships with the next release. `test.sh` fails if
  `version`, `appVersion` and the server crate's version differ.
- **Images default to `appVersion`.** One value, `imageTag`, sets the tag of
  all three images (they are built from one commit); each image can
  override it. A released chart deploys exactly its release. From a checkout
  of `develop`, set `imageTag=develop` (or a `sha-` tag) to run unreleased
  images.

### Publishing

- **OCI to GHCR**, `oci://ghcr.io/<owner>/charts/track-record`, by the CI job
  `publish-chart` for `v*` tags only. It needs the chart checks and the three
  image publishes of the same run, so a published chart never points at
  images that do not exist. `publish.sh` refuses a version that is not the
  one in `Chart.yaml`. The job uses the workflow's `GITHUB_TOKEN`
  (`packages: write`), like the images.
- **No chart for `develop` or `main` pushes.** A chart version must be
  SemVer, and moving tags (`develop`) don't fit chart versions. Unreleased
  states are installed from the repository.
- **No `sha-` cleanup** for the chart: only releases are pushed.
- Not chosen: a classic chart repository (`index.yaml` on GitHub Pages with
  chart-releaser). OCI needs no extra branch or Pages site and uses the
  registry the images are already in.

### What the chart deploys

| Resource | Notes |
|---|---|
| Database StatefulSet (optional) | `track-record-db`, one replica, `ReadWriteOnce` volume. `database.enabled=false` uses `externalDatabase` (any PostgreSQL 18, e.g. managed) instead |
| Migration Job (hook) | `track-record-migrate` from the database image |
| Server Deployment | `replicaCount` or an HPA (NFR-1); init container waits for the database |
| Attachment PVC | `ReadWriteMany` by default, storage class configurable, or an existing claim (ADR 0003) |
| Frontend Deployment | nginx; `TR_UI_API_UPSTREAM` points at the UI port Service |
| Ingresses / HTTPRoutes (optional) | One for machine clients, one for the web UI |
| NetworkPolicies | UI port and database closed to everything but their clients |
| `helm test` Pod | Read-only smoke checks |

- **Connection settings as `PG*` variables.** The server (sqlx) and
  `track-record-migrate` (libpq) read the same `PGHOST`, `PGPORT`,
  `PGDATABASE`, `PGUSER`, `PGPASSWORD` and `PGSSLMODE`. The server's
  `DATABASE_URL` holds only host, port and database, so the password never
  needs URL escaping and comes straight from a Secret.
- **Passwords:** an existing Secret (`existingSecret`), a value, or for the
  in-cluster database a generated one that is looked up and kept on
  upgrades. Renderers without cluster access (`helm template`, GitOps
  tools) cannot look it up and must set one of the other two.
- **Nothing is deleted with the release (FR-8).** The attachment PVC and
  the generated database password carry `helm.sh/resource-policy: keep`,
  and the database volume is retained by the StatefulSet. `helm uninstall`
  leaves the data; a reinstall with the same release name picks it up.
- **Hardened pods:** non-root, read-only root filesystems for the server,
  frontend and migration job (emptyDirs where nginx and `psql` write),
  no service account tokens, dropped capabilities. The server's `fsGroup`
  makes the attachment volume writable for its non-root user.

### Migrations

`track-record-migrate` runs as a Job with Helm hook annotations:

- **`pre-upgrade` always.** New server pods are created only after the
  schema is current, so they never find missing migrations. A failing
  migration stops the upgrade before anything else changes; the failed Job
  stays for its logs.
- **`pre-install` only with an external database.** The in-cluster database
  does not exist before the install, and its image applies all migrations
  itself on first start (ADR 0006). An external database already exists
  and gets its schema before the server starts.
- **Hook Secret.** Hooks run before the release's own resources are created
  or updated. When the chart manages the password, the Job reads it from a
  hook Secret of its own, created just before it and deleted when the hooks
  succeed.
- The Job waits with `pg_isready` until the database answers, bounded by
  `activeDeadlineSeconds`. Concurrent runs are safe (advisory lock,
  ADR 0006).

The server additionally waits for the database in an init container: on a
fresh install with the in-cluster database it would otherwise exit and
restart until the database finished its first start.

### Access (NFR-2)

- **Two Services for the server.** `<release>-server-machine` carries only
  the machine port and is what ingresses and routes point at;
  `<release>-server-ui` carries only the UI port, is always `ClusterIP` and
  is referenced only by the frontend. A misconfigured machine ingress
  cannot reach the UI port, because no Service it may use has it (and the
  machine port does not route UI-only operations either, ADR 0008).
- **Two ingresses**, `ingress.machine` and `ingress.ui`, each with its own
  hosts, class, TLS and annotations. The machine ingress routes `/api` and
  `/mcp` only and bypasses the auth proxy. The UI ingress routes everything
  to the frontend and carries the auth proxy (e.g. an `auth-url`
  annotation). They may share a host if the ingress controller merges
  ingresses per host, or use separate hosts. **Gateway API** `HTTPRoute`s
  (`httpRoute.machine`, `httpRoute.ui`) are the same split for clusters
  that use a Gateway instead.
- **NetworkPolicies on by default.** The UI port accepts only the frontend
  pods (plus `networkPolicy.uiApiFrom`, e.g. an auth proxy that forwards
  `/ui/api` itself); the machine port accepts everyone; the in-cluster
  database accepts only the server and the migration Job. Without a network
  plugin that enforces them, the Service split above still applies.
- `helm test` checks the split on a running release: the web UI and the UI
  API through the frontend answer, the machine API answers, and `/ui/api`
  on the machine port does not.

### Checks

- **`deploy/helm/test.sh`** (CI job `helm`): `helm lint --strict` and
  `helm template` for the default values and each `ci/*-values.yaml`,
  `kubeconform -strict` against the oldest supported and a current
  Kubernetes version, the access rules above on every rendering, errors
  for incomplete values (`values.schema.json` rejects unknown keys), and
  the version check. helm and kubeconform run in pinned containers, so it
  needs only Docker, like the other `test.sh` scripts.
- **`deploy/helm/install-test.sh`** (CI job `helm-install`, on kind):
  installs the chart into a throwaway namespace, runs `helm test`, uploads
  a run with a series and an attachment through the machine API with a
  token created on the UI port, upgrades (running the migration hook) and
  reads the data back, then installs a second release on a plain
  `postgres:18` as an external database (pre-install hook). Locally it
  runs against Docker Desktop Kubernetes. It uses the published `develop`
  images (`TR_IMAGE_TAG`), so it checks the chart, not the images of the
  commit under test.

## Consequences

- A release now also bumps `Chart.yaml` and publishes the chart; ADR 0020
  and the `release` skill list it. The first push creates the GHCR package
  `charts/track-record`, which starts private like the images did.
- Before the first release, the chart's default images (`0.1.0`) do not
  exist yet; installs from `develop` set `imageTag`.
- Reinstalling over a kept database volume with a newer chart does not
  migrate (no pre-install hook for the in-cluster database); run
  `helm upgrade` once more, which does.
- The in-cluster database is a single PostgreSQL pod without replication or
  backups. Production setups with terabytes of results (NFR-3) are
  expected to use `externalDatabase` (a managed or operator-run
  PostgreSQL); the in-cluster one is for small installations and trials.
- The wait-for-database init container pulls the database image (about
  700 MB) onto every node that runs a server pod. `server.waitForDatabase`
  turns it off.
- `ReadWriteOnce` attachment volumes work only while all server pods share
  a node; the default is `ReadWriteMany`, as ADR 0003 requires.
- The install check in CI depends on the published `develop` images and
  on pulling them from GHCR.
- `deploy/local/` stays as it is: it builds images from the working tree
  and pushes them to an in-cluster registry, which the chart does not do.

## Left for later ADRs

Backups (database and attachment volume, ADR 0003), signing the chart and
images, a PodDisruptionBudget and a `ServiceMonitor` once there are
metrics.
