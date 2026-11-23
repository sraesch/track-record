# Helm chart

`track-record/` deploys the whole stack from the published images: the
database (optional), the migration job, the server and the web UI. The
design is in [ADR 0021](../../docs/adr/0021-helm-chart.md); every value is
explained in [values.yaml](track-record/values.yaml).

```
deploy/helm/
  track-record/       The chart (values.schema.json checks the values)
    ci/               Values files that test.sh renders and checks
  test.sh             Lint, template, kubeconform, access rules (needs Docker)
  install-test.sh     Installs into a cluster and checks the running stack
  publish.sh          Packages the chart and pushes it to an OCI registry
```

## Install

Releases are published as OCI charts; the chart version is the release
version and deploys the images of that release:

```bash
helm install track-record oci://ghcr.io/sraesch/charts/track-record --version 0.1.0 --namespace track-record --create-namespace
```

From a checkout (e.g. `develop`, before a release), set the image tag:

```bash
helm install track-record deploy/helm/track-record --set imageTag=develop --namespace track-record --create-namespace
```

```bash
helm test track-record --namespace track-record
```

Without an ingress, reach it with port-forwards (`helm install` prints
them).

## Exposing it

The server has two ports (NFR-2, [ADR 0008](../../docs/adr/0008-backend-service.md)):

| Port | Paths | Auth | Exposed by |
|---|---|---|---|
| machine (8080) | `/api`, `/mcp` | API tokens | `ingress.machine` / `httpRoute.machine`, Service `<release>-server-machine` |
| UI (8081) | `/ui/api` | none | only through the frontend, Service `<release>-server-ui` (always ClusterIP) |

The web UI (frontend) belongs behind your auth proxy; machine clients
bypass it and authenticate with tokens. A setup with ingress-nginx and
oauth2-proxy, for example:

```yaml
ingress:
  machine:
    enabled: true
    className: nginx
    hosts: [bench-api.example.com]
    tls: [{hosts: [bench-api.example.com], secretName: bench-api-tls}]
    annotations:
      nginx.ingress.kubernetes.io/proxy-body-size: 256m      # uploads (TR_MAX_BODY_BYTES)
      nginx.ingress.kubernetes.io/proxy-read-timeout: "600"
  ui:
    enabled: true
    className: nginx
    hosts: [bench.example.com]
    tls: [{hosts: [bench.example.com], secretName: bench-tls}]
    annotations:
      nginx.ingress.kubernetes.io/auth-url: https://auth.example.com/oauth2/auth
      nginx.ingress.kubernetes.io/auth-signin: https://auth.example.com/oauth2/start?rd=$scheme://$host$request_uri
      nginx.ingress.kubernetes.io/proxy-read-timeout: "600"  # streamed comparisons and exports (NFR-7)
      nginx.ingress.kubernetes.io/proxy-buffering: "off"
server:
  config:
    mcpAllowedHosts: [bench-api.example.com]
```

With the Gateway API, use `httpRoute.machine` and `httpRoute.ui` with
`parentRefs` to your Gateways instead. Never route the UI port or
`/ui/api` past the auth proxy: it can invalidate and delete runs and
manage tokens. NetworkPolicies (on by default) let only the frontend reach
the UI port.

## Database

By default the chart runs PostgreSQL 18 with the schema
(`track-record-db`) as a single pod with a 50 Gi volume and a generated
password. That suits trials and small installations. For production
volumes (NFR-3), use a managed or operator-run PostgreSQL 18:

```yaml
database:
  enabled: false
externalDatabase:
  host: postgres.example.internal
  database: track_record      # must exist
  user: track_record          # owns the schema
  existingSecret: track-record-db
  secretKey: password
  sslMode: verify-full        # PGSSLROOTCERT via server.extraEnv/extraVolumes
```

Migrations run as the `track-record-migrate` job, a Helm hook before every
upgrade and, with an external database, before the install. A failed job
stops the upgrade and stays for its logs:

```bash
kubectl --namespace track-record logs job/track-record-migrate
```

## Attachments

The attachment volume is shared by all server pods, so it needs a
`ReadWriteMany` storage class (`attachments.persistence.storageClass`, e.g.
NFS, CephFS, EFS, Azure Files) or an existing claim
(`attachments.persistence.existingClaim`). `ReadWriteOnce` works only while
all server pods run on one node. The server runs as user 65532; the pod's
`fsGroup` makes the volume writable, except on NFS drivers that ignore it:
give the share to that user there.

## Uninstall

`helm uninstall` keeps the data (FR-8): the attachment PVC, the database
PVC (`data-<release>-track-record-database-0`) and the generated database
password Secret stay. A reinstall with the same release name uses them
again. Delete them by hand to remove the data for good.

## Checks

```bash
deploy/helm/test.sh
```

lints and renders the chart for the default values and each
`ci/*-values.yaml`, validates the manifests with kubeconform, checks that no
ingress or route reaches the UI port, that incomplete values are refused,
and that the chart version is the server's. Needs Docker only.

```bash
deploy/helm/install-test.sh
```

installs the chart into Docker Desktop Kubernetes (`KUBE_CONTEXT` for
another cluster, e.g. kind) in a throwaway namespace, runs `helm test`,
uploads a run with an attachment through the machine API, upgrades and
reads it back, then installs a second release on an external
`postgres:18`. It uses the published `develop` images (`TR_IMAGE_TAG`).
Needs helm, kubectl, curl, jq and python3. CI runs both (jobs `helm` and
`helm-install`).

## Releases

`version` and `appVersion` in `Chart.yaml` are the release version
([ADR 0020](../../docs/adr/0020-release-process.md)) and are set in the
`Prepare X.Y.Z release` commit. For the tag `vX.Y.Z`, CI publishes the
chart to `oci://ghcr.io/sraesch/charts/track-record` after the images
(`publish.sh`; without `REGISTRY_PASSWORD` it is a dry run):

```bash
deploy/helm/publish.sh 0.1.0 oci://ghcr.io/sraesch/charts
```
