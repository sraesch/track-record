# ADR 0014: Publishing the server and frontend images

**Status:** proposed · **Date:** 2026-11-22

## Context

[ADR 0007](0007-image-publishing.md) publishes the database image to GHCR
for `linux/amd64` and `linux/arm64` and deletes old `sha-` images after
every publish. It left the backend and frontend images for later. CI builds
both (`backend/Dockerfile`, `frontend/Dockerfile`) as a check, but never
pushes them, so a deployment (NFR-1) has no images to pull.

Unlike the database image, both have build steps: the server compiles Rust,
the frontend runs `npm ci` and Vite. Building the arm64 image under QEMU
emulation would take far longer than the amd64 one, especially for Rust.

## Decision

- **Same publishing as the database image:** `ghcr.io/<owner>/track-record-server`
  and `ghcr.io/<owner>/track-record-frontend`, with the triggers, tags,
  platforms, permissions and cleanup of ADR 0007. Each image publishes after
  its own checks passed (`backend` for the server, `frontend` for the UI), so
  a failure in one part does not hold back the others.
- **One reusable workflow for all three images:**
  `.github/workflows/publish-image.yml` pushes an image (job `push`) and then
  runs `.github/scripts/cleanup-images.sh` for it (job `cleanup`, one at a
  time per package). `ci.yml` calls it once per image
  (`publish-database-image`, `publish-server-image`, `publish-frontend-image`).
- **No emulation: build stages run on the build platform.** Both Dockerfiles
  use `FROM --platform=$BUILDPLATFORM` for the build stage.
  - The server cross-compiles with `cargo build --target` for the target
    architecture. For another platform the stage installs Debian's cross gcc
    and libc headers and the Rust target; the only C code is `ring`
    (rustls). Building for its own platform needs neither.
  - The frontend's build output is the same for every platform, so it is
    built once on the build platform; only the nginx stage differs.
  - The runtime stages only copy files, so nothing runs for the target
    platform. A single amd64 runner builds both images, as one
    multi-platform image each.
- **Not chosen: a native arm64 runner per platform**, merging the two
  images afterwards. It needs a push by digest and a merge job per image,
  and the intermediate per-platform images would be left as untagged
  versions that the cleanup script deliberately does not touch.

## Consequences

- The cleanup works unchanged: each image is one tagged index with its
  platform and attestation children.
- The checks in `backend` and `frontend` build only the runner's platform.
  The cross-compiled arm64 server is first built when publishing, on a push
  to `develop`, `main` or a tag. Locally, `docker buildx build --platform
  linux/amd64,linux/arm64` builds both.
- A new native dependency of the server (a `-sys` crate that compiles C)
  needs its cross toolchain in `backend/Dockerfile`; the arm64 publish fails
  otherwise.
- The server's publish compiles everything twice without a cache, which takes
  several minutes. A build cache can be added if it becomes a problem.
- The first push of each image creates its package. As in ADR 0007, check
  that the package is public, or cluster pulls need credentials.

## Left for later ADRs

Helm chart and its publishing, build caching, release notes.
