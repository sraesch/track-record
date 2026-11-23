#!/usr/bin/env bash
# Installs the Helm chart into a real cluster and checks the running stack
# (ADR 0021):
#
#   1. install with the in-cluster database, wait until everything is ready
#   2. helm test (web UI, UI API through the frontend, machine API, and no
#      UI API on the machine port)
#   3. through port-forwards: create a write token on the UI port, upload a
#      run with a case execution, a series and an attachment through the
#      machine API, close it and read everything back
#   4. helm upgrade, which runs the migration job as a pre-upgrade hook;
#      the run is still there afterwards
#   5. a second release on an external database (plain postgres:18): the
#      migration job creates the schema as a pre-install hook
#
#   deploy/helm/install-test.sh
#
# Everything goes into a throwaway namespace, which is deleted at the end
# (KEEP=1 keeps it). The images are the published ones, by default the
# `develop` images (TR_IMAGE_TAG overrides the tag), so this checks the
# chart, not the code of this commit.
#
# Needs helm, kubectl, curl, jq and python3, and a cluster with a default storage
# class: Docker Desktop Kubernetes (the default context) or kind.
# KUBE_CONTEXT overrides the context; it is never the current context by
# accident.
set -euo pipefail

context=${KUBE_CONTEXT:-docker-desktop}
namespace=${TR_TEST_NAMESPACE:-track-record-chart-test}
tag=${TR_IMAGE_TAG:-develop}
release=tr
chart=$(cd "$(dirname "$0")" && pwd)/track-record

k() { kubectl --context "$context" --namespace "$namespace" "$@"; }
h() { helm --kube-context "$context" --namespace "$namespace" "$@"; }
log() { printf '\n==> %s\n' "$*"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

forwards=()
cleanup() {
    status=$?
    for pid in ${forwards[@]+"${forwards[@]}"}; do kill "$pid" 2>/dev/null || true; done
    if [ "$status" -ne 0 ]; then
        echo "--- pods" >&2
        k get pods -o wide >&2 || true
        echo "--- events" >&2
        k get events --sort-by=.lastTimestamp 2>/dev/null | tail -30 >&2 || true
    fi
    if [ "${KEEP:-}" = 1 ]; then
        echo "Kept namespace $namespace (context $context)."
    else
        kubectl --context "$context" delete namespace "$namespace" --ignore-not-found --wait=false >/dev/null
    fi
    exit "$status"
}

kubectl --context "$context" get nodes >/dev/null \
    || fail "Kubernetes context $context is not reachable"
if kubectl --context "$context" get namespace "$namespace" >/dev/null 2>&1; then
    fail "namespace $namespace exists; delete it first (kubectl --context $context delete namespace $namespace)"
fi
trap cleanup EXIT
kubectl --context "$context" create namespace "$namespace" >/dev/null

common=(--values "$chart/ci/install-values.yaml" --set "imageTag=$tag" --wait --timeout 15m)

log "Install $release (images: $tag)"
h install "$release" "$chart" "${common[@]}"
k get pods

log "helm test"
h test "$release" --logs

# Forwards a service port to a free local port and sets the variable named
# $1 to its URL. Not in a subshell, so cleanup knows the process.
forward() {
    local out port=
    out=$(mktemp)
    kubectl --context "$context" --namespace "$namespace" port-forward "svc/$2" ":$3" >"$out" 2>&1 &
    forwards+=($!)
    for _ in $(seq 1 50); do
        port=$(sed -n 's/^Forwarding from 127\.0\.0\.1:\([0-9]*\) .*/\1/p' "$out" | head -1)
        [ -n "$port" ] && break
        sleep 0.2
    done
    [ -n "$port" ] || { cat "$out" >&2; fail "port-forward to $2 did not start"; }
    printf -v "$1" 'http://127.0.0.1:%s' "$port"
}
# A port-forward ends with its pod; start them again after a rollout.
start_forwards() {
    for pid in ${forwards[@]+"${forwards[@]}"}; do kill "$pid" 2>/dev/null || true; done
    forwards=()
    forward ui "$release-track-record-frontend" 80
    forward api "$release-track-record-server-machine" 8080
}
start_forwards

# curl that fails on HTTP errors and shows the response body.
call() {
    local out code
    out=$(mktemp)
    code=$(curl --silent --show-error --max-time 60 --output "$out" --write-out '%{http_code}' "$@")
    if [ "${code:0:1}" != 2 ]; then
        cat "$out" >&2
        # The URL only: the other arguments may hold the token.
        for arg in "$@"; do [[ $arg == http* ]] && url=$arg; done
        fail "HTTP $code for ${url:-?}"
    fi
    cat "$out"
}

log "Upload through the machine API"
token=$(call -X POST "$ui/ui/api/v1/tokens" -H 'content-type: application/json' \
    -d '{"name": "chart install test", "scope": "write"}' | jq -r .secret)
[[ $token == trk_* ]] || fail "no token secret in the response"
auth=(-H "authorization: Bearer $token")
json=(-H 'content-type: application/json')

uuid() { python3 -c 'import uuid; print(uuid.uuid4())'; }
run=$(uuid) ce=$(uuid) sample=$(uuid) series=$(uuid) attachment=$(uuid)

call -X PUT "$api/api/v1/runs/$run" "${auth[@]}" "${json[@]}" -d '{
    "suite": {"name": "chart-test", "definition": {"scenarios": ["load"]}},
    "sut_version": {"commit": "0000000", "branch": "chart-test", "build_config": "release"},
    "shared_environment": {"cluster": "chart-test"},
    "triggered_by": "ci",
    "started_at": "2026-01-01T00:00:00Z",
    "tags": ["chart-test"]
}' >/dev/null
call -X PUT "$api/api/v1/runs/$run/case-executions/$ce" "${auth[@]}" "${json[@]}" -d '{
    "test_case": {
        "scenario": {"name": "load", "definition": {"steps": ["open"]}},
        "parameters": {"size_mb": 10},
        "parameter_units": {"size_mb": "MB"}
    },
    "client_device": {"name": "chart-test-device", "details": {"os": "test"}},
    "metrics": {
        "load_time": {"unit": "s", "value_type": "float", "direction": "lower_is_better"},
        "fps": {"unit": "fps", "value_type": "float", "direction": "higher_is_better"}
    },
    "samples": [{
        "id": "'"$sample"'",
        "repetition": 0,
        "measurements": {"load_time": 1.5},
        "series": [{"id": "'"$series"'", "metric": "fps", "x_dimension": "time", "x_unit": "s",
                    "x": [0, 0.5, 1], "y": [10, 30, 60]}]
    }]
}' >/dev/null
content="attachment written by the chart install test $(date -u +%FT%TZ)"
call -X PUT "$api/api/v1/runs/$run/attachments/$attachment?file_name=install-test.txt" "${auth[@]}" \
    -H 'content-type: text/plain' --data-binary "$content" >/dev/null
