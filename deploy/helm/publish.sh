#!/usr/bin/env bash
# Packages the Helm chart and pushes it to an OCI registry (ADR 0021). CI
# runs it for release tags (job `publish-chart`):
#
#   deploy/helm/publish.sh 1.2.3 oci://ghcr.io/sraesch/charts
#
# The version must be the one in Chart.yaml (version and appVersion), which
# the `Prepare X.Y.Z release` commit sets (ADR 0020); the script refuses
# anything else. REGISTRY_USER and REGISTRY_PASSWORD log in to the
# registry. Without REGISTRY_PASSWORD it only packages and lints the chart,
# which is a dry run of the release.
#
# Needs Docker only: helm runs in the same pinned container as test.sh.
set -euo pipefail

HELM_IMAGE=alpine/helm:4.3.0

[ $# -eq 2 ] || { echo "usage: $0 VERSION oci://REGISTRY/PATH" >&2; exit 2; }
version=$1
target=$(tr '[:upper:]' '[:lower:]' <<<"$2")
[[ $target == oci://* ]] || { echo "the target must be an oci:// URL" >&2; exit 2; }

root=$(cd "$(dirname "$0")/../.." && pwd)
chart="$root/deploy/helm/track-record"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

chart_version=$(sed -n 's/^version: *//p' "$chart/Chart.yaml")
app_version=$(sed -n 's/^appVersion: *"\{0,1\}\([^"]*\)"\{0,1\}$/\1/p' "$chart/Chart.yaml")
if [ "$chart_version" != "$version" ] || [ "$app_version" != "$version" ]; then
    echo "Chart.yaml has version $chart_version and appVersion $app_version, not $version" >&2
    exit 1
fi

helm() {
    docker run --rm --interactive \
        --volume "$chart:/chart:ro" --volume "$work:/work" \
        --env HELM_REGISTRY_CONFIG=/work/registry.json \
        "$HELM_IMAGE" "$@"
}

helm lint --strict /chart
helm package /chart --destination /work
package="track-record-$version.tgz"
[ -f "$work/$package" ] || { echo "helm package did not write $package" >&2; exit 1; }

if [ -z "${REGISTRY_PASSWORD:-}" ]; then
    echo "Packaged $package; REGISTRY_PASSWORD is not set, so nothing is pushed (dry run)."
    exit 0
fi

registry=${target#oci://}
registry=${registry%%/*}
printf '%s' "$REGISTRY_PASSWORD" \
    | helm registry login "$registry" --username "${REGISTRY_USER:?REGISTRY_USER is not set}" --password-stdin
helm push "/work/$package" "$target"
echo "Pushed $target/track-record:$version"
