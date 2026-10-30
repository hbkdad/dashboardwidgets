"""One-off helper: serves a page that records a synthetic 1080p30 test clip with MediaRecorder and POSTs it back.
Run: python mkvideo.py <out-dir>   then open http://127.0.0.1:8765/ in a browser. Exits after the first upload."""
import http.server, sys, os

OUT = sys.argv[1]
PAGE = b"""<!doctype html><meta charset=utf-8><body style="font:16px sans-serif;background:#111;color:#eee"><pre id=log>starting</pre><canvas id=c width=1920 height=1080 style="width:480px"></canvas><script>
const log=(t)=>document.getElementById('log').textContent+='\\n'+t;
const c=document.getElementById('c'),x=c.getContext('2d');
const types=['video/mp4;codecs=avc1','video/mp4','video/webm;codecs=vp9','video/webm'];
const type=types.find(t=>MediaRecorder.isTypeSupported(t));log('type: '+type);
const stream=c.captureStream(30);const rec=new MediaRecorder(stream,{mimeType:type,videoBitsPerSecond:8000000});
const chunks=[];rec.ondataavailable=e=>chunks.push(e.data);
const t0=performance.now();
function draw(){const t=(performance.now()-t0)/1000;
 const g=x.createLinearGradient(0,0,1920,1080);g.addColorStop(0,'hsl('+(t*40%360)+',70%,35%)');g.addColorStop(1,'hsl('+((t*40+120)%360)+',70%,20%)');x.fillStyle=g;x.fillRect(0,0,1920,1080);
 for(let i=0;i<40;i++){x.fillStyle='hsla('+(i*9+t*30)+',80%,60%,.35)';x.beginPath();x.arc(960+Math.cos(t*.7+i)*(300+i*10),540+Math.sin(t*.9+i*1.3)*(200+i*6),30+i*2,0,7);x.fill()}
 if(performance.now()-t0<8000)requestAnimationFrame(draw)}
draw();rec.start(500);
setTimeout(()=>rec.stop(),8000);
rec.onstop=async()=>{const blob=new Blob(chunks,{type:type});log('bytes: '+blob.size);
 const ext=type.includes('mp4')?'mp4':'webm';const r=await fetch('/save?ext='+ext,{method:'POST',body:blob});log('saved: '+await r.text());document.title='DONE'};
</script>"""

class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200); self.send_header('Content-Type', 'text/html'); self.end_headers(); self.wfile.write(PAGE)
    def do_POST(self):
        n = int(self.headers.get('Content-Length', 0))
        ext = 'mp4' if 'ext=mp4' in self.path else 'webm'
        data = self.rfile.read(n)
        path = os.path.join(OUT, 'test-loop.' + ext)
        open(path, 'wb').write(data)
        self.send_response(200); self.end_headers(); self.wfile.write(('%s %d bytes' % (path, len(data))).encode())
        print('saved', path, len(data), flush=True)
        self.server.done = True
    def log_message(self, *a): pass

srv = http.server.HTTPServer(('127.0.0.1', 8765), H)
srv.done = False
while not srv.done:
    srv.handle_request()