call -X POST "$api/api/v1/runs/$run/close" "${auth[@]}" "${json[@]}" -d '{"status": "completed"}' >/dev/null

check_run() {
    local status
    status=$(call "$api/api/v1/runs/$run" "${auth[@]}" | jq -r .status)
    [ "$status" = completed ] || fail "run status is $status"
    [ "$(call "$api/api/v1/attachments/$attachment/content" "${auth[@]}")" = "$content" ] \
        || fail "attachment content differs"
    call "$ui/ui/api/v1/runs/$run" | jq -e '.attachments | length == 1' >/dev/null \
        || fail "the UI API does not show the attachment"
    echo "run $run: completed, attachment readable through both ports"
}
check_run

log "UI-only operations are not on the machine port"
code=$(curl --silent --max-time 60 --output /dev/null --write-out '%{http_code}' -X POST \
    "$api/ui/api/v1/runs/$run/invalidate" "${auth[@]}" "${json[@]}" -d '{"reason": "x"}')
[ "${code:0:1}" = 4 ] || fail "invalidate on the machine port answered $code"
echo "machine port: $code for /ui/api"

log "Upgrade (migration job as a pre-upgrade hook)"
h upgrade "$release" "$chart" "${common[@]}" --set-string 'server.podAnnotations.upgraded=yes'
h history "$release"
# The hook Job is deleted once it succeeded; its event stays.
k get events --field-selector "involvedObject.name=$release-track-record-migrate,reason=Completed" \
    | grep -q 'Job completed' || fail "the migration job did not run on upgrade"
echo "migration job completed"
start_forwards
check_run

log "Second release on an external database (pre-install migration)"
password=external-test-password
k create secret generic external-db --from-literal=password="$password" >/dev/null
k run external-postgres --image=postgres:18 --port=5432 \
    --env=POSTGRES_PASSWORD="$password" --env=POSTGRES_DB=benchmarks >/dev/null
k expose pod external-postgres --port=5432 >/dev/null
k wait --for=condition=Ready pod/external-postgres --timeout=5m >/dev/null
h install "$release-ext" "$chart" "${common[@]}" \
    --set database.enabled=false \
    --set externalDatabase.host=external-postgres \
    --set externalDatabase.database=benchmarks \
    --set externalDatabase.existingSecret=external-db \
    --set externalDatabase.sslMode=disable
h test "$release-ext" --logs
applied=$(k exec external-postgres -- psql -U postgres -d benchmarks -tAc 'SELECT count(*) FROM schema_migration')
[ "$applied" -gt 0 ] || fail "no migrations recorded in the external database"
echo "external database: $applied migrations applied by the job"

log "All install checks passed"
