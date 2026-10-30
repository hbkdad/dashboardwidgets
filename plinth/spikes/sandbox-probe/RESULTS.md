# Spike C results: hostile widgets vs sandboxed iframes in WebView2

Date: 2026-10-30. Windows 11 Home build 26300, WebView2 runtime 156.0.4314.50, wry 0.57.1, tao 0.37.2, release build. Method: a trusted host page runs hostile "widgets" as `<iframe sandbox="allow-scripts" srcdoc=...>`; each tries to escape or exfiltrate. A local TCP server on 127.0.0.1 records every request that actually arrives, so "reached the network" is proven server-side. Every request URL carries the widget variant id. No external hosts were contacted. Run: `cargo run --release -p sandbox-probe -- --hostcsp on|off`.

Variants: **V1** `sandbox="allow-scripts"` only. **V2** same plus a strict CSP meta (`default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'`) injected as the first element of the widget document. **V3** iframe navigates itself to an attacker URL (with the V2 CSP). **V4/V5** busy loop for 3 s.

## 1. Network exfiltration (server-side evidence)
| Channel | V1 sandbox only | V2 sandbox + strict CSP |
|---|---|---|
| fetch (response body readable) | **REACHED SERVER, body read** | blocked, no request |
| XMLHttpRequest | **REACHED** | blocked |
| `sendBeacon` | **REACHED** (POST) | returned true, **no request arrived** |
| `<img>` request | **REACHED** | blocked |
| WebSocket | **REACHED** (upgrade request seen) | blocked |
| dynamic `import()` of remote script | **REACHED, module loaded** | blocked |
| `<script src>` remote | **REACHED, executed** (`onload` fired) | blocked |
| Form POST | blocked by sandbox (no request) | blocked |
| Self-navigation exfil (`location.href = attacker URL`) | n/a | **REACHED SERVER with data in the query string, host CSP off** |
| Self-navigation, host page CSP `frame-src about:` | n/a | **blocked, no request** |
All OBSERVED, one run each for the table, repeated runs consistent for the V1/V2 hit lists.

**Conclusions:**
1. A sandbox alone does **not** stop network access: V1 could send and read responses and load and run remote code. The design's "network only to declared hosts" requires enforcement we have not built (a CSP, a custom protocol, or a proxy).
2. A strict CSP injected into the widget document blocked every direct channel, but **navigation-based exfiltration still worked** under it. The fix that worked here is a CSP on the **host** page (`frame-src about:`), which stopped self-navigation. Both layers are needed.
3. CSP via an in-document meta tag is only as safe as who writes the document: the host must generate the first elements. A real design should serve widget content from a custom protocol with CSP headers set by the host.
4. `net.fetch` with declared hosts cannot be a CSP `connect-src` allowlist for arbitrary user-approved hosts without regenerating the CSP per widget; that is feasible (host builds it from the approved manifest) but untested. Proxying all widget requests through the core is the other option, also untested.

## 2. Environment / bridge exposure
- `window.ipc` and `chrome.webview` exist inside the sandboxed iframe (`typeof` = object). The widget called `window.ipc.postMessage(...)`; **no such message reached the Rust handler in any run (0 of 6)**. So the direct bridge from an iframe was not delivered in this setup. OBSERVED, but the exposure of the objects is still a smell, and behavior may differ with other wry/WebView2 versions. Treat every IPC message as untrusted regardless and validate sender frame/origin.
- Widget origin is `"null"` (opaque), as intended.

## 3. Storage and cross-frame access (V1 and V2 identical)
Blocked with SecurityError: `localStorage`, `cookie`, `indexedDB`, `parent.document`, `parent.localStorage`, reading `top.location`, assigning `top.location` (top navigation), and `window.open` returned null. `fetch('file:///C:/Windows/win.ini')` failed. So per-widget storage isolation and no host-DOM access hold with `sandbox="allow-scripts"` and no `allow-same-origin`. Because of this, widget persistent storage must be provided by the host API (`plinth.storage`), as designed.

## 4. Resource exhaustion (important)
A 3 s busy loop in one sandboxed iframe **froze the host page's timers and a second widget's timers for ~3 s**: host timer max gap 3004-3007 ms (expected 50) and the victim widget's gap 3002-3004 ms, in **7 of 8 runs** (one early run showed a 64 ms gap, i.e. no starvation; I cannot explain the difference). There is no per-widget CPU watchdog. A single widget can hang the whole widget layer and the hub for as long as it loops. Memory exhaustion was NOT tested.
- Spike B showed process counts do not grow with iframes; this result matches: sandboxed iframes are not process-isolated from each other here.

