document.documentElement.dataset.ink = new URLSearchParams(location.search).get('ink') || 'light';
const t = document.getElementById('t'), d = document.getElementById('d');
function tick() {
  const n = new Date();
  t.textContent = n.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });
  d.textContent = n.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' });
}
tick(); setInterval(tick, 5000);
