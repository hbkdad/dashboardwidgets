// Regression check for the hub's output escaping. Run it in a browser console (or the Browser pane's JS tool) with
// plinth/apps/plinth-app/src/hub.html loaded. Expected result: {"pwned":0,"injected":0,"literal":true}.
// Covers the Wallpapers and Security views too (file names and process names are attacker-controlled text).
// Why: widget-controlled text (diag.log notes, manifest names/reasons/hosts) and proxy log targets are rendered in the
// trusted hub, which owns the permission-granting IPC. Unescaped, a hostile widget could run script there (found in review
// on 2026-10-30; the unfixed page injected the element and ran the handler).
(() => {
  window.ipc = { postMessage: () => {} };
  window.__pwned = 0;
  const evil = '<img src=x onerror="window.__pwned++"><script>window.__pwned++<\/script>"\'>';
  window.__onState({
    scene: 'graphite', applied: false, safe: false, motion: false, paused: false, firstRun: false, msg: null,
    widgets: [{ id: 'x.evil', name: evil, version: evil, slot: 0, first: false, perms: [{ k: 'net.fetch', label: evil, reason: evil, hosts: [evil], on: false }] }],
    activity: [{ seq: 1, widget: evil, action: 'note ' + evil, allowed: true }],
    destinations: [evil], proxy: [{ seq: 1, method: evil, target: evil, verdict: 'Deny' }],
    impact: { ram: 270, cpu: '2.0', hubRam: 200, motionCpu: 7 },
    wallpapers: [{ id: 'user.evil', name: evil, kind: 'video', bundled: false, author: evil }], wallpaper: 'user.evil', wallpaperCost: evil,
  });
  // Security Center: process names, paths, remote addresses and startup commands come from other programs on the PC.
  window.__onSecurity({
    firewall: [{ profile: evil, on: true }], av: [{ name: evil, summary: evil, on: true }], defender: { av: true, rtp: true, sigAgeDays: evil, tamper: true }, scOk: true,
    connections: [{ proc: evil, path: evil, pid: 1, remote: evil, class: evil, internet: true }],
    listeners: [{ proc: evil, path: evil, pid: 2, port: evil, bound: evil, exposure: evil, reachable: true }],
    startup: [{ name: evil, command: evil, path: evil, exists: false, scope: evil, source: evil, where: evil, userFolder: true }],
    plinth: { permAllowed: evil, permBlocked: evil, proxyAllowed: evil, proxyBlocked: evil, safe: false }, tookMs: 1000,
  });
  document.querySelector('#wl .wi > button').click();
  const n = (s) => document.querySelectorAll(s).length;
  return JSON.stringify({
    pwned: window.__pwned,
    injected: n('#wl img') + n('#log img') + n('#dest img') + n('#plog img') + n('#wl script') + n('#log script') + n('#dest script') + n('#plog script') + n('#wgrid img') + n('#wgrid script') + n('#sgrid img') + n('#sgrid script') + n('#sgrid svg'),
    literal: document.querySelector('#wl .wi b').textContent.startsWith('<img'),
  });
})();
