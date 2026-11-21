# ADR 0007: Publishing container images to GHCR

**Status:** proposed · **Date:** 2026-11-21

## Context

The database image `track-record-db` exists and is checked in CI
([ADR 0006](0006-database-image.md)), but it is not published. A Kubernetes
deployment (NFR-1) needs it in a registry. [ADR 0005](0005-ci.md) left the
registry, image tags and release versioning open. The repository is public on
GitHub.

## Decision

- **Registry:** GitHub Container Registry, `ghcr.io/<owner>/track-record-db`.
  CI logs in with the workflow's `GITHUB_TOKEN`; no extra secrets.
- **Publishing is a job in the CI workflow**, `publish-database-image`. It
  runs only for pushes and only after the database tests and the image
  checks passed. Pull requests never publish.
- **Triggers:** pushes to `develop` and `main`, and version tags `v*`
  (semantic versions, e.g. `v0.1.0`).
- **Tags:**

  | Push | Image tags |
  |---|---|
  | `develop` | `develop`, `sha-<short>` |
  | `main` | `main`, `latest`, `sha-<short>` |
  | tag `v1.2.3` | `1.2.3`, `1.2`, `sha-<short>` |

  `develop`, `main`, `latest` and `1.2` move. `sha-<short>` and `1.2.3`
  never change; deployments pin one of those.
- **Platforms:** `linux/amd64` and `linux/arm64`, as one multi-platform
  image.
- **Least privilege:** only the publish job gets `packages: write`; the
  workflow default stays `contents: read`. All actions in the workflow are
  pinned to commit hashes, as ADR 0005 requires once a job can write.
- **Cleanup after every publish:** the job `cleanup-database-images` runs
  `.github/scripts/cleanup-images.sh`. Of all images with a `sha-` tag, the
  newest 10 are kept; older ones are deleted if `sha-` tags are their only
  tags. Images with any other tag (`develop`, `main`, `latest`, releases)
  are never deleted.
  - A multi-platform image is one tagged index version plus untagged child
    versions (platforms, attestations). The script deletes an index with its
    children, index first, and never a child that a kept image references.
  - Other untagged versions are left alone, because they may belong to a
    publish that is still running.
  - It is our own script (gh, curl, jq) rather than a third-party action, so
    no outside code holds the delete permission. It has a `--dry-run` mode
    and can be run locally with a personal token (`delete:packages`).

## Consequences

- At most 10 `sha-` images plus the branch and release images stay in the
  registry. Rolling back further than 10 builds means using a release or
  rebuilding from the commit.
- Cleanup runs one at a time (concurrency group) so that two pushes don't
  delete the same versions.
- If a delete fails halfway, an index may be gone while some of its children
  remain as untagged versions. They are harmless and can be deleted by hand.
- A release is a git tag `v*` pushed to the commit to release. Release notes
  and a changelog are not decided yet.
- The image built for publishing is not byte-identical to the one the image
  checks tested (different builder, two platforms). Both come from the same
  commit and Dockerfile.
- The arm64 build is cross-built. The Dockerfile only copies files, so no
  emulation runs today; QEMU is set up for when it gets `RUN` steps.
- Pinned actions don't update themselves. Update the hashes deliberately
  (Dependabot can do this).
- The first push creates the package. Check its visibility in the package
  settings on GitHub: it should be public like the repository, or cluster
  pulls need credentials.

## Left for later ADRs

Helm chart and its publishing, the backend and frontend images, release
notes.
