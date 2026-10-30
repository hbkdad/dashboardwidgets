# Plinth (working name)

Desktop widgets you can trust: permission-gated, no-code to remix, no shell replacement. Design docs live one level up: `../README.md`, `../design/`, `../research/`.

## Status
Pre-alpha scaffold. **Only `crates/plinth-core` is built and tested** (12 unit tests pass: manifest validation, permission broker, activity log, pause governor). `hub/` is an unbuilt UI skeleton. There is no window layer, no widget host and no installer yet.

## Layout
```
crates/plinth-core/   pure logic: manifest.rs, broker.rs (Permission Center rules), governor.rs
hub/                  Svelte + Tauri 2 hub skeleton (NOT built or verified)
fixtures/             hostile-widget manifests used for the sandbox test plan
```

## Run what exists
```bash
cargo test
```
Requires Rust (tested with 1.99.1). Dependencies: `serde`, `serde_json` only.

## Rules the core already enforces (tested)
- Deny by default; a permission must be declared in the manifest **and** granted by the user.
- No filesystem, process-launch or arbitrary-native-code permission exists in the vocabulary.
- `net.fetch` requires exact declared hosts; no wildcards, no implicit subdomains.
- Revocation takes effect on the next check; Safe mode overrides all grants.
- An update never inherits grants for newly declared permissions, and dropped permissions lose their grants.

## Not decided yet (see `../README.md`)
License (open-source core vs closed), payments/store backend, final name (trademark unchecked). The workspace license is `UNLICENSED` until then. No code from AGPL projects (e.g. Seelen UI) may be copied in.

## Next engineering steps
See `ROADMAP.md`.
