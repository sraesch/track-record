---
name: release
description: Cut a new release of track-record following the repo's develop/main branching and Keep-a-Changelog conventions. Use when the user asks to "release", "cut a release", "ship a new version", "bump the version", "publish a release", or "tag a release". Syncs develop/main, bumps the version in the backend crates, the frontend package and the OpenAPI documents, finalizes CHANGELOG.md and frontend/APP_CHANGELOG.md, merges develop into main, tags it, waits for CI to publish the versioned images to GHCR, and publishes a GitHub Release with the changelog section as notes.
---

# Cut a track-record release

track-record ships releases by merging `develop` into `main` and tagging the merge with `vX.Y.Z`.
There is no separate release workflow: the **CI** workflow (`.github/workflows/ci.yml`) runs on
pushes to `main` and on `v*` tags and, after the checks pass, publishes the three images
`ghcr.io/sraesch/track-record-db`, `-server` and `-frontend` for amd64 and arm64 through
`.github/workflows/publish-image.yml` ([ADR 0007](../../../docs/adr/0007-image-publishing.md),
[ADR 0014](../../../docs/adr/0014-server-and-frontend-images.md)):

| Push | Image tags |
|---|---|
| `main` | `main`, `latest`, `sha-<short>` |
| tag `vX.Y.Z` | `X.Y.Z`, `X.Y`, `sha-<short>` |

The GitHub Release is created by hand in this workflow (step 9), with the matching `CHANGELOG.md`
section as notes. The process is decided in
[ADR 0020](../../../docs/adr/0020-release-process.md); change both together.

Follow the steps in order. This touches `main`, pushes a tag and publishes images, so
**confirm the version and pause before pushing** (step 7) **and before publishing the GitHub
Release** (step 9). Don't push or publish without the user's go-ahead.

## Conventions

- The version lives in **several files** that must agree (there is no check script; use the
  commands in step 4):
  - `backend/crates/core/Cargo.toml` and `backend/crates/server/Cargo.toml` → `[package] version`
    (plus the matching `track-record-core` / `track-record-server` entries in `backend/Cargo.lock`)
  - `frontend/package.json` → `"version"` (plus both `track-record-frontend` entries at the top of
    `frontend/package-lock.json`)
  - `backend/openapi/machine.json` and `backend/openapi/ui.json` → `info.version`. utoipa takes it
    from the server crate's version, and the MCP server reports it too. The documents are
    regenerated, never edited by hand; `committed_openapi_documents_are_current` in
    `backend/crates/server/tests/invariants.rs` fails if they are stale.
- SemVer, currently `0.x`.
- Commit messages follow the repo style (imperative, no emoji, no prefix):
  - prepare commit on `develop`: `Prepare X.Y.Z release`
  - merge commit on `main`: `Release X.Y.Z (merge develop)` (a `--no-ff` merge)
- The tag is **annotated**, named `vX.Y.Z`, message `Release X.Y.Z`, on the merge commit. Tags with
  a `-` (e.g. `v0.2.0-beta.1`) are pre-releases: the image gets only the full version tag, and the
  GitHub Release is marked as a pre-release.
- `CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/): the
  `## [Unreleased]` section rolls into `## [X.Y.Z] - YYYY-MM-DD`, and link definitions live at the
  bottom of the file. Step 9 extracts the section for the release notes, so keep the heading
  format exact. Entries cite requirement IDs (`FR-n`, `NFR-n`), ADRs and PRs; keep them.
- `frontend/APP_CHANGELOG.md` is the **user-facing changelog** shown on the web UI's About page
  (`/about`, imported raw by `frontend/src/about/Changelog.tsx` and rendered as Markdown without
  its HTML). Unlike `CHANGELOG.md` it is maintained with every user-visible change, under a
  `## Unreleased` heading at the top, grouped by area with `###` headings (`Runs`,
  `Definitions`, ...). At release time that heading becomes the version (step 4).

## Workflow

### 1. Sync develop and main

```bash
git fetch origin --prune --tags
git checkout develop && git pull --ff-only origin develop
git ls-remote --heads origin main
```

If `main` exists on the remote, update it too:

```bash
git checkout main 2>/dev/null || git checkout -b main origin/main
git pull --ff-only origin main
git checkout develop
```

**First release:** if there is no `main` yet, it is created from `develop` in step 5.

Confirm the working tree is clean (untracked local files like `.claude/` aside) and that everything
meant for this release is merged into `develop`. If a PR is still meant to be in the release, stop
and say so.

### 2. Decide the new version

Find the previous release: `git describe --tags --abbrev=0 --match 'v[0-9]*' main`. Let `PREV` be
that version without the `v`.

