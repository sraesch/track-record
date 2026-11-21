#!/usr/bin/env bash
# Removes the local stack and the in-cluster registry, including the database
# and attachment volumes. KUBE_CONTEXT overrides the context.
set -euo pipefail

context=${KUBE_CONTEXT:-docker-desktop}
kubectl --context "$context" delete namespace track-record track-record-registry --ignore-not-found
docker rm --force track-record-registry-relay >/dev/null 2>&1 || true
rm -rf "$(cd "$(dirname "$0")" && pwd)/.generated"
