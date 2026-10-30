//! Plinth app (v0.5): hub window + desktop layer with isolated widgets, permission broker, and a deny-by-default
//! network proxy that all widget traffic is forced through.
//!
//! * The hub is a normal window (trusted UI). Nothing touches your desktop until you press Apply.
//! * Apply creates the desktop layer: a window attached behind the desktop icons that paints the Scene wallpaper and
//!   hosts each widget in its own origin (`http://pw<N>.localhost`) with a CSP generated from its manifest and grants.
//! * Closing the hub exits the app and removes the layer (tray/background mode is not built yet).
//!
//! Dev switches: PLINTH_NO_ATTACH=1 (use a normal window instead of the desktop layer), PLINTH_STATE=<file>,
//! PLINTH_AUTOTEST=<seconds> (apply, exercise, verify, print a report, exit with 0/1).

mod state;
mod win;

use plinth_core::{impact, PermissionId};
use plinth_host::{Host, Package};
use plinth_proxy::{Dest, Log, Policy, Verdict};
use serde_json::{json, Value};
use state::State;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
use tao::platform::windows::WindowExtWindows;
use tao::window::{Window, WindowBuilder};
use wry::{WebContext, WebView, WebViewBuilder};

const HUB_HTML: &str = include_str!("hub.html");
const LAYER_HTML: &str = include_str!("layer.html");

enum Ev {
    Hub(String),
    Layer { slot: usize, msg: String },
    Deliver { slot: usize, json: String },
    Save,
    Menu(String),
    Auto(u8),
}

struct SceneStyle {
    id: &'static str,
    name: &'static str,
    wall: &'static str,
    mid: &'static str,
    accent: &'static str,
    ink: &'static str,
    glass: &'static str,
}

const SCENES: [SceneStyle; 3] = [
    SceneStyle { id: "dusk", name: "Dusk", wall: "linear-gradient(160deg,#2b1b4d 0%,#5b2a6b 40%,#d9667a 85%,#ffb27a 100%)", mid: "radial-gradient(60% 50% at 70% 30%,rgba(255,170,120,.35),transparent 70%)", accent: "ffb27a", ink: "light", glass: "rgba(30,20,50,.38)" },
    SceneStyle { id: "graphite", name: "Graphite", wall: "linear-gradient(160deg,#0d0f14 0%,#1a1e2b 55%,#2c3347 100%)", mid: "radial-gradient(50% 60% at 25% 70%,rgba(124,242,200,.22),transparent 70%)", accent: "7cf2c8", ink: "light", glass: "rgba(20,22,32,.42)" },
    SceneStyle { id: "paper", name: "Paper", wall: "linear-gradient(160deg,#e9e4da 0%,#cfd8dc 60%,#b7c4d6 100%)", mid: "radial-gradient(55% 55% at 75% 25%,rgba(255,255,255,.7),transparent 70%)", accent: "3b6cff", ink: "dark", glass: "rgba(255,255,255,.45)" },
];

fn scene(id: &str) -> &'static SceneStyle {
    SCENES.iter().find(|s| s.id == id).unwrap_or(&SCENES[1])
}

fn permission_label(p: PermissionId) -> &'static str {
    match p {
        PermissionId::SystemCpu => "CPU load",
        PermissionId::SystemRam => "Memory use",
        PermissionId::SystemGpu => "Graphics load",
        PermissionId::SystemDisk => "Disk use",
        PermissionId::SystemNetwork => "Network speed",
        PermissionId::MediaControl => "Media controls",
        PermissionId::CalendarRead => "Your calendar",
        PermissionId::NetFetch => "Internet access",
        PermissionId::Storage => "Remember data on this PC",
        PermissionId::Notifications => "Notifications",
    }
}

struct App {
    host: Arc<Mutex<Host>>,
    st: State,
    policy: Arc<Mutex<Policy>>,
    plog: Arc<Mutex<Log>>,
    proxy_port: u16,
    beacon: bool,
}

impl App {
    fn sync_proxy(&self) {
        let dests = self.host.lock().unwrap().allowed_destinations();
        let mut p = self.policy.lock().unwrap();
        p.set_allowed(dests.into_iter().map(|(host, port)| Dest { host, port }).collect());
        p.set_safe_mode(self.st.safe);
    }

