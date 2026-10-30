# host-demo results: plinth-host end to end in WebView2

Date: 2026-10-30. Windows 11 Home build 26300, WebView2 156.0.4314.50, release build. Command: `cargo run --release -p host-demo -- --secs 14` (exit code 0 = all checks pass). Optional: `PLINTH_TRACE=1` (print served requests), `PLINTH_ONLY=<text>` (run only hostile attempts whose name contains the text), `PLINTH_PROXY=1` (forced-proxy experiment).

## Setup
`fixtures/widgets/clock-cpu` (declares system.cpu and storage; the demo grants only system.cpu) and `fixtures/widgets/hostile` (declares system.cpu and net.fetch; nothing granted). Each is served from its own origin (`http://pw0.localhost`, `pw1`) by `plinth-host`, with the CSP generated from its manifest and grants; widget messages go through the broker and are logged; a local TCP server records anything that leaves a widget.

## Result: 11 of 11 checks passed, in 4 of 4 direct runs (OBSERVED)
- clock-cpu: granted CPU read allowed and logged; storage (declared, not granted) denied; direct fetch blocked by CSP; ran to completion.
- hostile: CPU read (declared, not granted) denied; RAM read (not declared) denied; storage denied; unknown method `exec` rejected; fetch, XHR, image beacon, WebSocket, remote script import all blocked; parent DOM and top-window navigation blocked; an impersonation message claiming another slot was answered as the sender (denied), because identity comes from the iframe, not the message.
- Permission Center log shows every call with allowed/BLOCKED, as designed.

## Bugs found and fixed while building
1. wry reports custom-protocol requests as `<scheme>://localhost/...`, not `http://pw0.localhost/...`; matching on the host header made every widget request 404. The slot is now taken from the protocol name the request arrived on.
2. The first version of the demo printed 4 of 11 passing: the checks correctly failed because no widget had run. (A check suite that can fail was worth having.)

## KNOWN GAP (OBSERVED, 4 of 4 runs)
The attack "navigate self to attacker URL" opens a **bare TCP connection** to the destination (no HTTP request). The host `frame-src` allowlist and header CSP do not prevent it. See the correction in `spikes/sandbox-probe/RESULTS.md`.

## Mitigation experiment: forced local proxy (OBSERVED, 4 of 4 runs)
With `PLINTH_PROXY=1` (all WebView2 traffic forced to a local listener, loopback not bypassed): bare connections to the destination **0**, HTTP requests to the destination **0**, 11 of 11 checks still pass. The proxy saw only the browser's own `CONNECT nav-edge.smartscreen.microsoft.com:443` (two lines) and empty connections to itself. Consequences for the design: (1) Plinth needs a built-in local proxy that denies by default and allows only declared and granted hosts per widget, which also gives an audit log and the data for the Impact Meter; (2) the proxy must allow-list WebView2's own service traffic (SmartScreen) or the user will see failures; (3) per-widget attribution at the proxy is not solved (proxy requests do not say which iframe sent them; candidates: per-origin `Referer`/`Origin` for HTTP, proxy credentials per origin, or a separate WebView2 profile per widget at a high memory cost). NOT TESTED: HTTPS interception needs, WebSocket through the proxy, proxy performance, behavior when the proxy is down.

## Not tested
Real telemetry provider (CPU is a mock value), multiple monitors, desktop-layer attach of the host (demo uses a normal window), memory/GPU exhaustion, safe-mode toggling at runtime, grant changes while widgets run (CSP changes need a reload), upgrade flow, more than 2 widgets.

---

# Spike D: real deny-by-default proxy (`plinth-proxy`), PLINTH_PROXY=1

Date: 2026-10-30, same machine and build. `plinth-proxy` (std only): accepts only `CONNECT host:port`; allows a tunnel only if the destination is in the allow-list built from the broker's granted `net.fetch` hosts (port 443), the proxy is not in Safe mode, and the resolved address is not loopback/private/link-local (SSRF and DNS-rebinding guard); denies plain HTTP, other methods, malformed and oversized requests; caps concurrent connections; logs every attempt. WebView2 is started with `--proxy-server=http://127.0.0.1:<port> --proxy-bypass-list=<-loopback>`.

## Unit and socket tests: 8 passed (OBSERVED)
Strict target parsing (userinfo tricks, paths, IPv6 brackets, port 0, double dots, trailing dot), exact host/port matching with no implicit subdomains, safe mode, private-address classifier (including IPv4-mapped IPv6), denial of plain HTTP/POST/garbage/9 KB headers, a real loopback tunnel echo test through an allow-listed destination (and denial of a neighbouring port), refusal of an allow-listed name that resolves to loopback, grant changes applying to the next request. Plus `Host::allowed_destinations()` (union of granted hosts, empty in safe mode) with a test.

## End to end with the hostile widget: 4 of 4 runs, exit code 0 (OBSERVED)
- 11 of 11 widget checks pass (functionality unchanged by the proxy).
- Direct connections to the destination server: **0 HTTP requests and 0 bare connections** (in direct mode the same attack produced 1 bare connection in 4 of 4 runs).
- Proxy log (5 entries, all denied, 0 allowed): two `CONNECT nav-edge.smartscreen.microsoft.com:443` (the browser's own SmartScreen check, denied) and three empty speculative connections to the proxy itself. The attacker's address never appears at the proxy: the navigation is blocked in the browser by the host `frame-src`, after Chromium's preconnect goes to the proxy rather than to the attacker.

## What is NOT proven
- The allow path with real HTTPS from a widget (needs a TLS endpoint; only the proxy's loopback echo tunnel was tested).
- Per-widget attribution: the proxy enforces the union of all widgets' granted hosts; widget A could reach widget B's host if the CSP did not stop it. CSP `connect-src` (generated per widget) is the per-widget layer; a failure of CSP would expose the union.
- WebSocket (`wss`) through CONNECT should work the same way but was not tested.
- Denying SmartScreen: widgets cannot navigate anyway and the host page is local, so no user-visible failure was seen; whether any WebView2 feature (certificate revocation, component updates) needs an allow-list is UNVERIFIED. Real DNS behavior with a remote attacker name was not observed (loopback only).
- Proxy performance and failure mode (proxy down means no network for widgets, which is the safe failure).
