# ADR 0020: Release process

**Status:** proposed · **Date:** 2026-11-23

## Context

[ADR 0007](0007-image-publishing.md) decided that pushes to `main` and `v*`
tags publish images, but left open what a release is beyond "a git tag `v*`",
and how release notes are written. Since then the repository has two
changelogs: `CHANGELOG.md` (Keep a Changelog, for operators and developers,
with requirement IDs, ADRs and PRs) and `frontend/APP_CHANGELOG.md` (short,
for the people who use the web UI, shown on the About page). Nothing has been
released yet: there is no `main` branch and no tag, and the versions disagree
(backend crates `0.1.0`, frontend `0.0.0`).

## Decision

- **Branches:** work lands on `develop` through pull requests. `main` only
  gets `--no-ff` merges of `develop`, one per release, with the message
  `Release X.Y.Z (merge develop)`. The first release creates `main` at the
  prepare commit.
- **One version for the whole system,** SemVer, `0.x` for now. It is set in
  the backend crates (`track-record-core`, `track-record-server`), the
  frontend package, the Helm chart (`version` and `appVersion`,
  [ADR 0021](0021-helm-chart.md)) and, generated from the server crate,
  the OpenAPI documents and the MCP server info. The database image has no version of
  its own; its schema version is the list of applied migrations.
- **Prepare commit** `Prepare X.Y.Z release` on `develop`: sets the version
  everywhere, rolls `## [Unreleased]` of `CHANGELOG.md` into
  `## [X.Y.Z] - YYYY-MM-DD` with a fresh empty `## [Unreleased]`, and
  renames `## Unreleased` in `APP_CHANGELOG.md` to `## X.Y.Z — YYYY-MM-DD`
  (without a new empty section, which the About page would show).
- **Tag:** annotated `vX.Y.Z` with the message `Release X.Y.Z`, on the merge
  commit. A tag with a `-` is a pre-release.
- **Images:** no extra workflow. The CI run for the tag publishes `X.Y.Z` and
  `X.Y` of all three images, the run for `main` publishes `main` and
  `latest` (ADR 0007).
- **Helm chart:** the same CI run for the tag publishes the chart as
  `oci://ghcr.io/<owner>/charts/track-record`, version `X.Y.Z`, once the
  three images are published (ADR 0021).
- **Release notes:** a GitHub Release for the tag, created once CI has
  published the images. Its notes are the `CHANGELOG.md` section of the
  version, with relative links pointed at the tag.
- **Who does it:** the steps are written down as the Claude Code skill
  `.claude/skills/release/SKILL.md`, which also works as a checklist by
  hand. It stops for confirmation before pushing and before publishing the
  GitHub Release.

## Consequences

- `develop` and `main` diverge by the merge commits; `main` is never merged
  back. Every release merge is a no-conflict `--no-ff` merge as long as
  nothing else is committed to `main`.
- The release is not automated end to end. A release workflow could take
  over steps 8 and 9 (wait for images, create the release) later; the skill
  is the specification for it.
- There is no script that checks that all versions agree. A forgotten bump
  of the server crate is caught by the OpenAPI test only if the documents
  were not regenerated either, and `deploy/helm/test.sh` fails if the chart
  and the server crate differ; the skill lists the checks.
- `APP_CHANGELOG.md` must keep an `## Unreleased` section while changes
  accumulate, and tests must not depend on the text of its version
  headings.

## Left for later ADRs

A release workflow, signing images. The Helm chart and its versioning are
decided in [ADR 0021](0021-helm-chart.md).