    fn persist(&mut self) {
        let h = self.host.lock().unwrap();
        self.st.grants = h.describe().into_iter().map(|w| (w.id, w.permissions.into_iter().filter(|p| p.granted).map(|p| p.id.as_str().to_string()).collect())).collect();
        self.st.storage = h.storage_snapshot();
        drop(h);
        self.st.save();
    }

    fn state_json(&self, msg: Option<&str>) -> String {
        let h = self.host.lock().unwrap();
        let info = h.describe();
        let names: HashMap<String, String> = info.iter().map(|w| (w.id.clone(), w.name.clone())).collect();
        let widgets: Vec<Value> = info
            .iter()
            .map(|w| {
                json!({ "id": w.id, "name": w.name, "version": w.version, "slot": w.slot, "first": w.id.starts_with("com.plinth."),
                    "perms": w.permissions.iter().map(|p| json!({ "k": p.id.as_str(), "label": permission_label(p.id), "reason": p.reason, "hosts": p.hosts, "on": p.granted })).collect::<Vec<_>>() })
            })
            .collect();
        let mut act: Vec<(u64, Value)> = Vec::new();
        for w in &info {
            for e in h.activity(&w.id) {
                act.push((e.seq, json!({ "seq": e.seq, "widget": names.get(&e.widget).cloned().unwrap_or_default(), "action": e.action, "allowed": e.allowed })));
            }
        }
        act.sort_by(|a, b| b.0.cmp(&a.0));
        let activity: Vec<Value> = act.into_iter().take(80).map(|(_, v)| v).collect();
        let dests: Vec<String> = h.allowed_destinations().into_iter().map(|(h, p)| format!("{h}:{p}")).collect();
        let n = info.len();
        drop(h);
        let proxy: Vec<Value> = self
            .plog
            .lock()
            .unwrap()
            .entries()
            .into_iter()
            .rev()
            .take(40)
            .map(|e| json!({ "seq": e.seq, "method": e.method, "target": e.target, "verdict": if e.verdict == Verdict::Allow { "Allow" } else { "Deny" } }))
            .collect();
        let cost = impact::scene_cost(0, n);
        json!({ "scene": self.st.scene, "applied": self.st.applied, "safe": self.st.safe, "firstRun": self.st.first_run, "widgets": widgets, "activity": activity,
            "destinations": dests, "proxy": proxy, "impact": { "ram": cost.ram_mb.round(), "cpu": format!("{:.1}", cost.cpu_pct) }, "msg": msg })
        .to_string()
    }

