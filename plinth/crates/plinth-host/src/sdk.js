(function () {
  let n = 0;
  const pending = new Map();
  window.addEventListener('message', (e) => {
    if (e.source !== parent) return;
    const r = e.data || {};
    // Presentation only, sent by the trusted layer page: the widget's skin (ink, accent, font). No code and no data flow back.
    if (r.plinthSkin) {
      const k = r.plinthSkin, de = document.documentElement;
      if (k.ink === 'light' || k.ink === 'dark') de.dataset.ink = k.ink;
      if (typeof k.accent === 'string' && /^#[0-9a-fA-F]{6}$/.test(k.accent)) de.style.setProperty('--accent', k.accent);
      const f = typeof k.font === 'string' && k.font.length < 160 ? k.font : '';
      de.style.setProperty('--plinth-font', f);
      if (document.body) document.body.style.fontFamily = f;
      return;
    }
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
