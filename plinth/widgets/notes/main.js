const q = new URLSearchParams(location.search);
document.documentElement.dataset.ink = q.get('ink') || 'light';
if (q.get('accent')) document.documentElement.style.setProperty('--accent', '#' + q.get('accent'));
const n = document.getElementById('n'), h = document.getElementById('h');
let timer;
plinth.storage.get('note').then((v) => { if (v) n.value = v; }).catch(() => { n.disabled = true; n.placeholder = 'Allow storage in Plinth > Safety to keep notes.'; });
n.addEventListener('input', () => {
  clearTimeout(timer);
  timer = setTimeout(() => plinth.storage.set('note', n.value).then(() => { h.textContent = 'Saved'; }).catch((e) => { h.textContent = 'Not saved: ' + e.message; }), 600);
});
