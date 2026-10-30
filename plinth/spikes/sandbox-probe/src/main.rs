//! Spike C: hostile-widget attacks against sandboxed iframes inside WebView2 (wry).
//! Usage: sandbox-probe [--hostcsp on|off]
//!
//! A trusted host page runs several "widgets" as iframes and each tries to escape or exfiltrate.
//! A local TCP server on 127.0.0.1 records every request that actually arrives, so "reached the
//! network" is proven server-side, not inferred from page-side errors. No external hosts are contacted.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

const HOST_HTML: &str = include_str!("host.html");

fn start_server(hits: Arc<Mutex<Vec<String>>>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let hits = hits.clone();
            std::thread::spawn(move || {
                let _ = s.set_read_timeout(Some(Duration::from_millis(800)));
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let first = req.lines().next().unwrap_or("").to_string();
                let upgrade = req.to_ascii_lowercase().contains("upgrade: websocket");
                hits.lock().unwrap().push(format!("{first}{}", if upgrade { " [websocket upgrade]" } else { "" }));
                let (ctype, body) = if first.contains("/x.js") {
                    ("text/javascript", "export default 1;")
                } else {
                    ("text/plain", "SERVER-SECRET")
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = s.write_all(resp.as_bytes());
            });
        }
    });
    port
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let host_csp = args.iter().position(|a| a == "--hostcsp").and_then(|i| args.get(i + 1)).map(|s| s == "on").unwrap_or(false);

    let hits = Arc::new(Mutex::new(Vec::<String>::new()));
    let port = start_server(hits.clone());
    println!("test server on 127.0.0.1:{port}; host CSP frame-src restriction: {}", if host_csp { "ON" } else { "off" });

    let csp_meta = if host_csp { "<meta http-equiv=\"Content-Security-Policy\" content=\"frame-src about:\">" } else { "" };
    let html = HOST_HTML.replace("__PORT__", &port.to_string()).replace("__HOSTCSP__", csp_meta);

    let event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("sandbox-probe (Spike C)")
        .with_inner_size(tao::dpi::LogicalSize::new(560.0, 320.0))
        .build(&event_loop)
        .unwrap();

    let ipc_proxy = proxy.clone();
    let _webview = WebViewBuilder::new()
        .with_html(html)
        .with_ipc_handler(move |req: wry::http::Request<String>| {
            let _ = ipc_proxy.send_event(format!("{}\u{1}{}", req.uri(), req.body()));
        })
        .build(&window)
        .expect("webview");

    let timeout_proxy = proxy;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(60));
        let _ = timeout_proxy.send_event("TIMEOUT\u{1}TIMEOUT".to_string());
    });

    let mut messages: Vec<(String, String)> = Vec::new();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(s) => {
                let (uri, body) = s.split_once('\u{1}').unwrap_or(("", &s));
                messages.push((uri.to_string(), body.to_string()));
                if body.contains("ALLDONE") || body == "TIMEOUT" {
                    println!("\n===== IPC messages received by the Rust host (uri | body) =====");
                    for (u, b) in &messages {
                        println!("{u} | {b}");
                    }
                    println!("\n===== Requests that actually reached the local server =====");
                    let h = hits.lock().unwrap();
                    if h.is_empty() {
                        println!("(none)");
                    }
                    for line in h.iter() {
                        println!("{line}");
                    }
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}