    fn layer_html(&self) -> String {
        let s = scene(&self.st.scene);
        let widgets: Vec<Value> = if self.st.safe { vec![] } else { self.host.lock().unwrap().installed().into_iter().map(|(slot, id)| json!({ "slot": slot, "id": id })).collect() };
        let banner = if self.st.safe { r#"<div style="position:fixed;left:50%;top:6%;transform:translateX(-50%);padding:10px 18px;border-radius:14px;background:rgba(0,0,0,.55);border:1px solid #ffb454;color:#ffb454;font:600 14px system-ui">Safe mode: widgets are paused</div>"# } else { "" };
        let beacon = if self.beacon { r#"<div style="position:fixed;left:50%;top:50%;width:40px;height:40px;margin:-20px 0 0 -20px;background:#ff00ff"></div>"# } else { "" };
        LAYER_HTML
            .replace("__WALL__", s.wall)
            .replace("__MID__", s.mid)
            .replace("__GLASS__", s.glass)
            .replace("__INK__", s.ink)
            .replace("__ACCENT__", s.accent)
            .replace("__WIDGETS__", &Value::Array(widgets).to_string())
            .replace("__BANNER__", banner)
            .replace("__BEACON__", beacon)
    }
}

struct Layer {
    _window: Window,
    webview: WebView,
}

fn widgets_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("PLINTH_WIDGETS") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        let beside = exe.with_file_name("widgets");
        if beside.is_dir() {
            return beside;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../widgets")
}

fn create_layer(target: &EventLoopWindowTarget<Ev>, app: &App, ctx: &mut WebContext, proxy: &EventLoopProxy<Ev>) -> Option<Layer> {
    let monitor = target.primary_monitor()?;
    let (size, pos) = (monitor.size(), monitor.position());
    let window = WindowBuilder::new().with_title("Plinth desktop layer").with_decorations(false).with_visible(false).with_resizable(false).build(target).ok()?;
    let attached = std::env::var_os("PLINTH_NO_ATTACH").is_none() && win::attach_to_desktop(window.hwnd() as isize, pos.x, pos.y, size.width as i32, size.height as i32);
    if !attached {
        eprintln!("desktop layer attach skipped or failed: using a normal window");
        window.set_inner_size(tao::dpi::PhysicalSize::new(size.width / 2, size.height / 2));
        window.set_visible(true);
    }
    let host = app.host.clone();
    let ipc_proxy = proxy.clone();
    let (w, h) = if attached { (size.width, size.height) } else { (size.width / 2, size.height / 2) };
    let mut b = WebViewBuilder::new_with_web_context(ctx)
        .with_html(app.layer_html())
        .with_bounds(wry::Rect { position: wry::dpi::PhysicalPosition::new(0, 0).into(), size: wry::dpi::PhysicalSize::new(w, h).into() })
        .with_ipc_handler(move |req: wry::http::Request<String>| {
            let Ok(v) = serde_json::from_str::<Value>(req.body()) else { return };
            if let (Some(slot), Some(msg)) = (v.get("slot").and_then(Value::as_u64), v.get("msg").and_then(Value::as_str)) {
                let _ = ipc_proxy.send_event(Ev::Layer { slot: slot as usize, msg: msg.to_string() });
            }
        });
    for slot in 0..plinth_host::THIRD_PARTY_SLOTS {
        let h = host.clone();
        // The slot is identified by the protocol the request arrived on, never by anything the page controls.
        b = b.with_custom_protocol(Host::scheme_for_slot(slot), move |_id, req| {
            let name = format!("{}.localhost", Host::scheme_for_slot(slot));
            let path = req.uri().path_and_query().map(|p| p.as_str().to_string()).unwrap_or_else(|| "/".into());
            let r = h.lock().unwrap().serve(&name, &path);
            let mut resp = wry::http::Response::builder().status(r.status);
            for (k, v) in &r.headers {
                resp = resp.header(k.as_str(), v.as_str());
            }
            resp.body(std::borrow::Cow::Owned(r.body)).unwrap()
        });
    }
    use wry::WebViewBuilderExtWindows;
    b = b.with_additional_browser_args(format!("--proxy-server=http://127.0.0.1:{} --proxy-bypass-list=<-loopback>", app.proxy_port));
    let webview = b.build(&window).ok()?;
    Some(Layer { _window: window, webview })
}

fn main() {
    let instance = format!("Local\\PlinthApp-{}", std::env::var("PLINTH_STATE").unwrap_or_default().chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>());
    if !win::first_instance(&instance) {
        println!("Plinth is already running; bringing its window to the front.");
        let _ = win::activate_existing_hub();
        return;
    }
    let autotest: Option<u64> = std::env::var("PLINTH_AUTOTEST").ok().and_then(|s| s.parse().ok());

    // ---- state, host, widgets ----
    let mut st = State::load();
    let host = Arc::new(Mutex::new(Host::new(Box::new(win::SystemProvider::new()))));
    {
        let mut h = host.lock().unwrap();
        let dir = widgets_dir();
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(&dir).map(|r| r.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
        dirs.sort();
        for d in dirs {
            match Package::from_dir(&d) {
                Ok(p) => {
                    let id = p.manifest.id.clone();
                    match h.install(p) {
                        Ok(slot) => println!("installed {id} in slot {slot}"),
                        Err(e) => eprintln!("cannot install {id}: {e:?}"),
                    }
                }
                Err(e) => eprintln!("skipping {}: {e:?}", d.display()),
            }
        }
        if st.first_run {
            // Built-in widgets are trusted by default; anything else starts with nothing granted.
            for w in h.describe() {
                if w.id.starts_with("com.plinth.") {
                    for p in &w.permissions {
                        let _ = h.grant(&w.id, p.id);
                    }
                }
            }
        } else {
            for (id, perms) in &st.grants {
                for k in perms {
                    if let Some(p) = PermissionId::parse(k) {
                        let _ = h.grant(id, p);
                    }
                }
            }
        }
        h.restore_storage(st.storage.clone());
        h.set_safe_mode(st.safe);
    }

    // ---- proxy ----
    let policy = Arc::new(Mutex::new(Policy::new()));
    let plog = Arc::new(Mutex::new(Log::default()));
    let proxy_port = plinth_proxy::start(policy.clone(), plog.clone()).expect("proxy");
    println!("network proxy on 127.0.0.1:{proxy_port}");
    st.first_run = st.first_run && autotest.is_none();
    let mut app = App { host: host.clone(), st, policy, plog, proxy_port, beacon: autotest.is_some() };
    app.sync_proxy();
    if app.st.first_run {
        app.persist(); // creates the state file so the welcome sheet shows once
    }

    // ---- windows ----
    let event_loop = EventLoopBuilder::<Ev>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let hub_window = WindowBuilder::new().with_title("Plinth").with_inner_size(tao::dpi::LogicalSize::new(1180.0, 780.0)).build(&event_loop).unwrap();
    let hub_id = hub_window.id();
    let mut hub_ctx = WebContext::new(Some(state::local_dir().join("wv-hub")));
    let hub_proxy = proxy.clone();
    let hub_view = WebViewBuilder::new_with_web_context(&mut hub_ctx)
        .with_html(HUB_HTML)
        .with_ipc_handler(move |req: wry::http::Request<String>| {
            let _ = hub_proxy.send_event(Ev::Hub(req.body().clone()));
        })
        .build(&hub_window)
        .expect("hub webview");
    // Tray icon + menu (background mode: closing the hub hides it; Quit in the tray exits).
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    let menu = Menu::new();
    let mi_open = MenuItem::new("Open Plinth", true, None);
    let mi_safe = MenuItem::new("Toggle Safe mode", true, None);
    let mi_quit = MenuItem::new("Quit Plinth", true, None);
    let _ = menu.append_items(&[&mi_open, &mi_safe, &PredefinedMenuItem::separator(), &mi_quit]);
    let (id_open, id_safe, id_quit) = (mi_open.id().clone(), mi_safe.id().clone(), mi_quit.id().clone());
    let _tray = tray_icon::Icon::from_rgba(win::tray_icon_rgba(), 32, 32)
        .ok()
        .and_then(|icon| tray_icon::TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("Plinth").with_icon(icon).build().ok());
    println!("tray icon: {}", if _tray.is_some() { "created" } else { "NOT created" });
    {
        let p = proxy.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let name = if e.id == id_open { "open" } else if e.id == id_safe { "safe" } else if e.id == id_quit { "quit" } else { return };
            let _ = p.send_event(Ev::Menu(name.to_string()));
        }));
        let p2 = proxy.clone();
        tray_icon::TrayIconEvent::set_event_handler(Some(move |e: tray_icon::TrayIconEvent| {
            if let tray_icon::TrayIconEvent::DoubleClick { .. } = e {
                let _ = p2.send_event(Ev::Menu("open".to_string()));
            }
        }));
    }
    let mut layer_ctx = WebContext::new(Some(state::local_dir().join("wv-layer")));
    let mut layer: Option<Layer> = None;

