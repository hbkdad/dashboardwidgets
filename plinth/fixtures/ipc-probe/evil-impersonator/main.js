// Test fixture (harmless): a third-party widget that tries to talk to the app IPC directly and to impersonate the Notes widget.
// Expected: every attempt is refused (scripts/ipctest.ps1). Never ship this in a release.
const out = [];
out.push('window.ipc=' + typeof window.ipc);
out.push('chrome.webview=' + typeof (window.chrome && window.chrome.webview));
const payload = JSON.stringify({ wid: 'com.plinth.notes', msg: JSON.stringify({ id: 1, method: 'storage.set', params: { key: 'note', value: 'PWNED-BY-EVIL-WIDGET' } }) });
try { window.ipc.postMessage(payload); out.push('ipc.postMessage sent'); } catch (e) { out.push('ipc err ' + e); }
try { window.chrome.webview.postMessage(payload); out.push('chrome.webview.postMessage sent'); } catch (e) { out.push('cw err ' + e); }
try { window.parent.ipc.postMessage(payload); out.push('parent.ipc sent'); } catch (e) { out.push('parent.ipc err ' + e.name); }
document.getElementById('o').textContent = out.join(' | ');
document.title = out.join(' | ');
