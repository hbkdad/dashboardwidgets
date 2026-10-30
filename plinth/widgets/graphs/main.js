const q = new URLSearchParams(location.search);
document.documentElement.dataset.ink = q.get('ink') || 'light';
const accent = '#' + (q.get('accent') || '7cf2c8');
const $ = (id) => document.getElementById(id);
const N = 90;                                   // about a minute and a half at one sample per second
const hist = [[], [], [], []];                  // cpu %, gpu %, ram %, network Mb/s (down)
const denied = new Set();
const get = async (key) => { try { const r = await plinth.data.get(key); denied.delete(key); return r; } catch (e) { if (/denied/.test(e.message)) denied.add(key); return null; } };
const push = (i, v) => { const h = hist[i]; h.push(v == null ? null : v); if (h.length > N) h.shift(); };
function draw(i, max, color) {
  const c = $('c' + i), r = c.getBoundingClientRect(), d = Math.min(window.devicePixelRatio || 1, 2);
  const w = Math.max(10, Math.round(r.width * d)), h = Math.max(10, Math.round(r.height * d));
  if (c.width !== w || c.height !== h) { c.width = w; c.height = h; }
  const x = c.getContext('2d'); x.clearRect(0, 0, w, h);
  const data = hist[i]; if (data.length < 2) return;
  const top = max || Math.max(1, ...data.filter((v) => v != null)) * 1.15;
  x.lineWidth = 1.6 * d; x.strokeStyle = color; x.lineJoin = 'round';
  const step = w / (N - 1), off = (N - data.length) * step;
  x.beginPath(); let started = false;
  data.forEach((v, k) => { if (v == null) { started = false; return; } const px = off + k * step, py = h - (Math.min(v, top) / top) * (h - 3 * d) - 1.5 * d; if (!started) { x.moveTo(px, py); started = true; } else x.lineTo(px, py); });
  x.stroke();
  x.lineTo(w, h); x.lineTo(off, h); x.closePath(); x.globalAlpha = 0.16; x.fillStyle = color; x.fill(); x.globalAlpha = 1;
}
async function tick() {
  const [cpu, gpu, ram, net] = await Promise.all(['system.cpu', 'system.gpu', 'system.ram', 'system.network'].map(get));
  push(0, cpu ? cpu.percent : null); push(1, gpu && gpu.util_pct != null ? gpu.util_pct : null);
  push(2, ram && ram.total_mb ? (100 * ram.used_mb) / ram.total_mb : null); push(3, net ? net.down_mbps : null);
  $('v0').textContent = cpu ? Math.round(cpu.percent) + '%' + (cpu.freq_mhz ? ' · ' + (cpu.freq_mhz / 1000).toFixed(1) + ' GHz' : '') : (denied.has('system.cpu') ? 'not allowed' : '--');
  $('v1').textContent = gpu ? (gpu.util_pct != null ? Math.round(gpu.util_pct) + '%' : '--') + (gpu.temp_c != null ? ' · ' + gpu.temp_c + '°C' : '') : (denied.has('system.gpu') ? 'not allowed' : '--');
  $('v2').textContent = ram && ram.total_mb ? (ram.used_mb / 1024).toFixed(1) + ' / ' + (ram.total_mb / 1024).toFixed(0) + ' GB' : (denied.has('system.ram') ? 'not allowed' : '--');
  $('v3').textContent = net ? '↓ ' + net.down_mbps + '  ↑ ' + net.up_mbps + ' Mb/s' : (denied.has('system.network') ? 'not allowed' : '--');
  draw(0, 100, accent); draw(1, 100, '#8ab4ff'); draw(2, 100, '#ffb454'); draw(3, 0, '#c792ff');
  $('h').textContent = denied.size ? 'Some graphs are switched off. Allow them in Plinth > Safety.' : '';
}
tick(); setInterval(tick, 1000);