    let tick = proxy.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        let _ = tick.send_event(Ev::Save);
    });
    if let Some(secs) = autotest {
        let p = proxy.clone();
        std::thread::spawn(move || {
            let hub = |p: &EventLoopProxy<Ev>, j: &str| {
                let _ = p.send_event(Ev::Hub(j.to_string()));
            };
            let nap = |s: u64| std::thread::sleep(Duration::from_secs(s));
            nap(2);
            hub(&p, r#"{"cmd":"apply"}"#); // same code path as pressing Apply in the hub
            nap(6);
            hub(&p, r#"{"cmd":"grant","id":"com.plinth.sysmon","perm":"system.cpu","on":false}"#); // user revokes CPU
            nap(5);
            let _ = p.send_event(Ev::Auto(5)); // snapshot activity count
            hub(&p, r#"{"cmd":"safe","on":true}"#);
            nap(5);
            let _ = p.send_event(Ev::Auto(6)); // compare: nothing may run in safe mode
            hub(&p, r#"{"cmd":"safe","on":false}"#);
            nap(secs.saturating_sub(24).max(2));
            let _ = p.send_event(Ev::Auto(2)); // show desktop
            nap(2);
            let _ = p.send_event(Ev::Auto(3)); // sample + restore
            nap(2);
            let _ = p.send_event(Ev::Auto(4)); // report
        });
    }
    let mut auto_visible: Option<bool> = None;
    let mut safe_count: (usize, usize) = (0, 0);
    let mut applied_before_start = app.st.applied;

    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        if applied_before_start {
            applied_before_start = false;
            layer = create_layer(target, &app, &mut layer_ctx, &proxy);
            println!("restored applied scene {} (layer created: {})", app.st.scene, layer.is_some());
        }
        let reply = |app: &App, msg: Option<&str>| {
            let _ = hub_view.evaluate_script(&format!("__onState({})", app.state_json(msg)));
        };
        match event {
            Event::UserEvent(Ev::Hub(body)) => {
                let Ok(v) = serde_json::from_str::<Value>(&body) else { return };
                let cmd = v.get("cmd").and_then(Value::as_str).unwrap_or("");
                let mut msg: Option<String> = None;
                let mut reload = false;
                match cmd {
                    "state" => {}
                    "scene" => {
                        if let Some(id) = v.get("id").and_then(Value::as_str) {
                            app.st.scene = scene(id).id.to_string();
                            reload = app.st.applied;
                        }
                    }
                    "apply" => {
                        app.st.applied = true;
                        if let Some(l) = layer.as_ref() {
                            let _ = l.webview.load_html(&app.layer_html());
                        } else {
                            layer = create_layer(target, &app, &mut layer_ctx, &proxy);
                        }
                        println!("hub: apply {} (layer present: {})", app.st.scene, layer.is_some());
                        msg = Some(format!("Applied {}. Your previous desktop is restored with Revert.", scene(&app.st.scene).name));
                    }
                    "revert" => {
                        app.st.applied = false;
                        layer = None;
                        println!("hub: revert");
                        msg = Some("Reverted. Your desktop is back to how it was.".into());
                    }
                    "grant" => {
                        let (id, perm, on) = (v.get("id").and_then(Value::as_str).unwrap_or(""), v.get("perm").and_then(Value::as_str).unwrap_or(""), v.get("on").and_then(Value::as_bool).unwrap_or(false));
                        if let Some(p) = PermissionId::parse(perm) {
                            let mut h = app.host.lock().unwrap();
                            if on {
                                let _ = h.grant(id, p);
                            } else {
                                h.revoke(id, p);
                            }
                            drop(h);
                            app.sync_proxy();
                            reload = app.st.applied;
                            msg = Some(if on { "Allowed.".into() } else { "Revoked. It takes effect immediately.".into() });
                        }
                    }
                    "safe" => {
                        app.st.safe = v.get("on").and_then(Value::as_bool).unwrap_or(false);
                        app.host.lock().unwrap().set_safe_mode(app.st.safe);
                        app.sync_proxy();
                        reload = app.st.applied;
                        msg = Some(if app.st.safe { "Safe mode on. Widgets are paused and the network is closed.".into() } else { "Safe mode off.".into() });
                    }
                    "firstrun_done" => {
                        app.st.first_run = false;
                        println!("hub: first run acknowledged");
                    }
                    _ => {}
                }
                if reload {
                    if let Some(l) = layer.as_ref() {
                        let _ = l.webview.load_html(&app.layer_html());
                    }
                }
                if cmd != "state" {
                    println!("hub: {cmd} handled");
                    app.persist();
                }
                reply(&app, msg.as_deref());
            }
            Event::UserEvent(Ev::Layer { slot, msg }) => {
                let json = app.host.lock().unwrap().bridge(slot, &msg);
                let _ = proxy.send_event(Ev::Deliver { slot, json });
            }
            Event::UserEvent(Ev::Deliver { slot, json }) => {
                if let Some(l) = layer.as_ref() {
                    let _ = l.webview.evaluate_script(&format!("__deliver({slot},{json})"));
                }
            }
            Event::UserEvent(Ev::Save) => app.persist(),
            Event::UserEvent(Ev::Menu(name)) => match name.as_str() {
                "open" => {
                    hub_window.set_visible(true);
                    hub_window.set_minimized(false);
                    hub_window.set_focus();
                }
                "safe" => {
                    let on = !app.st.safe;
                    let _ = proxy.send_event(Ev::Hub(json!({ "cmd": "safe", "on": on }).to_string()));
                }
                _ => {
                    app.persist();
                    *control_flow = ControlFlow::Exit;
                }
            },
            Event::UserEvent(Ev::Auto(step)) => match step {
                5 => safe_count.0 = total_activity(&app),
                6 => {
                    safe_count.1 = total_activity(&app);
                    println!("autotest: activity entries during safe mode window: {}", safe_count.1 - safe_count.0);
                }
                2 => win::show_desktop(),
                3 => {
                    auto_visible = Some(win::probe_center_is(0x00FF_00FF));
                    win::show_desktop();
                }
                _ => {
                    let code = autotest_report(&app, auto_visible, safe_count);
                    app.persist();
                    std::process::exit(code);
                }
            },
            Event::WindowEvent { event: WindowEvent::CloseRequested, window_id, .. } if window_id == hub_id => {
                // Background mode: hide the hub, keep the desktop layer and tray running. Quit from the tray menu.
                app.persist();
                hub_window.set_visible(false);
                println!("hub hidden (running in the background; use the tray icon to reopen or quit)");
            }
            _ => {}
        }
    });
}

fn total_activity(app: &App) -> usize {
    let h = app.host.lock().unwrap();
    h.describe().iter().map(|w| h.activity(&w.id).len()).sum()
}

fn autotest_report(app: &App, visible: Option<bool>, safe_count: (usize, usize)) -> i32 {
    let h = app.host.lock().unwrap();
    println!("\n===== Permission Center log =====");
    let mut allowed: HashMap<String, Vec<String>> = HashMap::new();
    let mut blocked: HashMap<String, Vec<String>> = HashMap::new();
    for w in h.describe() {
        println!("-- {} ({})", w.name, w.id);
        for e in h.activity(&w.id) {
            println!("   #{:<3} {:<7} {}", e.seq, if e.allowed { "allowed" } else { "BLOCKED" }, e.action);
            (if e.allowed { &mut allowed } else { &mut blocked }).entry(w.id.clone()).or_default().push(e.action.clone());
        }
    }
    drop(h);
    let entries = app.plog.lock().unwrap().entries();
    println!("\n===== plinth-proxy log =====");
    for e in &entries {
        println!("   #{:<3} {:<8} {:<42} {:?}", e.seq, e.method, e.target, e.verdict);
    }
    let has = |m: &HashMap<String, Vec<String>>, id: &str, what: &str| m.get(id).map_or(false, |v| v.iter().any(|a| a == what));
    let checks = [
        ("sysmon: CPU read allowed (granted at first run)", has(&allowed, "com.plinth.sysmon", "use system.cpu")),
        ("sysmon: memory read allowed", has(&allowed, "com.plinth.sysmon", "use system.ram")),
        ("notes: storage read allowed", has(&allowed, "com.plinth.notes", "use storage")),
        ("clock: made no permission requests (needs none)", !allowed.contains_key("com.plinth.clock") && !blocked.contains_key("com.plinth.clock")),
        ("revoke: after the user revoked CPU, sysmon's CPU request was BLOCKED immediately", has(&blocked, "com.plinth.sysmon", "use system.cpu")),
        ("revoke: only the revoked permission was blocked (memory stayed allowed)", blocked.values().flatten().all(|a| a == "use system.cpu")),
        ("safe mode: at most 2 stray widget events while on (iframes were removed)", safe_count.1.saturating_sub(safe_count.0) <= 2),
        ("hub apply command created the layer and persisted state", app.st.applied),
        ("proxy: 0 tunnels allowed (no widget has network access)", entries.iter().all(|e| e.verdict != Verdict::Allow)),
        ("layer visible after Show Desktop (beacon pixel read at screen center)", visible == Some(true)),
    ];
    println!("\n===== Checks =====");
    let mut failed = 0;
    for (n, ok) in &checks {
        println!("[{}] {n}", if *ok { "PASS" } else { "FAIL" });
        if !ok {
            failed += 1;
        }
    }
    println!("\n{} of {} checks passed", checks.len() - failed, checks.len());
    i32::from(failed != 0)
}
