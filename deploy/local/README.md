# Local stack on Docker Desktop Kubernetes

Runs the whole stack (database, migrations, server, web UI) in the local
Kubernetes cluster of Docker Desktop, to try the UI against a real backend.
Plain manifests (`base/`) with kustomize; the production Helm chart is still open.
**Local development only:** fixed database password, single replicas, no
auth proxy.

## Use

Needs Docker Desktop with Kubernetes enabled (context `docker-desktop`) and
`kubectl`. From the repository root:

```bash
deploy/local/up.sh
```

```bash
deploy/local/forward.sh
```

| URL | What |
|---|---|
| http://localhost:8088 | Web UI; the UI API is under `/ui/api` (no tokens) |
| http://localhost:8089/api | Machine API (API tokens), MCP at `/mcp` |

Run `up.sh` again after code changes: it rebuilds the images and rolls out
only those that changed. `forward.sh` reconnects by itself when pods are
replaced. New migrations are applied on the next rollout of
the server (init container). Data survives redeploys.

```bash
deploy/local/down.sh
```

removes everything, including the database and attachment volumes.

### Test data

With `forward.sh` running, fill the empty database with about a month of
realistic runs (see [frontend/README.md](../../frontend/README.md#test-data)):

```bash
cd frontend && npm run seed
```

### An API token for the harness or MCP

Tokens are created on the UI port, e.g. through the UI's proxy:

```bash
curl -s -X POST localhost:8088/ui/api/v1/tokens -H 'content-type: application/json' -d '{"name": "local harness", "scope": "write"}'
```

The response holds the secret (`trk_...`) once. Use it as
`Authorization: Bearer trk_...` on `localhost:8089/api` and `/mcp`.

## What runs

Namespace `track-record`:

| Workload | Image | Notes |
|---|---|---|
| `db` StatefulSet | `track-record-db` ([db/](../../db/README.md)) | PostgreSQL 18; schema applied on first start; 5 Gi volume |
| `server` Deployment | `track-record-server` ([backend/](../../backend/README.md)) | Init container runs `track-record-migrate`; attachments on a 5 Gi volume |
| `frontend` Deployment | `track-record-frontend` ([frontend/](../../frontend/README.md)) | nginx serving the app and proxying `/ui/api` to `server:8081` |

Namespace `track-record-registry`: an image registry (NodePort 30500,
`emptyDir`).

## Why the registry

Docker Desktop's kind-based Kubernetes does not see images built with
`docker build`. `up.sh` pushes them to the in-cluster registry instead, tagged
by content (layers and config), and writes a kustomize overlay (`.generated/`) that pins those
tags. The Docker daemon runs in Docker Desktop's VM and cannot reach a
`kubectl port-forward` on the Mac, so a relay container (`socat` on the VM's
host network) exposes the registry on the daemon's `localhost:30500`, where
Docker allows plain HTTP. The node pulls from its own `localhost:30500`.
