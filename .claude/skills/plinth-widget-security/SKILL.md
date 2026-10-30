---
name: plinth-widget-security
description: Security review checklist and test harness for Plinth widgets, the widget host, and the sandbox. Use when reviewing or changing plinth-host, plinth-core's broker, CSP generation, custom-protocol delivery, widget manifests or setup cards, or when asked whether a widget or package is safe.
---

# Plinth widget security review

Threat model: a third-party widget is hostile until proven otherwise. It may try to read files, launch programs, exfiltrate data, hang the UI, or gain permissions through updates.

## Invariants (each has a test; keep them green)
1. Deny by default: a permission must be declared in the manifest AND granted by the user (`plinth-core` broker tests).
2. No filesystem, process-launch, or native-code permission exists in the vocabulary (`manifest.rs` rejects unknown ids).
3. `net.fetch` needs exact declared hosts; no wildcards, schemes, or implicit subdomains; the CSP `connect-src` is generated from current grants.
4. Updates never inherit newly declared permissions; dropped permissions lose grants.
5. Packages are served from memory only; `manifest.json` and dotfiles are not servable; paths with `..`, backslashes, colons, or NUL are rejected (`package.rs` tests).
6. Third-party widgets each get their own origin (`pw<N>.localhost`) with a header CSP and a host `frame-src` allowlist (Spike C: 0 leaked requests, hog contained).
7. IPC is untrusted: identity comes from the host page's iframe bookkeeping, never from the message; unknown methods and bad JSON return errors.
8. Safe mode denies every bridge call and serves 403 for third-party widgets.
9. Setup cards contain no code; ids, versions, rects and accents are validated; unknown widgets are listed as "missing" and shown to the user before install.

## Review checklist for a change
- Does it add a new bridge method? Add a permission or prove it needs none, add quotas, add tests for denied/undeclared/safe-mode/bad-input.
- Does it touch CSP or headers? Re-run the sandbox probe in protocol mode and confirm 0 server hits and no host starvation.
- Does it add a way to load remote code, eval, or inline script? Reject unless there is a written exception.
- Does it log? Logs must not contain widget data beyond truncated notes.

## Run the harness
```bash
cd plinth
cargo test -p plinth-core -p plinth-host
cargo run --release -p sandbox-probe -- --delivery protocol    # expects: server hits (none), host timer gap ~60 ms
cargo run --release -p sandbox-probe -- --hostcsp on            # legacy srcdoc mode, for comparison
```
Interpret: any request listed under "reached the local server" in protocol mode, or a host timer gap near 3000 ms, is a regression.

## Known gaps (do not claim safety beyond these)
Memory and GPU exhaustion, many origins at once, side channels (Spectre-class), WebView2 or Windows versions other than the tested build, clipboard/camera/download prompts, supply-chain review of store packages.
