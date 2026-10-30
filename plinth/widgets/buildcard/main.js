const q = new URLSearchParams(location.search);
document.documentElement.dataset.ink = q.get('ink') || 'light';
if (q.get('accent')) document.documentElement.style.setProperty('--accent', '#' + q.get('accent'));
const $ = (id) => document.getElementById(id);
const clean = (s) => String(s || '').replace(/\(R\)|\(TM\)/g, '').replace(/\s+/g, ' ').trim();
plinth.data.get('system.info').then((i) => {
  $('c').textContent = clean(i.cpu_name) + (i.threads ? ' (' + i.threads + ' threads)' : '');
  $('g').textContent = clean(i.gpu_name) || 'not detected';
  $('r').textContent = i.ram_gb ? i.ram_gb + ' GB' : '--';
  $('o').textContent = i.os || '--';
}).catch(() => { $('h').textContent = 'Allow "your PC\'s parts list" in Plinth > Safety to show your build.'; });
