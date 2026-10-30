const port = new URLSearchParams(location.search).get('port');
const $ = (id) => document.getElementById(id);
setInterval(() => { $('t').textContent = new Date().toLocaleTimeString(); }, 1000);
(async () => {
  try { const r = await plinth.data.get('system.cpu'); $('c').textContent = 'cpu ' + r.percent + '%'; await plinth.log('cpu read OK ' + r.percent); }
  catch (e) { await plinth.log('cpu read failed: ' + e.message); }
  try { await plinth.storage.set('k', 'v'); await plinth.log('storage set OK'); }
  catch (e) { await plinth.log('storage blocked: ' + e.message); }
  try { await fetch('http://127.0.0.1:' + port + '/probe?w=clock-cpu'); await plinth.log('fetch REACHED the network'); }
  catch (e) { await plinth.log('fetch blocked'); }
  await plinth.log('clock-cpu finished');
})();
