#!/usr/bin/env bash
# Deletes old sha- images of a GHCR container package. See
# docs/adr/0007-image-publishing.md.
#
#   .github/scripts/cleanup-images.sh <owner> <package> [keep] [--dry-run]
#
# Of all images with a sha- tag, the newest <keep> (default 10) are kept.
# Older ones are deleted if sha- tags are their only tags; images that also
# carry another tag (develop, main, latest, 1.2.3, ...) are always kept.
#
# A multi-platform image is an index version plus untagged child versions
# (one per platform, plus attestations). The script deletes an index together
# with its children, index first, so a failure leaves only untagged leftovers,
# never a tagged image with missing parts. Children that a kept index also
# references are never deleted. Other untagged versions are left alone: they
# may belong to a publish that is still running.
#
# Needs gh, curl and jq. Authentication: GH_TOKEN, or the gh login. The token
# needs read and delete access to the package (GITHUB_TOKEN with
# packages: write in the workflow; delete:packages for a personal token).
set -euo pipefail

usage() { echo "usage: $0 <owner> <package> [keep] [--dry-run]" >&2; exit 2; }
dry_run=false
args=()
for arg in "$@"; do
    if [ "$arg" = --dry-run ]; then dry_run=true; else args+=("$arg"); fi
done
if [ "${#args[@]}" -lt 2 ] || [ "${#args[@]}" -gt 3 ]; then usage; fi
owner=${args[0]} package=${args[1]} keep=${args[2]:-10}
[[ $keep =~ ^[0-9]+$ ]] || usage

if [ "$(gh api "/users/$owner" --jq .type)" = Organization ]; then
    api="/orgs/$owner/packages/container/$package"
else
    api="/users/$owner/packages/container/$package"
fi

# All versions: id, digest (name), created_at, tags.
versions=$(gh api --paginate "$api/versions" \
    --jq '.[] | {id, digest: .name, created: .created_at, tags: .metadata.container.tags}' \
    | jq --slurp .)

# Images with a sha- tag, newest first, split into kept and deletable.
plan=$(jq --argjson keep "$keep" '
    [.[] | select(any(.tags[]; startswith("sha-")))] | sort_by(.created) | reverse
    | to_entries
    | map(.value + {delete: (.key >= $keep and all(.value.tags[]; startswith("sha-")))})
' <<<"$versions")

# Registry token for reading manifests.
token=$(curl --silent --fail --user "x:${GH_TOKEN:-$(gh auth token)}" \
    "https://ghcr.io/token?scope=repository:$owner/$package:pull" | jq --raw-output .token)

# Digests an image references (its children); none for a single manifest.
children() {
    curl --silent --fail --header "Authorization: Bearer $token" \
        --header 'Accept: application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json' \
        "https://ghcr.io/v2/$owner/$package/manifests/$1" \
        | jq --raw-output '.manifests[]?.digest'
}

# Every version an image consists of: the index, then its children.
image_digests() { echo "$1"; children "$1"; }

# Everything that kept images (with or without a sha- tag) reference.
protected=$(jq --raw-output --argjson plan "$plan" '
    ($plan | map(select(.delete) | .digest)) as $deleting
    | .[] | select((.tags | length) > 0) | select(.digest | IN($deleting[]) | not)
    | .digest' <<<"$versions" \
    | while read -r digest; do image_digests "$digest"; done | sort -u)

deleted=0
while read -r digest tags; do
    [ -n "$digest" ] || continue
    echo "deleting $tags ($digest)"
    while read -r part; do
        if grep --quiet --fixed-strings --line-regexp "$part" <<<"$protected"; then
            echo "  keeping $part: used by a kept image"
            continue
        fi
        id=$(jq --raw-output --arg d "$part" '.[] | select(.digest == $d) | .id' <<<"$versions")
        if [ -z "$id" ]; then
            echo "  $part: no package version (already deleted)"
            continue
        fi
        echo "  version $id ($part)"
        if ! $dry_run; then
            gh api --method DELETE "$api/versions/$id" --silent
        fi
        deleted=$((deleted + 1))
    done < <(image_digests "$digest")
done < <(jq --raw-output '.[] | select(.delete) | .digest + " " + (.tags | join(","))' <<<"$plan")

kept=$(jq '[.[] | select(.delete | not)] | length' <<<"$plan")
if $dry_run; then
    echo "dry run: would delete $deleted version(s); $kept sha- image(s) kept"
else
    echo "deleted $deleted version(s); $kept sha- image(s) kept"
fi
