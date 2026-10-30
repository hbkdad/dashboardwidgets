// Regression check for the hub's output escaping. Run it in a browser console (or the Browser pane's JS tool) with
// plinth/apps/plinth-app/src/hub.html loaded. Expected result: {"pwned":0,"injected":0,"literal":true}.
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
  });
  document.querySelector('#wl .wi > button').click();
  const n = (s) => document.querySelectorAll(s).length;
  return JSON.stringify({
    pwned: window.__pwned,
    injected: n('#wl img') + n('#log img') + n('#dest img') + n('#plog img') + n('#wl script') + n('#log script') + n('#dest script') + n('#plog script'),
    literal: document.querySelector('#wl .wi b').textContent.startsWith('<img'),
  });
})();
