const q = new URLSearchParams(location.search);
document.documentElement.dataset.ink = q.get('ink') || 'light';
if (q.get('accent')) document.documentElement.style.setProperty('--accent', '#' + q.get('accent'));
const $ = (id) => document.getElementById(id);
let denied = new Set();
async function poll() {
  for (const [key, bar, val, fmt] of [['system.cpu', 'cb', 'cv', (r) => [r.percent, Math.round(r.percent) + '%']], ['system.ram', 'rb', 'rv', (r) => [100 * r.used_mb / r.total_mb, (r.used_mb / 1024).toFixed(1) + ' / ' + (r.total_mb / 1024).toFixed(0) + ' GB']]]) {
    try { const r = await plinth.data.get(key); const [pct, txt] = fmt(r); $(bar).style.width = Math.min(100, pct) + '%'; $(val).textContent = txt; denied.delete(key); }
    catch (e) { $(val).textContent = 'off'; $(bar).style.width = '0'; denied.add(key); }
  }
  $('h').textContent = denied.size ? 'Allow access in Plinth > Safety' : '';
}
poll(); setInterval(poll, 2000);
