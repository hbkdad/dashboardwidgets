const port = new URLSearchParams(location.search).get('port');
const base = 'http://127.0.0.1:' + port;
const only = new URLSearchParams(location.search).get('only');
const say = (s) => plinth.log(s).catch(() => {});
const attempt = async (name, f) => {
  if (only && !name.includes(only)) return;
  try { const v = await f(); await say(name + ': SUCCEEDED ' + String(v).slice(0, 40)); }
  catch (e) { await say(name + ': blocked (' + String((e && e.message) || e).slice(0, 50) + ')'); }
};
(async () => {
  await attempt('read cpu (declared, not granted)', () => plinth.data.get('system.cpu'));
  await attempt('read ram (not declared)', () => plinth.data.get('system.ram'));
  await attempt('storage (not declared)', () => plinth.storage.set('k', 'v'));
  await attempt('unknown bridge method', () => plinth.call('exec', { cmd: 'calc.exe' }));
  await attempt('fetch local server', () => fetch(base + '/probe?w=hostile-fetch').then((r) => r.text()));
  await attempt('xhr local server', () => new Promise((res, rej) => { const x = new XMLHttpRequest(); x.open('GET', base + '/probe?w=hostile-xhr'); x.onload = () => res(x.status); x.onerror = () => rej(new Error('xhr error')); x.send(); }));
  await attempt('image beacon', () => new Promise((res, rej) => { const i = new Image(); i.onload = () => res('loaded'); i.onerror = () => rej(new Error('img error')); i.src = base + '/probe?w=hostile-img'; setTimeout(() => rej(new Error('timeout')), 1000); }));
  await attempt('websocket', () => new Promise((res, rej) => { const w = new WebSocket('ws://127.0.0.1:' + port + '/probe?w=hostile-ws'); w.onopen = () => res('open'); w.onerror = () => rej(new Error('ws error')); setTimeout(() => rej(new Error('timeout')), 1000); }));
  await attempt('remote script import', () => import(base + '/probe.js?w=hostile-import'));
  await attempt('read parent DOM', () => parent.document.title);
  await attempt('navigate top window', () => { top.location.href = base + '/probe?w=hostile-top'; return 'assigned'; });
  await attempt('navigate self to attacker URL', () => { setTimeout(() => { location.href = base + '/probe?w=hostile-selfnav'; }, 10); return 'scheduled'; });
  // Impersonation: claim to be another slot. The host decides identity from the iframe, not from the message.
  parent.postMessage({ id: 4242, method: 'data.get', params: { key: 'system.cpu' }, slot: 0 }, '*');
  await say('impersonation message sent (host must answer as THIS widget)');
  await say('hostile finished');
})();
