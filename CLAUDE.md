# Project memory (keep this file lean; details live in skills and docs)

Plinth (codename; public name pending clearance, see design/decisions.md D2): safe, shareable desktop widgets for Windows.
Repo root = this folder. Rust workspace in `plinth/`. Docs in `research/` and `design/`.

## Resume protocol (read these first, in order)
1. `claude-progress.md`: what is done, what is next, known failures.
2. `feature_list.json`: every feature with status and the evidence that proves it. Only mark `done` with evidence.
3. `design/decisions.md`: decisions already made (do not re-litigate; change only with new evidence).
4. `CHANGELOG.md` `[Unreleased]` for work in flight.

## Rules
- Evidence labels everywhere: FACT / CLAIM / INFERENCE / UNVERIFIED / NOT TESTED. Never present a guess as a fact. Use `.claude/skills/plinth-research`.
- Run `scripts/verify.ps1` (tests + build, optional app autotest) before any commit that claims something works.
- Spikes follow `.claude/skills/plinth-spike`; anything touching widgets, CSP, bridge, proxy follows `.claude/skills/plinth-widget-security`.
- Branch per task (`spike/`, `feat/`, `docs/`), conventional commits, merge to `main` only with a release per `CONTRIBUTING.md`.
- No code copied from AGPL/GPL projects (Seelen UI, Lively). Do not publish under the name "Plinth".
- Do not run destructive desktop actions (lock, sleep, restart) without asking; the app autotest sends Win+D and shows a full-screen layer for about 30 s.

## Where things are
`plinth/crates/{plinth-core,plinth-host,plinth-proxy}` logic and tests; `plinth/apps/plinth-app` the app; `plinth/apps/host-demo` and `plinth/spikes/*` experiments with RESULTS.md; `plinth/widgets/*` first-party widgets; `design/prototype/index.html` UX prototype.
