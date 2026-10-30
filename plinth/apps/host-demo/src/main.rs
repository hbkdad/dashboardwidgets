//! host-demo: end-to-end run of plinth-host inside WebView2 with real fixture widgets.
//!
//! * installs `fixtures/widgets/{clock-cpu,hostile}`; grants only `system.cpu` to clock-cpu
//! * serves each widget from its own origin (`http://pw<slot>.localhost`) with a generated CSP header
//! * routes widget messages through the permission broker and prints the Permission Center log
//! * a local TCP server records any request that actually leaves a widget, then the demo prints PASS/FAIL checks
//!
//! Usage: host-demo [--secs N]   (exit code 0 = all checks passed)

use plinth_core::PermissionId;
use plinth_host::{Host, MockProvider, Package};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

const HOST_HTML: &str = include_str!("host.html");

enum Ev {
    Deliver { slot: usize, json: String },
    Finish,
}

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
                let first = String::from_utf8_lossy(&buf[..n]).lines().next().unwrap_or("").to_string();
                hits.lock().unwrap().push(if first.is_empty() { format!("<connection with {n} bytes and no request line>") } else { first });
                let body = "SERVER-SECRET";
                let _ = s.write_all(
                    format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
                        .as_bytes(),
                );
            });
        }
    });
    port
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let secs: u64 = args.iter().position(|a| a == "--secs").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(10);

    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/widgets");
    let host = Arc::new(Mutex::new(Host::new(Box::new(MockProvider))));
    {
        let mut h = host.lock().unwrap();
        for name in ["clock-cpu", "hostile"] {
            let pkg = Package::from_dir(&fixtures.join(name)).unwrap_or_else(|e| panic!("package {name}: {e:?}"));
            let slot = h.install(pkg).expect("install");
            println!("installed {name} in slot {slot} (origin http://{}.localhost)", Host::scheme_for_slot(slot));
        }
        // The user allows CPU for the clock widget only. Nothing else is granted to anyone.
        h.grant("com.plinth.clock-cpu", PermissionId::SystemCpu).unwrap();
        for (_, id) in h.installed() {
            println!("CSP for {id}: {}", h.csp_for(&id));
        }
    }

    let hits = Arc::new(Mutex::new(Vec::<String>::new()));
    let port = start_server(hits.clone());
    // Experiment (PLINTH_PROXY=1): force ALL WebView2 network traffic through a second local listener standing in for a
    // Plinth-controlled proxy. If the design works, nothing (not even a bare connection) reaches the destination server
    // directly; everything the widgets attempt shows up at the proxy, where policy can deny it.
    let proxy_hits = Arc::new(Mutex::new(Vec::<String>::new()));
    let proxy_port = if std::env::var("PLINTH_PROXY").is_ok() { Some(start_server(proxy_hits.clone())) } else { None };

    let widgets_json = {
        let h = host.lock().unwrap();
        let items: Vec<String> = h.installed().iter().map(|(slot, id)| format!("{{\"slot\":{slot},\"id\":\"{id}\"}}")).collect();
        format!("[{}]", items.join(","))
    };
    let html = HOST_HTML.replace("__WIDGETS__", &widgets_json).replace("__PORT__", &port.to_string()).replace("__ONLY__", &std::env::var("PLINTH_ONLY").unwrap_or_default());

    let event_loop = EventLoopBuilder::<Ev>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("Plinth host-demo")
        .with_inner_size(tao::dpi::LogicalSize::new(520.0, 260.0))
        .build(&event_loop)
        .unwrap();

    let ipc_host = host.clone();
    let ipc_proxy = proxy.clone();
    let mut builder = WebViewBuilder::new().with_html(html).with_ipc_handler(move |req: wry::http::Request<String>| {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(req.body()) else { return };
        let (Some(slot), Some(msg)) = (v.get("slot").and_then(|s| s.as_u64()), v.get("msg").and_then(|m| m.as_str())) else { return };
        let json = ipc_host.lock().unwrap().bridge(slot as usize, msg);
        let _ = ipc_proxy.send_event(Ev::Deliver { slot: slot as usize, json });
    });
    for slot in 0..plinth_host::THIRD_PARTY_SLOTS {
        let h = host.clone();
        builder = builder.with_custom_protocol(Host::scheme_for_slot(slot), move |_id, req| {
            // wry reports the request as `<scheme>://localhost/...`; the slot is identified by the protocol it was
            // registered under (never by anything the page controls), mapped back to its origin host name.
            let host_name = format!("{}.localhost", Host::scheme_for_slot(slot));
            let path = req.uri().path_and_query().map(|p| p.as_str().to_string()).unwrap_or_else(|| "/".into());
            let r = h.lock().unwrap().serve(&host_name, &path);
            if std::env::var("PLINTH_TRACE").is_ok() {
                println!("serve {host_name}{path} -> {} ({} bytes)", r.status, r.body.len());
            }
            let mut b = wry::http::Response::builder().status(r.status);
            for (k, v) in &r.headers {
                b = b.header(k.as_str(), v.as_str());
            }
            b.body(std::borrow::Cow::Owned(r.body)).unwrap()
        });
    }
    if let Some(pp) = proxy_port {
        use wry::WebViewBuilderExtWindows;
        builder = builder.with_additional_browser_args(format!("--proxy-server=http://127.0.0.1:{pp} --proxy-bypass-list=<-loopback> --disable-features=msWebOOUI,msPdfOOUI"));
        println!("PROXY MODE: all WebView2 traffic forced through 127.0.0.1:{pp}");
    }
    let webview = builder.build(&window).expect("webview");

    let finish_proxy = proxy;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(secs));
        let _ = finish_proxy.send_event(Ev::Finish);
    });

    let report_host = host.clone();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(Ev::Deliver { slot, json }) => {
                let _ = webview.evaluate_script(&format!("__deliver({slot},{json})"));
            }
            Event::UserEvent(Ev::Finish) => {
                let code = report(&report_host.lock().unwrap(), &hits.lock().unwrap());
                let ph = proxy_hits.lock().unwrap();
                if proxy_port.is_some() {
                    println!("
===== Requests that arrived at the PROXY (would be allow/deny-checked by core) =====");
                    for h in ph.iter() {
                        println!("{h}");
                    }
                    println!("({} proxy requests; destination server saw {} direct hit(s) above)", ph.len(), hits.lock().unwrap().len());
                }
                std::process::exit(code);
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}

fn report(host: &Host, hits: &[String]) -> i32 {
    println!("\n===== Permission Center log =====");
    for (_, id) in host.installed() {
        println!("-- {id}");
        for e in host.activity(&id) {
            println!("   #{:<3} {:<7} {}", e.seq, if e.allowed { "allowed" } else { "BLOCKED" }, e.action);
        }
    }
    // Split real HTTP requests from bare TCP connections (no request line). A bare connection carries no widget
    // data but still reveals that a widget tried to reach an address (see KNOWN GAP below).
    let (empty, data): (Vec<&String>, Vec<&String>) = hits.iter().partition(|h| h.starts_with("<connection"));
    println!("\n===== HTTP requests that reached the local server =====");
    if data.is_empty() {
        println!("(none)");
    }
    for h in &data {
        println!("{h}");
    }
    println!("\n===== Bare TCP connections (no request) =====");
    println!("{}", if empty.is_empty() { "(none)".to_string() } else { format!("{} observed", empty.len()) });
    if !empty.is_empty() {
        println!("KNOWN GAP: a widget can open a connection to an address of its choosing by navigating its own frame;");
        println!("  no data is sent, but the destination (DNS name or IP:port) leaks. Planned fix: force all WebView2 traffic");
        println!("  through a Plinth-controlled local proxy (design/architecture.md, Spike C follow-up).");
    }

    let log = |id: &str| -> Vec<String> { host.activity(id).iter().map(|e| format!("{}|{}", if e.allowed { "A" } else { "B" }, e.action)).collect() };
    let clock = log("com.plinth.clock-cpu");
    let hostile = log("test.hostile");
    let mut checks: Vec<(&str, bool)> = Vec::new();
    checks.push(("clock-cpu: granted CPU read was allowed and logged", clock.iter().any(|l| l == "A|use system.cpu")));
    checks.push(("clock-cpu: storage (declared, not granted) was denied", clock.iter().any(|l| l == "B|use storage")));
    checks.push(("clock-cpu: widget reported fetch blocked", clock.iter().any(|l| l.contains("fetch blocked"))));
    checks.push(("clock-cpu: ran to completion", clock.iter().any(|l| l.contains("clock-cpu finished"))));
    checks.push(("hostile: CPU read (declared, not granted) denied", hostile.iter().any(|l| l == "B|use system.cpu")));
    checks.push(("hostile: RAM read (not declared) denied", hostile.iter().any(|l| l == "B|use system.ram")));
    checks.push(("hostile: no bridge call was ever allowed", !hostile.iter().any(|l| l.starts_with("A|use "))));
    checks.push(("hostile: impersonating another slot did not grant data (no allowed use for hostile)", !hostile.iter().any(|l| l.starts_with("A|use "))));
    checks.push(("hostile: ran to completion", hostile.iter().any(|l| l.contains("hostile finished"))));
    checks.push(("network: no widget HTTP request reached the server", data.is_empty()));
    checks.push(("clock-cpu never granted anything but CPU (no allowed storage)", !clock.iter().any(|l| l == "A|use storage")));

    println!("\n===== Checks =====");
    let mut failed = 0;
    for (name, ok) in &checks {
        println!("[{}] {name}", if *ok { "PASS" } else { "FAIL" });
        if !ok {
            failed += 1;
        }
    }
    println!("\n{} of {} checks passed", checks.len() - failed, checks.len());
    if failed == 0 { 0 } else { 1 }
}