## 5. What this means for the design (changes made)
1. **Process isolation is required for untrusted widgets**, or at least a watchdog that can kill and restart a hung widget. Because the host UI shares fate with widgets in this setup, the Permission Center and Safe mode must live in a **separate window/process** from the widget host. Candidate fix to test next: serve each widget from its own origin (custom protocol host name) to get site isolation into separate renderer processes (UNVERIFIED; measure RAM cost per widget).
2. **Enforce network policy in the platform, not the widget.** Strict CSP (generated by the host from the approved manifest) plus a host CSP to stop navigation exfil, or route all widget network calls through the core.
3. Do not allow remote script loading (`script-src` limited); bundle code inside the signed package.
4. Treat all IPC as untrusted; verify sender.
5. Add the Spike C fixtures as a CI regression suite (the probe can become a test harness).

## Not tested
Memory bombs, WebGL/GPU abuse, clipboard/camera/geolocation prompts (skipped to avoid user prompts), downloads, `postMessage` spoofing between widgets, service workers, WebAssembly, CSS-based exfil, DNS prefetch, speculative `<link rel=prefetch>`, side channels (Spectre), other WebView2 or Windows versions, custom-protocol delivery, per-widget origins.

## Verdict
Spike C: **the sandbox attribute alone is insufficient; sandbox + generated CSP + host `frame-src` held against every network channel tried, and storage/DOM isolation held. Availability (CPU starvation) is NOT contained.** Exit criterion "0 escapes" met only for the combined configuration and only for the listed attacks; the starvation finding means process isolation or a watchdog is a requirement, not an option.

---

# Update: per-widget origin via custom protocol (`--delivery protocol`)

Branch `spike/origin-isolation`. Widgets are served by the host through wry custom protocols (`http://pwa.localhost`, `pwb`, `pwc`), **one origin per widget**, with the CSP delivered as an **HTTP response header** (`default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'`) that the widget cannot modify. Iframes use `sandbox="allow-scripts allow-same-origin"` (real origin, no opaque origin). The host page CSP is `frame-src http://pwa.localhost http://pwb.localhost http://pwc.localhost`. Run: `cargo run --release -p sandbox-probe -- --delivery protocol`.

## Results (OBSERVED)
| Test | Result |
|---|---|
| Attack suite (P1) from origin pwa | **0 requests reached the local server.** fetch, XHR, image, WebSocket, remote `import()`, remote `<script>`, file fetch all blocked; `sendBeacon` returned true but no request arrived; form POST made no request |
| Self-navigation exfil (P2) | **No request arrived** (host `frame-src` allowlist) |
| Starvation: hog (busy loop 3 s) on pwa vs victim on pwb, bystander on pwc, and host | **Not starved in 5 of 5 runs**: host timer max gap 63-64 ms, victim/bystander 63-64 ms (expected ~50). Compare srcdoc delivery: 3000+ ms in 7 of 8 runs |
| Direct `window.ipc` call from the iframe | 0 messages reached Rust (as before); objects still exposed |
| Cookie read | allowed (empty) under a real origin; `localStorage` still raised SecurityError here; storage must still be provided by the host API |

## Cost (OBSERVED, single runs; includes a PowerShell measurement process in the tree, so absolute numbers are inflated by a constant, deltas are meaningful)
| State | Processes | Private |
|---|---|---|
| Host page only | 9 | 222 MB |
| + 2 widgets on 2 origins | 11 | 280 MB |
So **about +1 process and +29 MB private per extra origin** (compare about 2.5 MB per widget as srcdoc iframes in Spike B). Ten isolated widgets would be roughly +290 MB, which breaks the ~200 MB budget.

## Conclusions
1. Origin-per-widget delivery from a host-controlled custom protocol with a header CSP and an allowlisted host `frame-src` **closed every network channel tested and contained the CPU hog** in this setup. This is the recommended delivery for **third-party** widgets.
2. It costs about 29 MB and one process per widget, so use a **hybrid**: first-party/bundled widgets share one trusted origin (cheap); third-party widgets get their own origin; cap concurrent third-party widgets (a soft limit around 5-8 per monitor) and show the memory cost in the Permission Center.
3. The Permission Center and Safe mode must still live outside the widget host (separate window/process); the host-page timer was unaffected here, but only five runs and one attack pattern were tested.
4. Remaining unknowns: why the srcdoc/opaque-origin frames shared a renderer while origin-keyed frames did not (assumed site isolation; UNVERIFIED), memory bombs, GPU abuse, many origins at scale, whether Chromium merges processes under memory pressure, custom-protocol behavior on other WebView2 versions.
