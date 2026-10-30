(function () {
  let n = 0;
  const pending = new Map();
  window.addEventListener('message', (e) => {
    if (e.source !== parent) return;
    const r = e.data || {};
    const p = pending.get(r.id);
    if (!p) return;
    pending.delete(r.id);
    r.error ? p.rej(new Error(r.error)) : p.res(r.ok);
  });
  const call = (method, params) => new Promise((res, rej) => {
    const id = ++n;
    pending.set(id, { res, rej });
    parent.postMessage({ id, method, params: params || {} }, '*');
  });
  window.plinth = {
    call,
    data: { get: (key) => call('data.get', { key }) },
    storage: { get: (key) => call('storage.get', { key }), set: (key, value) => call('storage.set', { key, value }) },
    log: (text) => call('diag.log', { text: String(text) }),
  };
})();