**First release:** if there is no `v*` tag yet, there is no `PREV`. Propose `0.1.0` (the backend
crates already carry it; `frontend/package.json` is still `0.0.0` and gets bumped).

Otherwise read the `## [Unreleased]` section of `CHANGELOG.md`, propose a SemVer bump, and
**confirm with the user** (don't pick silently):

- Breaking changes → major. Pre-1.0, a breaking change is usually still a **minor** bump; flag it
  and let the user decide. Breaking here includes incompatible changes to `/api` or `/mcp`, the
  ingestion schema, or migrations that drop or rewrite data on existing databases.
- New features (an `### Added` / `### Changed` section) → **minor**.
- Only fixes (`### Fixed`) → **patch**.

If `## [Unreleased]` is empty, there is nothing to release; stop and say so.

Let `NEXT` be the agreed version.

### 3. Pre-flight gates

Run the same checks as CI and require all to pass. They need Docker and take several minutes, so
run them in the background:

```bash
db/test.sh && db/image-test.sh && backend/test.sh && frontend/test.sh
```

Also check that CI on `develop` is green for the commit being released:
`gh run list --branch develop --workflow CI --limit 1`.

### 4. Prepare the release on `develop`

On `develop`, make one `Prepare NEXT release` commit that does all of this:

1. **Set the version to `NEXT`** in `backend/crates/core/Cargo.toml`,
   `backend/crates/server/Cargo.toml` and `frontend/package.json` (skip a file that is already at
   `NEXT`). Then update the lockfiles, regenerate the OpenAPI documents and the frontend API types:

   ```bash
   (cd backend && cargo update --workspace --offline)
   (cd backend && cargo run -p track-record-server -- openapi machine > openapi/machine.json)
   (cd backend && cargo run -p track-record-server -- openapi ui > openapi/ui.json)
   (cd frontend && npm install --package-lock-only && npm run generate:api)
   ```

   Verify that every place says `NEXT`:

   ```bash
   grep -H '^version' backend/crates/*/Cargo.toml
   grep -A1 '^name = "track-record-' backend/Cargo.lock
   (cd frontend && node -p "[require('./package.json').version, require('./package-lock.json').version, require('./package-lock.json').packages[''].version].join(' ')")
   (cd backend/openapi && node -p "['machine','ui'].map(f => f + ': ' + require('./' + f + '.json').info.version).join(', ')")
   ```

   Only the version lines should change: the `track-record-*` entries in the lockfiles and
   `info.version` in the OpenAPI documents. `src/api/schema.d.ts` should not change at all. If
   other dependencies or paths moved, revert that and investigate.

2. **Finalize `CHANGELOG.md`:**
   - Rename `## [Unreleased]` to `## [NEXT] - YYYY-MM-DD` (today's date).
   - Insert a fresh, empty `## [Unreleased]` above it.
   - Update the link definitions at the bottom:
     - `[Unreleased]: https://github.com/sraesch/track-record/compare/vNEXT...HEAD`
     - add `[NEXT]: https://github.com/sraesch/track-record/compare/vPREV...vNEXT` above the
       `[PREV]` line, or on the first release
       `[NEXT]: https://github.com/sraesch/track-record/releases/tag/vNEXT`
   - Tidy the entries if needed (merge duplicates, user-facing wording), but don't drop anything
     without asking.
   - **First release:** the section still says "Nothing has been released yet. This is everything
     on `develop` so far." — remove that line. The `### Changed` intro ("These changes happened
     before any release …") and the preamble's "releases will use Semantic Versioning" need
     rewording too; propose wording and ask.

3. **Finalize `frontend/APP_CHANGELOG.md`** (the in-app changelog):
   - Rename `## Unreleased` to `## NEXT — YYYY-MM-DD`. Don't add an empty `## Unreleased` above
     it: the About page would show an empty section. The next user-visible change adds it again.
   - Check the section against the `CHANGELOG.md` section you just finalized: everything a user of
     the web UI would notice is there, and nothing else. Follow the file's style:
     - **User language**: what changed for the user, not how it was implemented ("Faster run
       list", not "Cursor paging").
     - **Self-contained**: no links, no references to files, issues, PRs, ADRs, requirement IDs
       or commits. The text must stand on its own inside the app.
     - Leave out internal changes (CI, images, tests, tooling, refactors, migrations) unless the
       user can see or feel them.
     - Older sections stay as they are.
   - If there is no `## Unreleased` section but the release has user-visible changes, write a
     `## NEXT — YYYY-MM-DD` section at the top distilled from `CHANGELOG.md`.
   - If a test hard-codes the `Unreleased` heading (`frontend/src/about/AboutPage.test.tsx` did
     before the first release), change it so it doesn't depend on the version heading, e.g. any
     level-2 heading.

   Show the finalized `CHANGELOG.md` section and the APP_CHANGELOG section to the user together
   with the version confirmation.

Then re-run the checks that cover these files and commit:

```bash
backend/test.sh -- committed_openapi_documents_are_current
frontend/test.sh -- about
```

```bash
git add backend/crates/core/Cargo.toml backend/crates/server/Cargo.toml backend/Cargo.lock \
  backend/openapi/machine.json backend/openapi/ui.json \
  frontend/package.json frontend/package-lock.json frontend/APP_CHANGELOG.md \
  CHANGELOG.md
git commit -m "Prepare NEXT release"
```

Add the About page test too if you changed it. Don't push yet; everything is pushed together in
step 7.

### 5. Merge develop into main

`main` only ever gets `--no-ff` release merges:

```bash
git checkout main
git merge --no-ff develop -m "Release NEXT (merge develop)"
```

**First release:** there is no `main` to merge into. Create it at the prepare commit instead
(`git checkout -b main develop`); the tag in step 6 goes on that commit.

If the merge conflicts, `main` has diverged unexpectedly; stop and investigate rather than forcing
it.

### 6. Tag the merge

Annotated tag on the merge commit:

```bash
git tag -a vNEXT -m "Release NEXT"
```

### 7. Push (confirm first)

Show the user what will be pushed (`git log --oneline origin/main..main`, or on the first release
`git log --oneline -5 main`, and the tag) and get their go-ahead, then:

```bash
git push origin develop
git push origin main
git push origin vNEXT
```

Each push starts a CI run: `develop` publishes the `develop` images, `main` the `main` and `latest`
images, and the tag the `NEXT` and `X.Y` images. On the first release, the `main` push creates the
branch on GitHub.

### 8. Wait for CI and the images

Watch the CI runs for `main` and the tag until they finish:

```bash
gh run list --workflow CI --event push --limit 5
gh run watch <run-id> --exit-status
```

They take about 15 minutes plus publishing (the server image compiles Rust for both platforms). If
a job fails, report which one and why (`gh run view <run-id> --log-failed`); a flaky job can be
retried with `gh run rerun <run-id> --failed`. Don't publish the GitHub Release while an image is
missing unless the user says so.

Then check that every image has the version tag for both platforms:

```bash
for image in track-record-db track-record-server track-record-frontend; do
  docker buildx imagetools inspect "ghcr.io/sraesch/$image:NEXT" | grep -E '^Name|Platform'
done
```

If a package is new (first release of an image), remind the user to check that its visibility in
the GitHub package settings is public, like the repository.

### 9. Publish the GitHub Release (confirm first)

Extract the `CHANGELOG.md` section as release notes. Relative links (ADRs, `docs/`, `frontend/`)
don't resolve in a release body, so point them at the tag:

```bash
notes="$(mktemp)"
awk -v v="NEXT" '
  index($0, "## [" v "]") == 1 { found = 1; next }
  found && (/^## \[/ || /^\[[^]]+\]: /) { exit }
  found
' CHANGELOG.md \
  | perl -pe 's{\]\((?!https?://|#)([^)]+)\)}{](https://github.com/sraesch/track-record/blob/vNEXT/$1)}g' \
  > "$notes"
```

Create a draft and show it to the user (`gh release view vNEXT --web` or the URL it prints):

```bash
gh release create vNEXT --verify-tag --draft --title "NEXT" --notes-file "$notes"
```

Add `--prerelease` for a tag with a `-`. With the user's go-ahead, publish it:

```bash
gh release edit vNEXT --draft=false --latest
```

For a pre-release, leave out `--latest`.

### 10. Wrap up

- Return to `develop`: `git checkout develop`.
- Confirm CI on `main` and on the tag is green.
- Report to the user: the version, the release URL, the image tags published
  (`ghcr.io/sraesch/track-record-{db,server,frontend}:NEXT`), and the CI status.

## Done when

- [ ] The backend crates, `backend/Cargo.lock`, `frontend/package.json`, `frontend/package-lock.json`
      and both OpenAPI documents are at `NEXT`.
- [ ] `CHANGELOG.md` has a dated `## [NEXT]` section, a fresh empty `## [Unreleased]`, and updated
      links.
- [ ] `frontend/APP_CHANGELOG.md` has a self-contained `## NEXT — YYYY-MM-DD` section at the top and
      no empty `## Unreleased`.
- [ ] `develop` is pushed with the prepare commit; `main` has the `--no-ff` release merge (or, on the
      first release, was created at the prepare commit).
- [ ] Annotated tag `vNEXT` is pushed.
- [ ] CI succeeded for `main` and the tag, and all three images exist as `NEXT` for amd64 and arm64.
- [ ] The GitHub Release `vNEXT` is published with the changelog notes.
