# Working agreement: versions, branches, updates

## Versioning (SemVer)
- Single source of truth: `plinth/Cargo.toml` `[workspace.package] version`. Docs and the changelog follow it.
- `0.y.z` until the public MVP: bump **minor** when a roadmap phase milestone completes (e.g. Spike C done, Alpha), **patch** for fixes and doc-only releases.
- Manifest format (`format` field in `.plinth` bundles, see `design/architecture.md`) has its own integer version and is not tied to the app version.
- Tags: `vX.Y.Z` on `main` only, annotated (`git tag -a v0.1.0 -m "..."`).

## Branches
- `main`: always builds and passes `cargo test` (enforced by CI). Never commit work in progress directly to it.
- Feature branches: `spike/<name>`, `feat/<name>`, `docs/<name>`, `fix/<name>`. Merge to `main` by pull request (self-review is fine) with squash or merge commit.

## Commits
Conventional style, imperative mood: `feat: ...`, `fix: ...`, `docs: ...`, `test: ...`, `chore: ...`, `spike: ...` (one logical change per commit). Include a short body for decisions.

## Every change that matters updates, in the same PR
1. `CHANGELOG.md` under `[Unreleased]` (Added / Changed / Fixed / Decisions / Known open items).
2. The relevant design doc if a decision changed (`design/*.md`), and `plinth/ROADMAP.md` checkboxes.
3. Test or spike results go in the spike's `RESULTS.md` with date, machine, build number, and method; mark anything not run as NOT TESTED.

## Releases ("updates")
1. Move `[Unreleased]` entries into a new `## [X.Y.Z] - YYYY-MM-DD` section.
2. Bump the workspace version in `plinth/Cargo.toml`.
3. Commit `chore: release vX.Y.Z`, tag, push with tags.
4. Product update channel (installer/auto-update) is not built yet; see `plinth/ROADMAP.md`.

## Evidence rules (carried from the research)
Facts need a source or a measurement; judgment is labelled INFERENCE; unverified items are labelled UNVERIFIED or NOT TESTED. No code is copied from AGPL/GPL projects (Seelen UI, Lively) into this repo.
