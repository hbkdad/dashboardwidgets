// Regression check for the hub's output escaping. Run it in a browser console (or the Browser pane's JS tool) with
// plinth/apps/plinth-app/src/hub.html loaded. Expected result: {"pwned":0,"extraScripts":0,"onerror":0,"literal":true,...}.
// Covers every view that shows text another party controls: widget names and notes, wallpaper file names, library file and folder
// names, monitor names (a display reports its own name), process names, startup commands, profile names.
// Why: the hub owns the permission-granting IPC; unescaped text there would let a hostile widget, file name or display name run
// script with that power (found in review on 2026-10-30; the unfixed page injected the element and ran the handler).
(() => {
  window.ipc = { postMessage: () => {} };
  window.__pwned = 0;
  const evil = '<img src=x onerror="window.__pwned++"><script>window.__pwned++<\/script>"\'>';
  const mid = 'M"><img src=x onerror="window.__pwned++">';
  window.__onState({
    scene: 'graphite', applied: false, safe: false, motion: false, paused: false, firstRun: false, msg: null,
    widgets: [{ id: 'x.evil', name: evil, version: evil, slot: 0, first: false, perms: [{ k: 'net.fetch', label: evil, reason: evil, hosts: [evil], on: false }] }],
    activity: [{ seq: 1, widget: evil, action: 'note ' + evil, allowed: true }],
    destinations: [evil], proxy: [{ seq: 1, method: evil, target: evil, verdict: 'Deny' }],
    impact: { ram: 270, cpu: '2.0', hubRam: 200, motionCpu: 7 },
    wallpapers: [{ id: 'user.evil', name: evil, kind: 'video', bundled: false, author: evil }], wallpaper: 'user.evil', wallpaperCost: evil,
    monitors: [{ id: mid, n: 1, name: evil, x: 0, y: 0, w: 1920, h: 1080, dpi: 96, scale: 100, primary: true, hz: 60, portrait: false, wallpaper: 'user.evil', layer: true }],
    placements: [{ widget: 'x.evil', name: evil, home: mid, homeConnected: true, monitor: mid, parked: true, clipped: true, anchor: 'top-left' }],
    fallback: mid, parkAll: false, canUndo: false, fakeMonitors: false,
    lib: { count: 1, duplicates: 0, problems: 1, revision: 1, roots: [{ path: evil, bundled: false, excludes: [evil], online: false, count: 1 }] },
    libSel: [{ monitor: mid, key: '0123456789abcdef0123456789abcdef', name: evil, fit: 'fill', bg: '#000000' }], slides: [{ monitor: mid, minutes: 30, shuffle: false, filter: 'all' }], scrim: 'none', profiles: [evil],
  });
  window.__onLibrary({ req: { id: window.L ? 0 : 0, pg: 0 }, page: { total: 0, items: [] } });
  // the grid ignores replies for an old query token, so use the live token
  const tok = (typeof L !== 'undefined') ? L.token : 1;
  window.__onLibrary({ req: { id: tok, pg: 0 }, page: { total: 2, offset: 0, items: [
    { key: '0123456789abcdef0123456789abcdef', name: evil, path: evil, w: 1, h: 1, fav: false, status: evil, ok: false, bundled: false },
    { key: 'fedcba9876543210fedcba9876543210', name: evil, path: evil, w: 1, h: 1, fav: true, status: 'ready', ok: true, bundled: false } ] } });
  window.__onSecurity({
    firewall: [{ profile: evil, on: true }], av: [{ name: evil, summary: evil, on: true }], defender: { av: true, rtp: true, sigAgeDays: evil, tamper: true }, scOk: true,
    connections: [{ proc: evil, path: evil, pid: 1, remote: evil, class: evil, internet: true }],
    listeners: [{ proc: evil, path: evil, pid: 2, port: evil, bound: evil, exposure: evil, reachable: true }],
    startup: [{ name: evil, command: evil, path: evil, exists: false, scope: evil, source: evil, where: evil, userFolder: true }],
    plinth: { permAllowed: evil, permBlocked: evil, proxyAllowed: evil, proxyBlocked: evil, safe: false }, tookMs: 1000,
  });
  document.querySelector('#wl .wi > button').click();
  // open the library detail for the readable-but-unusable item and for the normal one
  L.sel = { key: 'fedcba9876543210fedcba9876543210', name: evil, path: evil, w: 1, h: 1, fav: false, status: 'ready', ok: true }; renderDetail();
  const n = (s) => document.querySelectorAll(s).length;
  return JSON.stringify({
    pwned: window.__pwned,
    extraScripts: n('script') - 1,
    onerror: n('[onerror]'),
    literal: document.querySelector('#wl .wi b').textContent.startsWith('<img'),
    secLiteral: document.querySelector('#sgrid .tbl td').textContent.startsWith('<img'),
    gridLiteral: !!document.querySelector('.lcard .nm') && document.querySelector('.lcard .nm').textContent.startsWith('<img'),
    monLiteral: document.querySelector('#mcards b').textContent.includes('<img'),
  });
})();
