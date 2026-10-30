const q = new URLSearchParams(location.search);
document.documentElement.dataset.ink = q.get('ink') || 'light';
if (q.get('accent')) document.documentElement.style.setProperty('--accent', '#' + q.get('accent'));
const $ = (id) => document.getElementById(id);
const denied = new Set();
const get = async (key) => { try { const r = await plinth.data.get(key); denied.delete(key); return r; } catch (e) { if (/denied/.test(e.message)) denied.add(key); return null; } };
const pct = (n) => (n == null ? '--' : Math.round(n) + '%');
const gb = (mb) => (mb / 1024).toFixed(mb >= 10240 ? 0 : 1);
const set = (id, w) => { $(id).style.width = Math.max(0, Math.min(100, w || 0)) + '%'; };
async function tick() {
  const [cpu, gpu, ram, net, disk] = await Promise.all(['system.cpu', 'system.gpu', 'system.ram', 'system.network', 'system.disk'].map(get));
  if (cpu) {
    $('cd').style.setProperty('--p', cpu.percent); $('cp').textContent = Math.round(cpu.percent);
    $('cm').textContent = cpu.freq_mhz ? (cpu.freq_mhz / 1000).toFixed(2) + ' GHz' : 'load';
  } else { $('cp').textContent = '--'; $('cm').textContent = denied.has('system.cpu') ? 'not allowed' : ''; }
  if (gpu) {
    $('gd').style.setProperty('--p', gpu.util_pct || 0); $('gp').textContent = gpu.util_pct == null ? '--' : Math.round(gpu.util_pct);
    $('gn').textContent = (gpu.name || 'GPU').replace(/^NVIDIA GeForce /, '').replace(/^NVIDIA /, '');
    const bits = [];
    if (gpu.temp_c != null) bits.push(gpu.temp_c + '°C');
    if (gpu.power_w != null) bits.push(gpu.power_w + ' W');
    if (gpu.fan_pct != null) bits.push('fan ' + gpu.fan_pct + '%');
    $('gm').textContent = bits.join(' · ') || 'load only';
    if (gpu.vram_total_mb) { $('vv').textContent = gb(gpu.vram_used_mb) + ' / ' + gb(gpu.vram_total_mb) + ' GB'; set('vb', 100 * gpu.vram_used_mb / gpu.vram_total_mb); }
    else { $('vv').textContent = 'n/a'; set('vb', 0); }
  } else { $('gp').textContent = '--'; $('gm').textContent = denied.has('system.gpu') ? 'not allowed' : 'no data'; }
  if (ram) { $('rv').textContent = gb(ram.used_mb) + ' / ' + gb(ram.total_mb) + ' GB'; set('rb', 100 * ram.used_mb / ram.total_mb); } else { $('rv').textContent = denied.has('system.ram') ? 'off' : '--'; }
  $('nv').textContent = net ? '↓ ' + net.down_mbps + '  ↑ ' + net.up_mbps + ' Mb/s' : (denied.has('system.network') ? 'off' : '--');
  if (disk) { $('dv').textContent = pct(disk.busy_pct) + ' · ' + (disk.read_mbs + disk.write_mbs).toFixed(1) + ' MB/s'; set('db', disk.busy_pct); } else { $('dv').textContent = denied.has('system.disk') ? 'off' : '--'; }
  $('h').textContent = denied.size ? 'Some readings are switched off. Allow them in Plinth > Safety.' : '';
}
tick(); setInterval(tick, 1500);
