#!/usr/bin/env bash
# Checks the Helm chart without a cluster (ADR 0021): helm lint and
# helm template for the default values and every ci/*-values.yaml,
# kubeconform on the rendered manifests, the access rules of NFR-2, the
# errors for incomplete values, and that the chart version is the server's.
#
#   deploy/helm/test.sh
#
# Needs Docker only: helm and kubeconform run in pinned containers. CI runs
# the same script (job `helm`). deploy/helm/install-test.sh installs the
# chart into a real cluster.
set -euo pipefail

HELM_IMAGE=alpine/helm:4.3.0
KUBECONFORM_IMAGE=ghcr.io/yannh/kubeconform:v0.7.0
# The oldest Kubernetes the chart supports (Chart.yaml kubeVersion) and a
# current one.
KUBE_VERSIONS=(1.28.0 1.33.0)

root=$(cd "$(dirname "$0")/../.." && pwd)
chart="$root/deploy/helm/track-record"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

helm() {
    docker run --rm --volume "$chart:/chart:ro" "$HELM_IMAGE" "$@"
}
log() { printf '\n==> %s\n' "$*"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

docker pull --quiet "$HELM_IMAGE" >/dev/null
docker pull --quiet "$KUBECONFORM_IMAGE" >/dev/null

log "Chart version"
chart_version=$(sed -n 's/^version: *//p' "$chart/Chart.yaml")
app_version=$(sed -n 's/^appVersion: *"\{0,1\}\([^"]*\)"\{0,1\}$/\1/p' "$chart/Chart.yaml")
server_version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/backend/crates/server/Cargo.toml" | head -1)
echo "chart $chart_version, appVersion $app_version, server crate $server_version"
[ "$chart_version" = "$app_version" ] || fail "Chart.yaml version and appVersion differ"
[ "$chart_version" = "$server_version" ] || fail "Chart.yaml version is not the server crate's version (ADR 0020)"

# name=values-file pairs; "default" has no file.
cases=(default)
for f in "$chart"/ci/*-values.yaml; do
    cases+=("$(basename "$f" -values.yaml)")
done

for case in "${cases[@]}"; do
    args=()
    [ "$case" = default ] || args=(--values "/chart/ci/$case-values.yaml")

    log "$case: lint"
    helm lint --strict /chart ${args[@]+"${args[@]}"}

    log "$case: template"
    helm template release /chart --namespace benchmarks ${args[@]+"${args[@]}"} >"$work/$case.yaml"

    for version in "${KUBE_VERSIONS[@]}"; do
        log "$case: kubeconform (Kubernetes $version)"
        docker run --rm --interactive "$KUBECONFORM_IMAGE" \
            -strict -summary -kubernetes-version "$version" \
            -schema-location default \
            -schema-location 'https://raw.githubusercontent.com/datreeio/CRDs-catalog/main/{{.Group}}/{{.ResourceKind}}_{{.ResourceAPIVersion}}.json' \
            - <"$work/$case.yaml"
    done
done

# Splits rendered manifests into one file per document, named <kind>-<n>.
split_docs() {
    local dir=$1
    mkdir -p "$dir"
    awk -v dir="$dir" '
        /^---/ { n++; next }
        { doc[n] = doc[n] $0 "\n"; if ($1 == "kind:" && !kind[n]) kind[n] = $2 }
        END { for (i in doc) if (kind[i]) printf "%s", doc[i] > (dir "/" kind[i] "-" i) }
    ' "$2"
}

log "Access rules (NFR-2)"
for case in "${cases[@]}"; do
    split_docs "$work/docs-$case" "$work/$case.yaml"
    # No ingress or route may reach the UI port or its Service.
    for doc in "$work/docs-$case"/Ingress-* "$work/docs-$case"/HTTPRoute-*; do
        [ -e "$doc" ] || continue
        if grep -Eq 'server-ui|8081|name: ui$' "$doc"; then
            fail "$case: $(basename "$doc") references the UI port"
        fi
        # Machine routes carry /api and /mcp only.
        if grep -q 'server-machine' "$doc"; then
            paths=$(grep -E '^ *(- )?(path|value): /' "$doc" | sed 's/.*: //' | sort -u | tr '\n' ' ')
            [ "$paths" = "/api /mcp " ] || fail "$case: machine route has paths: $paths"
        fi
    done
    # The UI port's Service stays inside the cluster.
    for doc in "$work/docs-$case"/Service-*; do
        if grep -q 'port: 8081' "$doc"; then
            grep -q 'type: ClusterIP' "$doc" || fail "$case: the UI port Service is not ClusterIP"
            grep -q 'port: 8080' "$doc" && fail "$case: a Service carries both ports"
        fi
    done
    echo "$case: ok"
done

log "Incomplete values are refused"
expect_error() {
    local message=$1
    shift
    if out=$(helm template release /chart "$@" 2>&1); then
        fail "rendered without error: $*"
    fi
    grep -q "$message" <<<"$out" || fail "expected \"$message\" for $*, got: $out"
    echo "ok: $message"
}
expect_error "externalDatabase.host is required" --set database.enabled=false --set externalDatabase.password=x
expect_error "externalDatabase.password or externalDatabase.existingSecret is required" \
    --set database.enabled=false --set externalDatabase.host=db
expect_error "ingress.machine.hosts must list at least one host" --set ingress.machine.enabled=true
expect_error "httpRoute.ui.parentRefs is required" --set httpRoute.ui.enabled=true
expect_error "additional properties .replicas. not allowed" --set server.replicas=3

log "All chart checks passed"
