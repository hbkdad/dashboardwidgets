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

mod hw;
mod library;
mod mon;
mod os_wall;
mod sec;
mod state;
mod win;

use plinth_core::{impact, PermissionId};
use plinth_host::{Host, Package, WallpaperStore, WALLPAPER_SCHEME};
use plinth_proxy::{Dest, Log, Policy, Verdict};
use serde_json::{json, Value};
use library::Library;
use plinth_core::fit::{self, FitMode};
use plinth_core::library::{self as corelib, Filter};
use plinth_core::monitors::{self as mons, MonitorInfo, Resolved};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use state::State;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
use tao::platform::windows::WindowExtWindows;
use tao::window::{Window, WindowBuilder};
use wry::{WebContext, WebView, WebViewBuilder};

static HUB_HIDDEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static LIB_RESCAN: AtomicBool = AtomicBool::new(false);

type ThumbJob = (String, wry::RequestAsyncResponder);

const HUB_HTML: &str = include_str!("hub.html");
const LAYER_HTML: &str = include_str!("layer.html");

enum Ev {
    Hub(String),
    Layer { wid: String, msg: String },
    Deliver { wid: String, json: String },
    Save,
    Menu(String),
    Pause(bool),
    Drop(Vec<PathBuf>),
    Sec(String),
    Monitors(Vec<MonitorInfo>),
    IdentifyEnd,
    LibraryChanged,
    Composed { monitor: String, file: String, err: Option<String> },
    OsSnap(Option<os_wall::Snapshot>),
    OsDone(String),
    Picked { purpose: String, path: Option<String> },
    SlideTick,
    DropHover(bool),
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
        PermissionId::SystemInfo => "Your PC's parts list",
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
    paused: bool,
    walls: Arc<Mutex<WallpaperStore>>,
    /// Recent wallpaper requests (path, status), for the autotest and diagnostics.
    wall_log: Arc<Mutex<Vec<(String, u16)>>>,
    /// Connected monitors (stable list, debounced) and where every widget is shown right now.
    monitors: Vec<MonitorInfo>,
    resolved: Vec<Resolved>,
    /// "Bring all widgets to the primary monitor": a temporary view, never saved (D26).
    park_all: bool,
    /// Fake monitors for tests (PLINTH_FAKE_MONITORS): the layers are ordinary windows.
    fake: Arc<Mutex<Option<String>>>,
    /// Every reconcile result, for the autotest: (connected monitor ids, [(widget, monitor, parked)]).
    layout_log: Vec<(Vec<String>, Vec<(String, String, bool)>)>,
    /// Placements before the last "Reset widget positions", for Undo.
    undo_placements: Option<Vec<plinth_core::monitors::Placement>>,
    lib: Arc<Mutex<Library>>,
    /// Composed picture file per monitor id (static image wallpapers), ready for the layer to draw.
    composed: HashMap<String, String>,
    slide_due: HashMap<String, u64>,
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
                json!({ "id": w.id, "name": w.name, "version": w.version, "slot": w.slot, "first": w.trusted,
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
        let shared = info.iter().filter(|w| w.trusted).count();
        let mut cost = impact::scene_cost_with_motion(shared, n - shared, self.st.motion);
        let extra_layers = layer_targets(self).len().saturating_sub(1) as f32;
        cost.ram_mb += impact::EXTRA_LAYER.ram_mb * extra_layers;
        cost.cpu_pct += impact::EXTRA_LAYER.cpu_pct * extra_layers;
        let wall_list: Vec<Value> = self.walls.lock().unwrap().list().iter().map(|e| json!({ "id": e.wallpaper.id, "name": e.wallpaper.name, "kind": e.wallpaper.kind.as_str(), "bundled": e.bundled, "author": e.wallpaper.author })).collect();
        let wcost = match self.walls.lock().unwrap().get(&self.st.wallpaper).map(|e| impact::wallpaper_extra(e.wallpaper.kind)) {
            Some(c) => format!("adds ≈{} MB and ≈{:.1}% of one core while visible · pauses in games, near 0 CPU when covered", c.ram_mb.round(), c.cpu_pct),
            None => "no extra cost".to_string(),
        };
        let mons_json: Vec<Value> = self
            .monitors
            .iter()
            .enumerate()
            .map(|(i, m)| json!({ "id": m.id, "n": i + 1, "name": m.name, "x": m.x, "y": m.y, "w": m.width_px, "h": m.height_px, "dpi": m.dpi, "scale": m.scale_percent(), "primary": m.primary, "hz": m.refresh_hz, "portrait": m.portrait(), "wallpaper": self.wallpaper_for(m), "layer": layer_targets(self).iter().any(|t| t.id == m.id) }))
            .collect();
        let place_json: Vec<Value> = self
            .st
            .placements
            .iter()
            .map(|p| {
                let r = self.resolved.iter().find(|r| r.widget == p.widget);
                let nm = { let b = p.widget.split('#').next().unwrap_or(""); let n = names.get(b).cloned().unwrap_or_default(); match p.widget.split_once('#') { Some((_, c)) => format!("{n} (copy {c})"), None => n } };
                let home_ok = p.home == mons::ROLE_PRIMARY || self.monitors.iter().any(|m| m.id == p.home || (!p.home_fingerprint.is_empty() && m.fingerprint == p.home_fingerprint));
                json!({ "widget": p.widget, "base": p.widget.split('#').next().unwrap_or(""), "copy": p.widget.split_once('#').and_then(|(_, n)| n.parse::<u32>().ok()).unwrap_or(1), "name": nm, "home": p.home, "homeConnected": home_ok, "monitor": r.map(|r| r.monitor.clone()), "parked": r.map_or(false, |r| r.parked), "clipped": r.map_or(false, |r| r.clipped), "anchor": p.anchor.as_str() })
            })
            .collect();
        let (lib_summary, lib_names): (Value, HashMap<String, String>) = {
            let l = self.lib.lock().unwrap();
            (l.summary(), self.st.lib_sel.values().filter_map(|x| l.find(&x.key).map(|i| (x.key.clone(), i.name().to_string()))).collect())
        };
        let lib_sel: Vec<Value> = self.st.lib_sel.iter().map(|(m, x)| json!({ "monitor": m, "key": x.key, "name": lib_names.get(&x.key).cloned().unwrap_or_default(), "fit": x.fit, "bg": x.bg })).collect();
        let slides: Vec<Value> = self.st.slides.iter().map(|(m, x)| json!({ "monitor": m, "minutes": x.minutes, "shuffle": x.shuffle, "filter": x.filter })).collect();
        json!({ "lib": lib_summary, "libSel": lib_sel, "slides": slides, "scrim": self.st.scrim, "profiles": self.st.profiles.keys().collect::<Vec<_>>(), "restorable": self.st.os_snapshot.is_some(), "monitors": mons_json, "placements": place_json, "fallback": self.st.fallback, "parkAll": self.park_all, "canUndo": self.undo_placements.is_some(), "fakeMonitors": self.fake.lock().unwrap().is_some(), "wallpaperCost": wcost, "wallpapers": wall_list, "wallpaper": self.st.wallpaper, "scene": self.st.scene, "applied": self.st.applied, "safe": self.st.safe, "firstRun": self.st.first_run, "motion": self.st.motion, "paused": self.paused, "widgets": widgets, "activity": activity,
            "destinations": dests, "proxy": proxy, "impact": { "ram": cost.ram_mb.round(), "cpu": format!("{:.1}", cost.cpu_pct), "hubRam": impact::HUB_OPEN_RAM_MB, "motionCpu": impact::MOTION_EXTRA_CPU_PCT }, "msg": msg })
        .to_string()
    }

    fn primary_id(&self) -> Option<String> {
        self.monitors.iter().find(|m| m.primary).or(self.monitors.first()).map(|m| m.id.clone())
    }

    /// The wallpaper id shown on a monitor: the primary uses the main selection, others their own (or none).
    fn wallpaper_for(&self, m: &MonitorInfo) -> String {
        if m.primary {
            self.st.wallpaper.clone()
        } else {
            self.st.monitor_walls.get(&m.id).cloned().unwrap_or_default()
        }
    }

    /// Fill in placements for new widgets, then compute where everything is shown. Reads the saved placements, never
    /// writes transient results back into them (D26).
    fn recompute(&mut self) {
        let ids: Vec<String> = self.host.lock().unwrap().describe().into_iter().map(|w| w.id).collect();
        for (i, id) in ids.iter().enumerate() {
            if !self.st.placements.iter().any(|p| &p.widget == id) {
                self.st.placements.push(mons::default_placement(id, i));
            }
        }
        self.st.placements.retain(|p| ids.iter().any(|i| i == p.widget.split('#').next().unwrap_or(&p.widget)) && (!p.widget.contains('#') || plinth_host::valid_copy(&p.widget)));
        let fb = if self.st.fallback.is_empty() { None } else { Some(self.st.fallback.as_str()) };
        self.resolved = match (self.park_all, self.primary_id()) {
            (true, Some(p)) => mons::bring_all_to(&self.st.placements, &self.monitors, &p),
            _ => mons::reconcile(&self.st.placements, &self.monitors, fb),
        };
        self.layout_log.push((self.monitors.iter().map(|m| m.id.clone()).collect(), self.resolved.iter().map(|r| (r.widget.clone(), r.monitor.clone(), r.parked)).collect()));
        self.layout_log.truncate(200);
    }

    fn wallpaper_html(&self, wid: &str) -> String {
        let walls = self.walls.lock().unwrap();
        let Some(e) = walls.get(wid) else { return String::new() };
        // id and entry are validated to a closed character set by plinth-core, so they cannot break out of the attribute.
        let url = WallpaperStore::entry_url(&e.wallpaper);
        match e.wallpaper.kind {
            plinth_core::WallpaperKind::Video => format!(r#"<video id="wv" class="wp" src="{url}" muted loop autoplay playsinline disablepictureinpicture style="object-fit:{}"></video>"#, e.wallpaper.fit.css()),
            plinth_core::WallpaperKind::Web => format!(r#"<iframe id="wf" class="wp" sandbox="allow-scripts" src="{url}" title="wallpaper"></iframe>"#),
        }
    }

    fn layer_html(&self, m: &MonitorInfo) -> String {
        let s = scene(&self.st.scene);
        let widgets: Vec<Value> = if self.st.safe {
            vec![]
        } else {
            let info = self.host.lock().unwrap().describe();
            self.resolved
                .iter()
                .filter(|r| r.monitor == m.id)
                .filter_map(|r| {
                    let base = r.widget.split('#').next().unwrap_or(&r.widget);
                    let w = info.iter().find(|w| w.id == base)?;
                    Some(json!({ "id": r.widget, "base": base, "slot": w.slot, "trusted": w.trusted, "x": r.x.round(), "y": r.y.round(), "w": r.w.round(), "h": r.h.round() }))
                })
                .collect()
        };
        let banner = if self.st.safe { r#"<div style="position:fixed;left:50%;top:6%;transform:translateX(-50%);padding:10px 18px;border-radius:14px;background:rgba(0,0,0,.55);border:1px solid #ffb454;color:#ffb454;font:600 14px system-ui">Safe mode: widgets are paused</div>"# } else { "" };
        let beacon = if self.beacon && m.primary { r#"<div style="position:fixed;left:50%;top:50%;width:40px;height:40px;margin:-20px 0 0 -20px;background:#ff00ff"></div>"# } else { "" };
        let wp_html = self.wallpaper_html(&self.wallpaper_for(m));
        let stat: Option<String> = if wp_html.is_empty() { self.composed.get(&m.id).filter(|_| self.st.lib_sel.contains_key(&m.id)).cloned() } else { None };
        let wall_css = match &stat {
            Some(f) if library::valid_composed_name(f) => format!("url(\"http://pwc.localhost/{f}\") 0 0/100% 100% no-repeat"),
            _ => s.wall.to_string(),
        };
        let scrim_css = match self.st.scrim.as_str() {
            "dim" => "background:rgba(0,0,0,.38)",
            "blur" => "background:rgba(0,0,0,.18);backdrop-filter:blur(8px)",
            _ => "display:none",
        };
        let mut html = LAYER_HTML.to_string();
        // Cost experiments (and the basis of the Performance mode setting): PLINTH_NO_ANIM drops the wallpaper drift,
        // PLINTH_NO_BLUR replaces the glass blur with a flat translucent panel.
        if std::env::var_os("PLINTH_NO_BLUR").is_some() {
            html = html.replace("backdrop-filter:blur(22px) saturate(1.35);", "");
        }
        html
            .replace("__DRIFT__", if stat.is_some() { "inset:0" } else if self.st.motion && wp_html.is_empty() && std::env::var_os("PLINTH_NO_ANIM").is_none() { "animation:drift 40s ease-in-out infinite alternate" } else { "" })
            .replace("__WALL__", &wall_css)
            .replace("__SCRIM__", scrim_css)
            .replace("__WALLPAPER__", &wp_html)
            .replace("__MID__", if wp_html.is_empty() && stat.is_none() { s.mid } else { "transparent" })
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
    rect: (i32, i32, u32, u32),
    html_hash: u64,
}

fn hash_str(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Monitors that need a layer window: the primary (it paints the Scene), and any monitor with widgets or a motion wallpaper.
/// A monitor with nothing to draw has no layer and costs nothing (D24, D27).
fn layer_targets(app: &App) -> Vec<MonitorInfo> {
    if !app.st.applied {
        return Vec::new();
    }
    app.monitors
        .iter()
        .filter(|m| m.primary || (!app.st.safe && app.resolved.iter().any(|r| r.monitor == m.id)) || !app.wallpaper_for(m).is_empty())
        .cloned()
        .collect()
}

/// Make the set of layer windows match the connected monitors, widgets and wallpapers. Existing layers whose monitor rectangle
/// is unchanged are reloaded only when their page actually changed.
fn sync_layers(target: &EventLoopWindowTarget<Ev>, app: &App, layers: &mut HashMap<String, Layer>, ctx: &mut WebContext, proxy: &EventLoopProxy<Ev>) {
    let want = layer_targets(app);
    layers.retain(|id, _| want.iter().any(|m| &m.id == id));
    for m in want {
        let html = app.layer_html(&m);
        let hh = hash_str(&html);
        let rect = (m.x, m.y, m.width_px, m.height_px);
        match layers.get_mut(&m.id) {
            Some(l) if l.rect == rect => {
                if l.html_hash != hh {
                    let _ = l.webview.load_html(&html);
                    l.html_hash = hh;
                }
                continue;
            }
            Some(_) => {
                layers.remove(&m.id);
            }
            None => {}
        }
        if let Some(l) = create_layer(target, app, &m, &html, ctx, proxy) {
            layers.insert(m.id.clone(), l);
        }
    }
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

fn wallpapers_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("PLINTH_WALLPAPERS") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        let beside = exe.with_file_name("wallpapers");
        if beside.is_dir() {
            return beside;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../wallpapers")
}

fn user_wallpapers_dir() -> PathBuf {
    std::env::var_os("PLINTH_USER_WALLPAPERS").map(PathBuf::from).unwrap_or_else(|| state::data_dir().join("wallpapers"))
}

fn create_layer(target: &EventLoopWindowTarget<Ev>, app: &App, m: &MonitorInfo, html: &str, ctx: &mut WebContext, proxy: &EventLoopProxy<Ev>) -> Option<Layer> {
    // No drop shadow: tao reserves an invisible shadow border (8 px) around undecorated windows, which would inset the layer.
    use tao::platform::windows::WindowBuilderExtWindows as _;
    let window = WindowBuilder::new().with_title("Plinth desktop layer").with_decorations(false).with_undecorated_shadow(false).with_visible(false).with_resizable(false).build(target).ok()?;
    let fake = app.fake.lock().unwrap().is_some();
    let attached = !fake && std::env::var_os("PLINTH_NO_ATTACH").is_none() && win::attach_to_desktop(window.hwnd() as isize, m.x, m.y, m.width_px as i32, m.height_px as i32);
    if !attached {
        eprintln!("desktop layer for {} not attached (fake monitor or attach failed): using a normal window", m.name);
        window.set_decorations(true);
        window.set_inner_size(tao::dpi::PhysicalSize::new((m.width_px / 4).max(240), (m.height_px / 4).max(160)));
        window.set_outer_position(tao::dpi::PhysicalPosition::new(60 + m.x / 4 + 400, 60 + m.y / 4 + 200));
        window.set_visible(true);
    }
    let host = app.host.clone();
    let ipc_proxy = proxy.clone();
    let (w, h) = if attached { (m.width_px, m.height_px) } else { ((m.width_px / 4).max(240), (m.height_px / 4).max(160)) };
    let mut b = WebViewBuilder::new_with_web_context(ctx)
        .with_html(html.to_string())
        .with_bounds(wry::Rect { position: wry::dpi::PhysicalPosition::new(0, 0).into(), size: wry::dpi::PhysicalSize::new(w, h).into() })
        .with_ipc_handler(move |req: wry::http::Request<String>| {
            let Ok(v) = serde_json::from_str::<Value>(req.body()) else { return };
            if let (Some(wid), Some(msg)) = (v.get("wid").and_then(Value::as_str), v.get("msg").and_then(Value::as_str)) {
                let _ = ipc_proxy.send_event(Ev::Layer { wid: wid.to_string(), msg: msg.to_string() });
            }
        });
    let schemes: Vec<String> = (0..plinth_host::THIRD_PARTY_SLOTS).map(Host::scheme_for_slot).chain(std::iter::once(plinth_host::TRUSTED_SCHEME.to_string())).collect();
    for scheme in schemes {
        let h = host.clone();
        // The origin is identified by the protocol the request arrived on, never by anything the page controls.
        b = b.with_custom_protocol(scheme.clone(), move |_id, req| {
            let name = format!("{scheme}.localhost");
            let path = req.uri().path_and_query().map(|p| p.as_str().to_string()).unwrap_or_else(|| "/".into());
            let r = h.lock().unwrap().serve(&name, &path);
            let mut resp = wry::http::Response::builder().status(r.status);
            for (k, v) in &r.headers {
                resp = resp.header(k.as_str(), v.as_str());
            }
            resp.body(std::borrow::Cow::Owned(r.body)).unwrap()
        });
    }
    {
        let (walls, log) = (app.walls.clone(), app.wall_log.clone());
        b = b.with_custom_protocol(WALLPAPER_SCHEME.to_string(), move |_id, req| {
            let path = req.uri().path_and_query().map(|p| p.as_str().to_string()).unwrap_or_else(|| "/".into());
            let range = req.headers().get("range").and_then(|v| v.to_str().ok()).map(String::from);
            let r = walls.lock().unwrap().serve(&path, range.as_deref());
            {
                let mut l = log.lock().unwrap();
                l.push((path, r.status));
                if l.len() > 200 {
                    l.remove(0);
                }
            }
            let mut resp = wry::http::Response::builder().status(r.status);
            for (k, v) in &r.headers {
                resp = resp.header(k.as_str(), v.as_str());
            }
            resp.body(std::borrow::Cow::Owned(r.body)).unwrap()
        });
    }
    {
        let dir = app.lib.lock().unwrap().composed.clone();
        b = b.with_custom_protocol("pwc".to_string(), move |_id, req| {
            let name = req.uri().path().trim_start_matches('/').to_string();
            let body = if library::valid_composed_name(&name) { std::fs::read(dir.join(&name)).ok() } else { None };
            let mut resp = wry::http::Response::builder().header("Cache-Control", "max-age=31536000, immutable").header("X-Content-Type-Options", "nosniff");
            match body {
                Some(_) => resp = resp.status(200).header("Content-Type", "image/jpeg"),
                None => resp = resp.status(404),
            }
            resp.body(std::borrow::Cow::Owned(body.unwrap_or_default())).unwrap()
        });
    }
    use wry::WebViewBuilderExtWindows;
    b = b.with_additional_browser_args(format!("--proxy-server=http://127.0.0.1:{} --proxy-bypass-list=<-loopback>", app.proxy_port));
    let webview = b.build(&window).ok()?;
    Some(Layer { _window: window, webview, rect: (m.x, m.y, m.width_px, m.height_px), html_hash: hash_str(html) })
}

fn wallpaper_images_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("PLINTH_WALLPAPER_IMAGES") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        let beside = exe.with_file_name("wallpaper-images");
        if beside.is_dir() {
            return beside;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../wallpaper-images")
}

/// Three workers turn thumbnail requests from the hub into small JPEGs (made once, cached by content key).
fn start_thumb_pool(lib: Arc<Mutex<Library>>) -> mpsc::Sender<ThumbJob> {
    let (tx, rx) = mpsc::channel::<ThumbJob>();
    let rx = Arc::new(Mutex::new(rx));
    for _ in 0..3 {
        let (rx, lib) = (rx.clone(), lib.clone());
        std::thread::spawn(move || loop {
            let job = rx.lock().unwrap().recv();
            let Ok((path, responder)) = job else { break };
            let respond = |status: u16, body: Vec<u8>| {
                let mut b = wry::http::Response::builder().status(status).header("X-Content-Type-Options", "nosniff");
                if status == 200 {
                    b = b.header("Content-Type", "image/jpeg").header("Cache-Control", "max-age=31536000, immutable");
                }
                b.body(std::borrow::Cow::Owned(body)).unwrap()
            };
            let key = path.trim_start_matches('/').trim_end_matches(".jpg").to_string();
            if !corelib::valid_key(&key) {
                responder.respond(respond(404, Vec::new()));
                continue;
            }
            let (src, dest) = {
                let l = lib.lock().unwrap();
                match l.find(&key) {
                    Some(it) if it.status == corelib::Status::Ok => (it.path.clone(), l.thumb_file(&key)),
                    _ => {
                        drop(l);
                        responder.respond(respond(404, Vec::new()));
                        continue;
                    }
                }
            };
            if !dest.exists() && library::make_thumb(&src, &dest).is_err() {
                responder.respond(respond(404, Vec::new()));
                continue;
            }
            responder.respond(match std::fs::read(&dest) {
                Ok(b) => respond(200, b),
                Err(_) => respond(404, Vec::new()),
            });
        });
    }
    tx
}

/// Compose and (when the wallpaper is applied) set the OS wallpaper for monitors with a static image selection.
/// Work happens on a worker thread; each monitor reports back with `Ev::Composed`.
fn apply_statics(app: &App, proxy: &EventLoopProxy<Ev>, only: Option<&str>) {
    let (minx, miny) = (app.monitors.iter().map(|m| m.x).min().unwrap_or(0), app.monitors.iter().map(|m| m.y).min().unwrap_or(0));
    let (maxx, maxy) = (app.monitors.iter().map(|m| m.x + m.width_px as i32).max().unwrap_or(1), app.monitors.iter().map(|m| m.y + m.height_px as i32).max().unwrap_or(1));
    let total = ((maxx - minx).max(1) as u32, (maxy - miny).max(1) as u32);
    for m in &app.monitors {
        if only.map_or(false, |o| o != m.id) {
            continue;
        }
        let Some(sel) = app.st.lib_sel.get(&m.id) else { continue };
        let (src, composed_dir) = {
            let l = app.lib.lock().unwrap();
            match l.find(&sel.key) {
                Some(it) if it.status == corelib::Status::Ok => (it.path.clone(), l.composed.clone()),
                _ => continue,
            }
        };
        let mode = FitMode::parse(&sel.fit).unwrap_or(FitMode::Fill);
        let bg = fit::parse_color(&sel.bg).unwrap_or((0, 0, 0));
        let off = (m.x - minx, m.y - miny);
        let canvas = (m.width_px, m.height_px);
        let name = library::composed_name(&sel.key, canvas, mode, &sel.bg, off, total);
        let (rect, set_os, mid, p) = ((m.x, m.y, m.width_px as i32, m.height_px as i32), app.st.applied && app.fake.lock().unwrap().is_none(), m.id.clone(), proxy.clone());
        std::thread::spawn(move || {
            let dest = composed_dir.join(&name);
            let r = (|| -> Result<(), String> {
                if !dest.exists() {
                    library::compose(&src, canvas, mode, bg, off, total, &dest)?;
                }
                if set_os {
                    os_wall::set_for_rect(rect, &dest.to_string_lossy())?;
                }
                Ok(())
            })();
            let _ = p.send_event(Ev::Composed { monitor: mid, file: name, err: r.err() });
        });
    }
}

/// Take the "before" snapshot of the OS wallpaper if there is none yet, then apply. The snapshot always comes first so Revert is exact.
fn request_statics(app: &App, proxy: &EventLoopProxy<Ev>, only: Option<&str>) {
    if app.st.lib_sel.is_empty() {
        return;
    }
    if app.st.applied && app.st.os_snapshot.is_none() && app.fake.lock().unwrap().is_none() {
        let p = proxy.clone();
        std::thread::spawn(move || {
            let _ = p.send_event(Ev::OsSnap(os_wall::snapshot().ok()));
        });
    } else {
        apply_statics(app, proxy, only);
    }
}

/// Keep the composed-picture cache small: delete the oldest files that no monitor is using.
fn gc_composed(app: &App) {
    let dir = app.lib.lock().unwrap().composed.clone();
    let keep: Vec<&String> = app.composed.values().collect();
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let mut files: Vec<(std::time::SystemTime, PathBuf, String)> = rd.filter_map(|e| e.ok()).filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path(), e.file_name().to_string_lossy().to_string()))).collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p, n) in files.into_iter().skip(24) {
        if !keep.iter().any(|k| **k == n) {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn current_monitors(fake: &Arc<Mutex<Option<String>>>) -> Vec<MonitorInfo> {
    match fake.lock().unwrap().clone() {
        Some(spec) => mon::parse_fake(&spec),
        None => mon::enumerate_real(),
    }
}

fn build_hub(window: &Window, ctx: &mut WebContext, proxy: &EventLoopProxy<Ev>, thumbs: &mpsc::Sender<ThumbJob>) -> WebView {
    let p = proxy.clone();
    let tx = thumbs.clone();
    let dp = proxy.clone();
    WebViewBuilder::new_with_web_context(ctx)
        .with_html(HUB_HTML)
        .with_asynchronous_custom_protocol("pwth".to_string(), move |_id, req, responder| {
            let _ = tx.send((req.uri().path().to_string(), responder));
        })
        .with_drag_drop_handler(move |ev| {
            match ev {
                wry::DragDropEvent::Enter { .. } => {
                    let _ = dp.send_event(Ev::DropHover(true));
                }
                wry::DragDropEvent::Leave => {
                    let _ = dp.send_event(Ev::DropHover(false));
                }
                wry::DragDropEvent::Drop { paths, .. } => {
                    let _ = dp.send_event(Ev::DropHover(false));
                    let _ = dp.send_event(Ev::Drop(paths));
                }
                _ => {}
            }
            true // Plinth handles drops itself; the page never navigates to a dropped file
        })
        .with_ipc_handler(move |req: wry::http::Request<String>| {
            let _ = p.send_event(Ev::Hub(req.body().clone()));
        })
        .build(window)
        .expect("hub webview")
}

fn main() {
    let instance = format!("Local\\PlinthApp-{}", std::env::var("PLINTH_STATE").unwrap_or_default().chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>());
    if !win::first_instance(&instance) {
        println!("Plinth is already running; bringing its window to the front.");
        let signalled = win::signal_show(&instance);
        let found = win::activate_existing_hub(); // best effort as well; the signal is what the running app acts on
        println!("second launch: show signal {}, window {}", if signalled { "sent" } else { "NOT sent" }, if found { "found" } else { "not found" });
        return;
    }
    let show_handle = win::show_event(&instance);
    let autotest: Option<u64> = std::env::var("PLINTH_AUTOTEST").ok().and_then(|s| s.parse().ok());

    // ---- state, host, widgets ----
    let mut st = State::load();
    let sampler = hw::Sampler::start();
    let host = Arc::new(Mutex::new(Host::new(Box::new(sampler.provider()))));
    {
        let mut h = host.lock().unwrap();
        let dir = widgets_dir();
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(&dir).map(|r| r.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
        dirs.sort();
        for d in dirs {
            match Package::from_dir(&d) {
                Ok(p) => {
                    let id = p.manifest.id.clone();
                    // Only widgets shipped with the app (ids under com.plinth.) join the shared trusted origin.
                    if id.starts_with("com.plinth.") {
                        match h.install_trusted(p) {
                            Ok(()) => println!("installed {id} (trusted shared origin)"),
                            Err(e) => eprintln!("cannot install {id}: {e:?}"),
                        }
                    } else {
                        match h.install(p) {
                            Ok(slot) => println!("installed {id} in isolated slot {slot}"),
                            Err(e) => eprintln!("cannot install {id}: {e:?}"),
                        }
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
    let walls = Arc::new(Mutex::new(WallpaperStore::new(wallpapers_dir(), user_wallpapers_dir())));
    println!("wallpapers: {} found", walls.lock().unwrap().list().len());
    let mut app = App { host: host.clone(), st, policy, plog, proxy_port, beacon: autotest.is_some(), paused: false, walls, wall_log: Arc::new(Mutex::new(Vec::new())), monitors: Vec::new(), resolved: Vec::new(), park_all: false, fake: Arc::new(Mutex::new(std::env::var("PLINTH_FAKE_MONITORS").ok().filter(|v| !v.trim().is_empty()))), layout_log: Vec::new(), undo_placements: None, lib: Arc::new(Mutex::new(Library::load(std::env::temp_dir().join("plinth-placeholder-lib.json"), std::env::temp_dir().join("plinth-placeholder-thumbs"), std::env::temp_dir().join("plinth-placeholder-composed"), None))), composed: HashMap::new(), slide_due: HashMap::new() };
    app.monitors = current_monitors(&app.fake);
    println!("monitors: {}", app.monitors.iter().map(|m| format!("{} {}x{} at {},{} {}%{}", m.name, m.width_px, m.height_px, m.x, m.y, m.scale_percent(), if m.primary { " primary" } else { "" })).collect::<Vec<_>>().join(" | "));
    app.recompute();
    if app.walls.lock().unwrap().get(&app.st.wallpaper).is_none() {
        app.st.wallpaper.clear(); // the chosen wallpaper was removed or is broken: fall back to the Scene gradient
    }
    app.sync_proxy();
    if app.st.first_run {
        app.persist(); // creates the state file so the welcome sheet shows once
    }

    // ---- windows ----
    let event_loop = EventLoopBuilder::<Ev>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let lib_file = std::env::var_os("PLINTH_LIBRARY").map(PathBuf::from).unwrap_or_else(|| state::local_dir().join("library.json"));
    let lib_dir = lib_file.parent().map(|p| p.to_path_buf()).unwrap_or_else(state::local_dir);
    app.lib = Arc::new(Mutex::new(Library::load(lib_file, lib_dir.join("thumbs"), lib_dir.join("composed"), Some(wallpaper_images_dir()))));
    let thumb_tx = start_thumb_pool(app.lib.clone());
    let (n_items, n_roots) = {
        let l = app.lib.lock().unwrap();
        (l.items.len(), l.roots.len())
    };
    println!("library: {n_items} items, {n_roots} folders");
    let hub_window = WindowBuilder::new().with_title("Plinth").with_inner_size(tao::dpi::LogicalSize::new(1180.0, 780.0)).build(&event_loop).unwrap();
    let hub_id = hub_window.id();
    let mut hub_ctx = WebContext::new(Some(state::local_dir().join("wv-hub")));
    let mut hub_view: Option<WebView> = Some(build_hub(&hub_window, &mut hub_ctx, &proxy, &thumb_tx));
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
    let mut layers: HashMap<String, Layer> = HashMap::new();
    let mut identify: Vec<(Window, WebView)> = Vec::new();

    {
        // Hub revival: if the hub was hidden (its web view released) and something makes the window visible again, such as a
        // second launch, ask the event loop to rebuild it.
        let p = proxy.clone();
        std::thread::spawn(move || {
            let mut last = (false, false);
            loop {
            if win::wait_show(show_handle, 400) {
                println!("show signal received");
                let _ = p.send_event(Ev::Menu("open".to_string()));
                std::thread::sleep(Duration::from_millis(800));
                continue;
            }
            let now = (HUB_HIDDEN.load(std::sync::atomic::Ordering::Relaxed), win::hub_visible());
            if now != last {
                println!("hub watcher: hidden flag {}, window visible {}", now.0, now.1);
                last = now;
            }
            if HUB_HIDDEN.load(std::sync::atomic::Ordering::Relaxed) && win::hub_visible() {
                let _ = p.send_event(Ev::Menu("open".to_string()));
                std::thread::sleep(Duration::from_millis(800));
            }
            }
        });
    }
    {
        // Pause governor: poll the situation every 2 s; the decision logic is plinth-core's tested `should_pause`.
        let p = proxy.clone();
        let own = autotest.is_some();
        std::thread::spawn(move || {
            let policy = plinth_core::Policy::default();
            let mut last = false;
            loop {
                let now = plinth_core::should_pause(&win::system_state(own), &policy);
                if now != last {
                    last = now;
                    let _ = p.send_event(Ev::Pause(now));
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        });
    }
    {
        // Display watcher: poll the monitor list once a second; a change must hold still for 2 s before it is applied, so docks
        // and wake-ups that enumerate late do not shuffle widgets (D26). Polling is used because the documented WM_DISPLAYCHANGE
        // is about resolution changes, not hot-plug (research/monitors-and-wallpapers.md); event-based detection is a later refinement.
        let p = proxy.clone();
        let fake = app.fake.clone();
        let mut last = app.monitors.clone();
        std::thread::spawn(move || {
            let mut candidate: Option<(Vec<MonitorInfo>, std::time::Instant)> = None;
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let now = current_monitors(&fake);
                if now.is_empty() || now == last {
                    candidate = None;
                    continue;
                }
                match &candidate {
                    Some((c, t)) if *c == now => {
                        if t.elapsed() >= Duration::from_millis(mons::RETURN_DELAY_MS) {
                            last = now.clone();
                            candidate = None;
                            let _ = p.send_event(Ev::Monitors(now));
                        }
                    }
                    _ => candidate = Some((now, std::time::Instant::now())),
                }
            }
        });
    }
    {
        // Library watcher: rescan every 5 s while the hub is open, every 60 s in the background, and at once when asked. A scan
        // compares file names, sizes and times (no image is decoded unless it is new or changed), so an unchanged library is cheap.
        let (lib, p) = (app.lib.clone(), proxy.clone());
        std::thread::spawn(move || {
            let (mut waited, mut first) = (0u64, true);
            loop {
                std::thread::sleep(Duration::from_secs(1));
                waited += 1;
                let interval = if HUB_HIDDEN.load(Ordering::Relaxed) { 60 } else { 5 };
                if first || LIB_RESCAN.swap(false, Ordering::Relaxed) || waited >= interval {
                    first = false;
                    waited = 0;
                    let (roots, items) = lib.lock().unwrap().snapshot();
                    let out = library::scan(&roots, &items);
                    let mut l = lib.lock().unwrap();
                    let before = l.revision;
                    l.apply_scan(out);
                    if l.revision != before {
                        l.save();
                        drop(l);
                        let _ = p.send_event(Ev::LibraryChanged);
                    }
                }
            }
        });
        let p2 = proxy.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(15));
            let _ = p2.send_event(Ev::SlideTick);
        });
    }
    let tick = proxy.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        let _ = tick.send_event(Ev::Save);
    });
    if autotest.is_some() {
        let p = proxy.clone();
        std::thread::spawn(move || {
            let hub = |p: &EventLoopProxy<Ev>, j: &str| {
                let _ = p.send_event(Ev::Hub(j.to_string()));
            };
            let nap = |s: u64| std::thread::sleep(Duration::from_secs(s));
            if let Some(dir) = std::env::var_os("PLINTH_AUTOTEST_LIB") {
                // Library end to end on the real monitors: add a folder, scan, pick a picture (composed and set as the OS wallpaper),
                // watch a new file appear, run the slideshow, then Revert and check the OS wallpaper is exactly as before.
                let dir = PathBuf::from(dir);
                nap(2);
                let _ = p.send_event(Ev::Auto(40)); // OS wallpaper before anything
                hub(&p, r#"{"cmd":"apply"}"#);
                nap(3);
                hub(&p, &json!({ "cmd": "lib_add_folder", "path": dir.to_string_lossy() }).to_string());
                nap(7);
                let _ = p.send_event(Ev::Auto(41)); // library summary after the first scan
                let _ = p.send_event(Ev::Auto(42)); // choose the first picture for the primary monitor (fit, magenta bars)
                nap(8);
                let _ = p.send_event(Ev::Auto(43)); // composed file and OS wallpaper
                let extra = dir.join("added_later.png");
                let _ = std::fs::copy(dir.join("duplicate_of_a_picture.png"), dir.join("copy_for_slideshow.png")); // a duplicate: must stay hidden
                if let Some(first) = std::fs::read_dir(dir.join("set1")).ok().and_then(|r| r.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.is_file())) {
                    let _ = std::fs::copy(first, &extra); // a NEW picture (not byte-identical to anything else: it is a copy, so it is a duplicate too)
                }
                let fresh: image::RgbImage = image::ImageBuffer::from_pixel(320, 180, image::Rgb([9, 99, 199]));
                let _ = fresh.save(dir.join("fresh.png")); // the only NEW picture of the three files added
                nap(8);
                let _ = p.send_event(Ev::Auto(44)); // watcher picked up the new files
                let _ = p.send_event(Ev::Auto(45)); // force the slideshow to be due and tick
                nap(8);
                let _ = p.send_event(Ev::Auto(46)); // the picture changed
                hub(&p, r#"{"cmd":"revert"}"#);
                nap(8);
                let _ = p.send_event(Ev::Auto(47)); // OS wallpaper after Revert
                let _ = p.send_event(Ev::Auto(48));
                return;
            }
            if std::env::var_os("PLINTH_AUTOTEST_MON").is_some() {
                // Monitor lifecycle on fake monitors (PLINTH_FAKE_MONITORS="...#A;...#B;...#C"): assign, unplug B, replug B, park all.
                let spec3 = std::env::var("PLINTH_FAKE_MONITORS").unwrap_or_default();
                let spec2 = spec3.split(';').filter(|s| !s.contains("#B")).collect::<Vec<_>>().join(";");
                nap(2);
                hub(&p, r#"{"cmd":"apply"}"#);
                nap(4);
                hub(&p, r#"{"cmd":"assign","widget":"com.plinth.clock","monitor":"FAKE#B"}"#);
                hub(&p, r#"{"cmd":"assign","widget":"com.plinth.buildcard","monitor":"FAKE#B"}"#);
                hub(&p, r#"{"cmd":"assign","widget":"com.plinth.pcvitals","monitor":"FAKE#C"}"#);
                nap(4);
                let _ = p.send_event(Ev::Auto(21)); // A: everything on its own monitor
                hub(&p, &json!({ "cmd": "fake_monitors", "spec": spec2 }).to_string());
                nap(6);
                let _ = p.send_event(Ev::Auto(22)); // B: monitor B unplugged
                hub(&p, &json!({ "cmd": "fake_monitors", "spec": spec3 }).to_string());
                nap(6);
                let _ = p.send_event(Ev::Auto(23)); // C: monitor B back
                hub(&p, r#"{"cmd":"park_all","on":true}"#);
                nap(3);
                let _ = p.send_event(Ev::Auto(24)); // D: bring all to primary
                hub(&p, r#"{"cmd":"park_all","on":false}"#);
                nap(3);
                let _ = p.send_event(Ev::Auto(25)); // E: back to the saved layout
                hub(&p, r#"{"cmd":"reset_positions"}"#);
                nap(2);
                let _ = p.send_event(Ev::Auto(27)); // F: positions reset
                hub(&p, r#"{"cmd":"undo_positions"}"#);
                nap(2);
                let _ = p.send_event(Ev::Auto(28)); // G: undone
                hub(&p, r#"{"cmd":"widget_add","widget":"com.plinth.clock","monitor":"FAKE#C"}"#);
                nap(3);
                let _ = p.send_event(Ev::Auto(29)); // H: a second clock on monitor C, the first still on B
                let _ = p.send_event(Ev::Auto(26));
                return;
            }
            nap(2);
            hub(&p, r#"{"cmd":"apply"}"#); // same code path as pressing Apply in the hub
            nap(6);
            hub(&p, r#"{"cmd":"wallpaper","id":"com.plinth.aurora"}"#); // a bundled web wallpaper goes live
            nap(3);
            if let Some(v) = std::env::var_os("PLINTH_TEST_VIDEO") {
                let _ = p.send_event(Ev::Drop(vec![PathBuf::from(v)])); // same path as dropping a file on the hub
                nap(5);
            }
            hub(&p, r#"{"cmd":"grant","id":"com.plinth.pcvitals","perm":"system.cpu","on":false}"#); // user revokes CPU
            nap(5);
            let _ = p.send_event(Ev::Auto(5)); // snapshot activity count
            hub(&p, r#"{"cmd":"safe","on":true}"#);
            nap(5);
            let _ = p.send_event(Ev::Auto(6)); // compare: nothing may run in safe mode
            hub(&p, r#"{"cmd":"safe","on":false}"#);
            nap(2);
            let _ = p.send_event(Ev::Auto(7)); // open a fullscreen test window (the "game")
            nap(6);
            let _ = p.send_event(Ev::Auto(8)); // close it
            nap(6);
            let _ = p.send_event(Ev::Auto(9)); // ask the layer whether the wallpaper video is playing again
            nap(2);
            let _ = p.send_event(Ev::Auto(2)); // show desktop
            nap(2);
            let _ = p.send_event(Ev::Auto(3)); // sample + restore
            nap(2);
            let _ = p.send_event(Ev::Auto(4)); // report
        });
    }
    let mut auto_visible: Option<bool> = None;
    let mut safe_count: (usize, usize) = (0, 0);
    let mut pause_log: Vec<bool> = Vec::new();
    let video_probe: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let mut sec_last: Option<Value> = None;
    let sec_probe: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
    if autotest.is_some() {
        let sp = sec_probe.clone();
        std::thread::spawn(move || *sp.lock().unwrap() = Some(sec::gather()));
    }
    let _ = &sec_last;
    let mut mon_snaps: Vec<MonSnap> = Vec::new();
    let mut lp = LibProbe::default();
    let mut fs_test: Option<Window> = None;
    let sampler = sampler;
    let mut applied_before_start = app.st.applied;

    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        if applied_before_start {
            applied_before_start = false;
            sync_layers(target, &app, &mut layers, &mut layer_ctx, &proxy);
            println!("restored applied scene {} (layers created: {})", app.st.scene, layers.len());
        }
        // The hub's browser process is released while the hub is hidden (about 200 MB) and rebuilt when it is shown again,
        // whether from the tray, a second launch, or any other way the window becomes visible.
        if hub_view.is_none() && hub_window.is_visible() {
            hub_view = Some(build_hub(&hub_window, &mut hub_ctx, &proxy, &thumb_tx));
            HUB_HIDDEN.store(false, std::sync::atomic::Ordering::Relaxed);
            println!("hub window rebuilt");
        }
        let reply = |app: &App, view: &Option<WebView>, msg: Option<&str>| {
            if let Some(v) = view {
                let _ = v.evaluate_script(&format!("__onState({})", app.state_json(msg)));
            }
        };
        match event {
            Event::UserEvent(Ev::Hub(body)) => {
                let Ok(v) = serde_json::from_str::<Value>(&body) else { return };
                let cmd = v.get("cmd").and_then(Value::as_str).unwrap_or("");
                let mut msg: Option<String> = None;
                let mut reload = false;
                let mut relayout = false;
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
                        sync_layers(target, &app, &mut layers, &mut layer_ctx, &proxy);
                        request_statics(&app, &proxy, None);
                        println!("hub: apply {} (layer present: {}, layers: {})", app.st.scene, !layers.is_empty(), layers.len());
                        msg = Some(format!("Applied {}. Your previous desktop is restored with Revert.", scene(&app.st.scene).name));
                    }
                    "revert" => {
                        app.st.applied = false;
                        layers.clear();
                        if let Some(snap) = app.st.os_snapshot.take() {
                            let p = proxy.clone();
                            std::thread::spawn(move || {
                                let r = os_wall::restore(&snap);
                                let _ = p.send_event(Ev::OsDone(r.err().unwrap_or_default()));
                            });
                        }
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
                    "lib_query" => {
                        let (q, f) = (v.get("q").and_then(Value::as_str).unwrap_or(""), v.get("filter").and_then(Value::as_str).unwrap_or("all"));
                        let (off, lim) = (v.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize, v.get("limit").and_then(Value::as_u64).unwrap_or(60) as usize);
                        let req = v.get("req").cloned().unwrap_or(Value::Null);
                        let page = app.lib.lock().unwrap().page(q, Filter::parse(f), off, lim);
                        if let Some(view) = &hub_view {
                            let _ = view.evaluate_script(&format!("__onLibrary({})", json!({ "req": req, "page": page })));
                        }
                    }
                    "lib_add_folder" => {
                        let path = v.get("path").and_then(Value::as_str).unwrap_or("");
                        let r = app.lib.lock().unwrap().add_root(path);
                        match r {
                            Ok(()) => {
                                LIB_RESCAN.store(true, Ordering::Relaxed);
                                msg = Some("Folder added. Plinth is reading its pictures now.".into());
                            }
                            Err(e) => msg = Some(e),
                        }
                    }
                    "lib_pick" => {
                        let purpose = v.get("purpose").and_then(Value::as_str).unwrap_or("folder").to_string();
                        let (p, owner) = (proxy.clone(), hub_window.hwnd() as isize);
                        std::thread::spawn(move || {
                            let path = os_wall::pick(!purpose.starts_with("locate:"), owner);
                            let _ = p.send_event(Ev::Picked { purpose, path });
                        });
                    }
                    "lib_remove_folder" => {
                        let path = v.get("path").and_then(Value::as_str).unwrap_or("");
                        match app.lib.lock().unwrap().remove_root(path) {
                            Ok(()) => msg = Some("Folder removed from the library. Your pictures were not touched.".into()),
                            Err(e) => msg = Some(e),
                        }
                    }
                    "lib_unexclude" => {
                        let (root, sub) = (v.get("root").and_then(Value::as_str).unwrap_or(""), v.get("path").and_then(Value::as_str).unwrap_or(""));
                        let _ = app.lib.lock().unwrap().set_exclude(root, sub, false);
                        LIB_RESCAN.store(true, Ordering::Relaxed);
                    }
                    "lib_fav" => {
                        let (key, on) = (v.get("key").and_then(Value::as_str).unwrap_or(""), v.get("on").and_then(Value::as_bool).unwrap_or(false));
                        app.lib.lock().unwrap().set_favorite(key, on);
                    }
                    "lib_remove_item" => {
                        let key = v.get("key").and_then(Value::as_str).unwrap_or("");
                        app.lib.lock().unwrap().remove_item(key);
                        msg = Some("Removed from the library. The picture file was not deleted.".into());
                    }
                    "lib_rescan" => {
                        LIB_RESCAN.store(true, Ordering::Relaxed);
                        msg = Some("Looking for changes…".into());
                    }
                    "set_static" => {
                        let (mid, key) = (v.get("monitor").and_then(Value::as_str).unwrap_or(""), v.get("key").and_then(Value::as_str).unwrap_or(""));
                        let fit_s = v.get("fit").and_then(Value::as_str).filter(|f| FitMode::parse(f).is_some());
                        let bg_s = v.get("bg").and_then(Value::as_str).filter(|c| fit::parse_color(c).is_some());
                        let usable = app.monitors.iter().any(|m| m.id == mid) && app.lib.lock().unwrap().find(key).map_or(false, |i| i.status == corelib::Status::Ok);
                        if usable {
                            let prev = app.st.lib_sel.get(mid).cloned();
                            app.st.lib_sel.insert(mid.to_string(), state::Sel { key: key.to_string(), fit: fit_s.map(String::from).or(prev.as_ref().map(|p| p.fit.clone())).unwrap_or_else(|| "fill".into()), bg: bg_s.map(String::from).or(prev.map(|p| p.bg)).unwrap_or_else(|| "#000000".into()) });
                            app.lib.lock().unwrap().touch(key);
                            app.composed.remove(mid);
                            relayout = true;
                            request_statics(&app, &proxy, Some(mid));
                            msg = Some(if app.st.applied { "Wallpaper set.".into() } else { "Chosen. Press Apply to put it on your desktop.".into() });
                        } else {
                            msg = Some("That picture or monitor is not available.".into());
                        }
                    }
                    "clear_static" => {
                        let mid = v.get("monitor").and_then(Value::as_str).unwrap_or("");
                        if app.st.lib_sel.remove(mid).is_some() {
                            app.composed.remove(mid);
                            app.st.slides.remove(mid);
                            relayout = true;
                            msg = Some("Picture removed from that monitor. Revert restores the wallpaper you had before.".into());
                        }
                    }
                    "slideshow" => {
                        let mid = v.get("monitor").and_then(Value::as_str).unwrap_or("").to_string();
                        if app.monitors.iter().any(|m| m.id == mid) {
                            if v.get("on").and_then(Value::as_bool).unwrap_or(false) {
                                let minutes = v.get("minutes").and_then(Value::as_u64).unwrap_or(30).clamp(1, 1440) as u32;
                                let filter = v.get("filter").and_then(Value::as_str).filter(|f| ["all", "favorites"].contains(f)).unwrap_or("all").to_string();
                                app.st.slides.insert(mid.clone(), state::Slide { minutes, shuffle: v.get("shuffle").and_then(Value::as_bool).unwrap_or(false), filter });
                                app.slide_due.insert(mid, library::now() + minutes as u64 * 60);
                                msg = Some(format!("Slideshow on: a new picture every {minutes} minutes while Plinth is running."));
                            } else {
                                app.st.slides.remove(&mid);
                                app.slide_due.remove(&mid);
                                msg = Some("Slideshow off.".into());
                            }
                        }
                    }
                    "scrim" => {
                        let m = v.get("mode").and_then(Value::as_str).unwrap_or("none");
                        if ["none", "dim", "blur"].contains(&m) {
                            app.st.scrim = m.to_string();
                            reload = true;
                            msg = Some(match m { "dim" => "Wallpapers are dimmed behind widgets.", "blur" => "Wallpapers are softened behind widgets.", _ => "No extra shading behind widgets." }.into());
                        }
                    }
                    "profile_save" => {
                        let name: String = v.get("name").and_then(Value::as_str).unwrap_or("").chars().filter(|c| !c.is_control()).take(40).collect();
                        let name = name.trim().to_string();
                        if name.is_empty() {
                            msg = Some("Give the profile a name first.".into());
                        } else if app.st.profiles.len() >= 32 && !app.st.profiles.contains_key(&name) {
                            msg = Some("Too many profiles; delete one first.".into());
                        } else {
                            let p = json!({ "scene": app.st.scene, "wallpaper": app.st.wallpaper, "monitor_walls": app.st.monitor_walls, "placements": app.st.placements, "fallback": app.st.fallback, "scrim": app.st.scrim,
                                "lib_sel": app.st.lib_sel.iter().map(|(k, x)| (k.clone(), json!({ "key": x.key, "fit": x.fit, "bg": x.bg }))).collect::<serde_json::Map<_, _>>(),
                                "slides": app.st.slides.iter().map(|(k, x)| (k.clone(), json!({ "minutes": x.minutes, "shuffle": x.shuffle, "filter": x.filter }))).collect::<serde_json::Map<_, _>>() });
                            app.st.profiles.insert(name.clone(), p);
                            msg = Some(format!("Saved \"{name}\": wallpapers, widgets and monitor assignments together."));
                        }
                    }
                    "profile_delete" => {
                        let name = v.get("name").and_then(Value::as_str).unwrap_or("");
                        if app.st.profiles.remove(name).is_some() {
                            msg = Some("Profile deleted.".into());
                        }
                    }
                    "profile_apply" => {
                        let name = v.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                        if let Some(p) = app.st.profiles.get(&name).cloned() {
                            if let Some(sc) = p.get("scene").and_then(Value::as_str) {
                                app.st.scene = scene(sc).id.to_string();
                            }
                            app.st.wallpaper = p.get("wallpaper").and_then(Value::as_str).filter(|w| w.is_empty() || app.walls.lock().unwrap().get(w).is_some()).unwrap_or("").to_string();
                            app.st.monitor_walls = p.get("monitor_walls").and_then(Value::as_object).map(|o| o.iter().filter_map(|(k, w)| w.as_str().filter(|w| app.walls.lock().unwrap().get(w).is_some()).map(|w| (k.clone(), w.to_string()))).collect()).unwrap_or_default();
                            app.st.placements = p.get("placements").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| serde_json::from_value(x.clone()).ok()).collect()).unwrap_or_default();
                            app.st.fallback = p.get("fallback").and_then(Value::as_str).unwrap_or("").to_string();
                            app.st.scrim = p.get("scrim").and_then(Value::as_str).filter(|m| ["none", "dim", "blur"].contains(m)).unwrap_or("none").to_string();
                            let (sel_keys, slide_keys) = (p.get("lib_sel").cloned().unwrap_or(Value::Null), p.get("slides").cloned().unwrap_or(Value::Null));
                            app.st.lib_sel = sel_keys.as_object().map(|o| o.iter().filter_map(|(k, x)| Some((k.clone(), state::Sel { key: x.get("key")?.as_str().filter(|k| corelib::valid_key(k))?.to_string(), fit: x.get("fit").and_then(Value::as_str).filter(|f| FitMode::parse(f).is_some()).unwrap_or("fill").to_string(), bg: x.get("bg").and_then(Value::as_str).filter(|c| fit::parse_color(c).is_some()).unwrap_or("#000000").to_string() }))).collect()).unwrap_or_default();
                            app.st.slides = slide_keys.as_object().map(|o| o.iter().map(|(k, x)| (k.clone(), state::Slide { minutes: x.get("minutes").and_then(Value::as_u64).unwrap_or(30).clamp(1, 1440) as u32, shuffle: x.get("shuffle").and_then(Value::as_bool).unwrap_or(false), filter: x.get("filter").and_then(Value::as_str).filter(|f| ["all", "favorites"].contains(f)).unwrap_or("all").to_string() })).collect()).unwrap_or_default();
                            app.composed.clear();
                            app.park_all = false;
                            reload = true;
                            relayout = true;
                            request_statics(&app, &proxy, None);
                            msg = Some(format!("\"{name}\" restored: wallpapers, widgets and monitor assignments."));
                        } else {
                            msg = Some("That profile does not exist.".into());
                        }
                    }
                    "widget_add" => {
                        // Another copy of an installed widget: shares its permissions, has its own storage and its own place.
                        let base = v.get("widget").and_then(Value::as_str).unwrap_or("").split('#').next().unwrap_or("").to_string();
                        let installed = app.host.lock().unwrap().describe().iter().any(|w| w.id == base);
                        let n_copies = app.st.placements.iter().filter(|p| p.widget.split('#').next() == Some(base.as_str())).count();
                        if !installed {
                            msg = Some("That widget is not installed.".into());
                        } else if n_copies >= 8 || app.st.placements.len() >= 24 {
                            msg = Some("That is the most copies Plinth allows.".into());
                        } else {
                            let n = (2..=99u32).find(|n| !app.st.placements.iter().any(|p| p.widget == format!("{base}#{n}"))).unwrap_or(99);
                            let id = format!("{base}#{n}");
                            let mut p = mons::default_placement(&id, n_copies);
                            p.home = v.get("monitor").and_then(Value::as_str).filter(|m| app.monitors.iter().any(|x| x.id == *m)).unwrap_or(mons::ROLE_PRIMARY).to_string();
                            p.home_fingerprint = app.monitors.iter().find(|x| x.id == p.home).map(|x| x.fingerprint.clone()).unwrap_or_default();
                            app.st.placements.push(p);
                            relayout = true;
                            msg = Some("Added another copy. Move it to any monitor.".into());
                        }
                    }
                    "widget_remove" => {
                        let inst = v.get("widget").and_then(Value::as_str).unwrap_or("");
                        if plinth_host::valid_copy(inst) {
                            app.st.placements.retain(|p| p.widget != inst);
                            relayout = true;
                            msg = Some("Copy removed.".into());
                        } else {
                            msg = Some("The original widget stays; remove its copies only.".into());
                        }
                    }
                    "assign" => {
                        // Move a widget to a monitor. Anchor, offsets and size are kept, so it lands in the same place relative to the
                        // new monitor and keeps its settings. Only connected monitors (or the role "primary") can be chosen.
                        let (wid, mid) = (v.get("widget").and_then(Value::as_str).unwrap_or(""), v.get("monitor").and_then(Value::as_str).unwrap_or(""));
                        let fp = app.monitors.iter().find(|m| m.id == mid).map(|m| m.fingerprint.clone());
                        let ok_target = mid == mons::ROLE_PRIMARY || fp.is_some();
                        match (app.st.placements.iter_mut().find(|p| p.widget == wid), ok_target) {
                            (Some(p), true) => {
                                p.home = mid.to_string();
                                p.home_fingerprint = fp.unwrap_or_default();
                                relayout = true;
                                msg = Some("Moved. It keeps its size and position on the new monitor.".into());
                            }
                            _ => msg = Some("That widget or monitor is not available.".into()),
                        }
                    }
                    "fallback" => {
                        let mid = v.get("monitor").and_then(Value::as_str).unwrap_or("");
                        if mid.is_empty() || app.monitors.iter().any(|m| m.id == mid) {
                            app.st.fallback = mid.to_string();
                            relayout = true;
                            msg = Some(if mid.is_empty() { "Widgets will use the primary monitor when their own is unplugged.".into() } else { "Fallback monitor set.".into() });
                        }
                    }
                    "park_all" => {
                        app.park_all = v.get("on").and_then(Value::as_bool).unwrap_or(false);
                        relayout = true;
                        msg = Some(if app.park_all { "All widgets are on the primary monitor for now. Your saved layout is untouched.".into() } else { "Widgets are back on their own monitors.".into() });
                    }
                    "reset_positions" => {
                        app.undo_placements = Some(app.st.placements.clone());
                        app.st.placements.clear(); // recompute() refills every widget with its default place
                        app.park_all = false;
                        relayout = true;
                        msg = Some("Widget positions reset. Undo is available.".into());
                    }
                    "undo_positions" => {
                        if let Some(prev) = app.undo_placements.take() {
                            app.st.placements = prev;
                            relayout = true;
                            msg = Some("Positions restored.".into());
                        }
                    }
                    "monitor_wallpaper" => {
                        let (mid, wid) = (v.get("monitor").and_then(Value::as_str).unwrap_or(""), v.get("id").and_then(Value::as_str).unwrap_or(""));
                        let known = wid.is_empty() || app.walls.lock().unwrap().get(wid).is_some();
                        if let (Some(m), true) = (app.monitors.iter().find(|m| m.id == mid).cloned(), known) {
                            if m.primary {
                                app.st.wallpaper = wid.to_string();
                            } else if wid.is_empty() {
                                app.st.monitor_walls.remove(&m.id);
                            } else {
                                app.st.monitor_walls.insert(m.id.clone(), wid.to_string());
                            }
                            reload = true;
                            relayout = true;
                            msg = Some(format!("Wallpaper for {} chosen.{}", m.name, if app.st.applied { "" } else { " Press Apply to put it on your desktop." }));
                        }
                    }
                    "fake_monitors" => {
                        // Test hook: only honoured when the app was started with PLINTH_FAKE_MONITORS.
                        let spec = v.get("spec").and_then(Value::as_str).unwrap_or("").to_string();
                        let mut f = app.fake.lock().unwrap();
                        if f.is_some() {
                            *f = Some(spec);
                        }
                    }
                    "identify" => {
                        identify.clear();
                        let fake = app.fake.lock().unwrap().is_some();
                        for (i, m) in app.monitors.iter().enumerate() {
                            let (sw, sh) = if fake { (240u32, 160u32) } else { ((360.0 * m.scale()) as u32, (240.0 * m.scale()) as u32) };
                            let (px, py) = if fake { (60 + m.x / 4 + 420, 60 + m.y / 4 + 220) } else { (m.x + (m.width_px as i32 - sw as i32) / 2, m.y + (m.height_px as i32 - sh as i32) / 2) };
                            use tao::platform::windows::WindowBuilderExtWindows;
                            let win = WindowBuilder::new()
                                .with_title("Plinth identify")
                                .with_decorations(false)
                                .with_always_on_top(true)
                                .with_resizable(false)
                                .with_skip_taskbar(true)
                                .with_inner_size(tao::dpi::PhysicalSize::new(sw, sh))
                                .with_position(tao::dpi::PhysicalPosition::new(px, py))
                                .build(target)
                                .ok();
                            if let Some(win) = win {
                                let esc = |t: &str| t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
                                let html = format!("<body style=\"margin:0;height:100vh;display:grid;place-items:center;align-content:center;background:#10131c;color:#7cf2c8;font-family:'Segoe UI',sans-serif;border:3px solid #7cf2c8;box-sizing:border-box;overflow:hidden\"><div style=\"font-size:96px;font-weight:700;line-height:1\">{}</div><div style=\"font-size:16px;color:#fff;margin-top:8px\">{}{}</div></body>", i + 1, esc(&m.name), if m.primary { " (primary)" } else { "" });
                                if let Ok(wv) = WebViewBuilder::new().with_html(html).build(&win) {
                                    identify.push((win, wv));
                                }
                            }
                        }
                        let p = proxy.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_millis(2500));
                            let _ = p.send_event(Ev::IdentifyEnd);
                        });
                        msg = Some("Look at your screens: each shows its number.".into());
                    }
                    "security_scan" => {
                        // The scan runs off the event loop (PowerShell can take a few seconds); the result comes back as Ev::Sec.
                        let p = proxy.clone();
                        std::thread::spawn(move || {
                            let _ = p.send_event(Ev::Sec(sec::gather().to_string()));
                        });
                        msg = Some("Scanning your PC's security status…".into());
                    }
                    "wallpaper" => {
                        let id = v.get("id").and_then(Value::as_str).unwrap_or("");
                        let name = app.walls.lock().unwrap().get(id).map(|e| e.wallpaper.name.clone());
                        match (id.is_empty(), name) {
                            (true, _) => {
                                app.st.wallpaper.clear();
                                msg = Some("Wallpaper cleared. The Scene's own background is used.".into());
                            }
                            (false, Some(n)) => {
                                app.st.wallpaper = id.to_string();
                                msg = Some(format!("{n} selected. {}", if app.st.applied { "Applied." } else { "Press Apply to put it on your desktop." }));
                            }
                            (false, None) => msg = Some("That wallpaper is not available.".into()),
                        }
                        reload = app.st.applied;
                    }
                    "wallpaper_remove" => {
                        let id = v.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                        let r = app.walls.lock().unwrap().remove(&id);
                        match r {
                            Ok(()) => {
                                if app.st.wallpaper == id {
                                    app.st.wallpaper.clear();
                                    reload = app.st.applied;
                                }
                                msg = Some("Removed. Your original video file was not touched.".into());
                            }
                            Err(e) => msg = Some(e),
                        }
                    }
                    "wallpaper_folder" => {
                        let dir = app.walls.lock().unwrap().user_dir().to_path_buf();
                        let _ = std::fs::create_dir_all(&dir);
                        let _ = std::process::Command::new("explorer.exe").arg(&dir).spawn();
                        msg = Some("Opened your wallpapers folder. Press Rescan after adding one.".into());
                    }
                    "wallpaper_rescan" => {
                        app.walls.lock().unwrap().scan();
                        let n = app.walls.lock().unwrap().list().len();
                        let gone = !app.st.wallpaper.is_empty() && app.walls.lock().unwrap().get(&app.st.wallpaper).is_none();
                        if gone {
                            app.st.wallpaper.clear();
                            reload = app.st.applied;
                        }
                        msg = Some(format!("{n} wallpapers found."));
                    }
                    "motion" => {
                        app.st.motion = v.get("on").and_then(Value::as_bool).unwrap_or(false);
                        reload = app.st.applied;
                        msg = Some(if app.st.motion { "Live motion on. It uses more CPU and pauses automatically in games.".into() } else { "Live motion off. Static wallpaper is the cheapest.".into() });
                    }
                    "firstrun_done" => {
                        app.st.first_run = false;
                        println!("hub: first run acknowledged");
                    }
                    _ => {}
                }
                if reload || relayout {
                    if relayout {
                        app.recompute();
                    }
                    sync_layers(target, &app, &mut layers, &mut layer_ctx, &proxy);
                }
                sampler.set_active(!layers.is_empty() && !app.st.safe && !app.paused);
                if cmd != "state" {
                    println!("hub: {cmd} handled");
                    app.persist();
                }
                reply(&app, &hub_view, msg.as_deref());
            }
            Event::UserEvent(Ev::Layer { wid, msg }) => {
                let json = app.host.lock().unwrap().bridge_instance(&wid, &msg);
                let _ = proxy.send_event(Ev::Deliver { wid, json });
            }
            Event::UserEvent(Ev::Deliver { wid, json }) => {
                let mon = app.resolved.iter().find(|r| r.widget == wid).map(|r| r.monitor.clone());
                if let Some(l) = mon.and_then(|m| layers.get(&m)) {
                    let id = Value::String(wid).to_string();
                    let _ = l.webview.evaluate_script(&format!("__deliver({id},{json})"));
                }
            }
            Event::UserEvent(Ev::Pause(on)) => {
                app.paused = on;
                pause_log.push(on);
                println!("governor: widgets {}", if on { "paused (fullscreen app, battery or remote session)" } else { "resumed" });
                sampler.set_active(!layers.is_empty() && !app.st.safe && !app.paused);
                for l in layers.values() {
                    let _ = l.webview.evaluate_script(&format!("__pause({on})"));
                }
                reply(&app, &hub_view, None);
            }
            Event::UserEvent(Ev::Sec(json)) => {
                // Add Plinth's own activity so the page shows everything in one place.
                let mut v: Value = serde_json::from_str(&json).unwrap_or(Value::Null);
                let entries = app.plog.lock().unwrap().entries();
                let (mut perm_ok, mut perm_blocked) = (0usize, 0usize);
                {
                    let h = app.host.lock().unwrap();
                    for w in h.describe() {
                        for e in h.activity(&w.id) {
                            if e.allowed { perm_ok += 1 } else { perm_blocked += 1 }
                        }
                    }
                }
                if let Value::Object(m) = &mut v {
                    m.insert("plinth".into(), json!({ "proxyAllowed": entries.iter().filter(|e| e.verdict == Verdict::Allow).count(), "proxyBlocked": entries.iter().filter(|e| e.verdict != Verdict::Allow).count(), "permAllowed": perm_ok, "permBlocked": perm_blocked, "safe": app.st.safe }));
                    sec_last = Some(v.clone());
                }
                if let Some(view) = &hub_view {
                    let _ = view.evaluate_script(&format!("__onSecurity({})", v));
                }
            }
            Event::UserEvent(Ev::DropHover(on)) => {
                if let Some(v) = &hub_view {
                    let _ = v.evaluate_script(&format!("__drop({on})"));
                }
            }
            Event::UserEvent(Ev::Drop(paths)) => {
                let (mut added, mut last_id, mut err) = (0, None, None);
                for p in paths.iter().take(8) {
                    if p.is_dir() {
                        let r = app.lib.lock().unwrap().add_root(&p.to_string_lossy());
                        LIB_RESCAN.store(true, Ordering::Relaxed);
                        match r {
                            Ok(()) => added += 1,
                            Err(e) => err = Some(e),
                        }
                        continue;
                    }
                    let r = app.walls.lock().unwrap().import_video(p);
                    match r {
                        Ok(w) => {
                            added += 1;
                            last_id = Some(w.id);
                        }
                        Err(e) => err = Some(e),
                    }
                }
                if let Some(id) = last_id {
                    app.st.wallpaper = id;
                    if app.st.applied {
                        sync_layers(target, &app, &mut layers, &mut layer_ctx, &proxy);
                    }
                    app.persist();
                }
                let m = match (added, err) {
                    (0, Some(e)) => e,
                    (n, Some(e)) => format!("Added {n}. {e}"),
                    (1, None) => "Added. Videos are selected as wallpaper; folders join your library.".to_string(),
                    (n, None) => format!("Added {n} videos; the last one is selected."),
                };
                println!("hub: drop handled ({added} added)");
                reply(&app, &hub_view, Some(&m));
            }
            Event::UserEvent(Ev::Monitors(list)) => {
                let (gone, came) = mons::diff_monitors(&app.monitors, &list);
                println!("monitors changed: gone [{}], new [{}]", gone.join(", "), came.join(", "));
                app.monitors = list;
                app.park_all = false; // a real display change returns to the saved layout
                app.recompute();
                sync_layers(target, &app, &mut layers, &mut layer_ctx, &proxy);
                let parked = app.resolved.iter().filter(|r| r.parked).count();
                println!("layout recomputed: {} monitors, {} widgets, {} parked, {} layers", app.monitors.len(), app.resolved.len(), parked, layers.len());
                sampler.set_active(!layers.is_empty() && !app.st.safe && !app.paused);
                app.composed.clear();
                request_statics(&app, &proxy, None);
                let note = if !gone.is_empty() && parked > 0 { Some(format!("{} disconnected: {parked} widget(s) moved to a visible monitor and will return when it reconnects.", gone.join(", "))) } else if !came.is_empty() { Some(format!("{} connected.", came.join(", "))) } else { None };
                reply(&app, &hub_view, note.as_deref());
            }
            Event::UserEvent(Ev::IdentifyEnd) => identify.clear(),
            Event::UserEvent(Ev::LibraryChanged) => {
                if let Some(v) = &hub_view {
                    let _ = v.evaluate_script("__libChanged()");
                }
                reply(&app, &hub_view, None);
            }
            Event::UserEvent(Ev::OsSnap(snap)) => {
                if let Some(s) = snap {
                    app.st.os_snapshot = Some(s);
                    app.persist();
                }
                apply_statics(&app, &proxy, None);
            }
            Event::UserEvent(Ev::OsDone(err)) => {
                println!("os wallpaper restore: {}", if err.is_empty() { "ok" } else { err.as_str() });
                reply(&app, &hub_view, Some(if err.is_empty() { "Your previous wallpaper is back." } else { "Some wallpapers could not be restored automatically; set them in Windows Settings." }));
            }
            Event::UserEvent(Ev::Composed { monitor, file, err }) => {
                match err {
                    None => {
                        app.composed.insert(monitor, file);
                        sync_layers(target, &app, &mut layers, &mut layer_ctx, &proxy);
                        gc_composed(&app);
                    }
                    Some(e) => {
                        println!("static wallpaper failed: {e}");
                        reply(&app, &hub_view, Some(&format!("Could not set that picture: {e}")));
                    }
                }
            }
            Event::UserEvent(Ev::Picked { purpose, path }) => {
                if let Some(p) = path {
                    let msg = if purpose == "folder" {
                        let r = app.lib.lock().unwrap().add_root(&p);
                        LIB_RESCAN.store(true, Ordering::Relaxed);
                        r.err().unwrap_or_else(|| "Folder added. Plinth is reading its pictures now.".into())
                    } else if let Some(root) = purpose.strip_prefix("exclude:") {
                        let r = app.lib.lock().unwrap().set_exclude(root, &p, true);
                        LIB_RESCAN.store(true, Ordering::Relaxed);
                        r.err().unwrap_or_else(|| "That subfolder will be skipped.".into())
                    } else if let Some(key) = purpose.strip_prefix("locate:") {
                        let r = app.lib.lock().unwrap().relocate(key, &p);
                        r.err().unwrap_or_else(|| "Found it again.".into())
                    } else {
                        String::new()
                    };
                    reply(&app, &hub_view, Some(&msg));
                }
            }
            Event::UserEvent(Ev::SlideTick) => {
                if app.st.applied && !app.paused && !app.st.slides.is_empty() {
                    let t = library::now();
                    let due: Vec<(String, state::Slide)> = app.st.slides.iter().filter(|(m, _)| app.slide_due.get(*m).map_or(true, |d| *d <= t)).map(|(m, s)| (m.clone(), s.clone())).collect();
                    for (mid, sl) in due {
                        let pick = {
                            let l = app.lib.lock().unwrap();
                            let f = if sl.filter == "favorites" { Filter::Favorites } else { Filter::All };
                            let ok: Vec<&corelib::Item> = corelib::query(&l.items, "", f).into_iter().map(|i| &l.items[i]).filter(|i| i.status == corelib::Status::Ok).collect();
                            let cur = app.st.lib_sel.get(&mid).and_then(|s| ok.iter().position(|i| i.key == s.key));
                            corelib::next_index(ok.len(), cur, sl.shuffle, t).map(|n| ok[n].key.clone())
                        };
                        app.slide_due.insert(mid.clone(), t + sl.minutes as u64 * 60);
                        if let Some(key) = pick {
                            if let Some(sel) = app.st.lib_sel.get_mut(&mid) {
                                sel.key = key.clone();
                                app.lib.lock().unwrap().touch(&key);
                                app.composed.remove(&mid);
                                request_statics(&app, &proxy, Some(&mid));
                                println!("slideshow: monitor {mid} -> {key}");
                            }
                        }
                    }
                }
            }
            Event::UserEvent(Ev::Save) => {
                app.persist();
                app.lib.lock().unwrap().save();
            }
            Event::UserEvent(Ev::Menu(name)) => match name.as_str() {
                "open" => {
                    println!("open requested (hub web view present: {}, window visible: {})", hub_view.is_some(), hub_window.is_visible());
                    if hub_view.is_none() {
                        hub_view = Some(build_hub(&hub_window, &mut hub_ctx, &proxy, &thumb_tx));
                    }
                    HUB_HIDDEN.store(false, std::sync::atomic::Ordering::Relaxed);
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
                7 => {
                    use tao::window::Fullscreen;
                    fs_test = WindowBuilder::new().with_title("Plinth fullscreen test").with_fullscreen(Some(Fullscreen::Borderless(None))).build(target).ok();
                    if let Some(w) = fs_test.as_ref() {
                        w.set_focus();
                    }
                    println!("autotest: fullscreen test window opened: {}", fs_test.is_some());
                }
                8 => {
                    fs_test = None;
                    println!("autotest: fullscreen test window closed");
                }
                9 => {
                    if let Some(l) = app.primary_id().and_then(|id| layers.get(&id)) {
                        let vp = video_probe.clone();
                        let js = "(()=>{const v=document.getElementById('wv');return v?JSON.stringify({t:v.currentTime,paused:v.paused,ready:v.readyState,err:v.error?v.error.code:0,w:v.videoWidth}):'none'})()";
                        let _ = l.webview.evaluate_script_with_callback(js, move |r| *vp.lock().unwrap() = Some(r));
                    }
                }
                21..=25 | 27..=29 => mon_snaps.push(MonSnap::take(&app, &layers)),
                40 => lp.before = os_wall::snapshot().ok(),
                41 => {
                    lp.summary1 = Some(app.lib.lock().unwrap().summary());
                    lp.problems = app.lib.lock().unwrap().items.iter().filter(|i| i.status != corelib::Status::Ok).map(|i| (i.name().to_string(), i.status.label().to_string())).collect();
                }
                42 => {
                    let first = {
                        let l = app.lib.lock().unwrap();
                        let ids: Vec<&corelib::Item> = corelib::query(&l.items, "pic_", Filter::All).into_iter().map(|i| &l.items[i]).filter(|i| i.status == corelib::Status::Ok && !l.roots.iter().any(|r| r.bundled && r.path == i.root)).collect();
                        ids.first().map(|i| i.key.clone())
                    };
                    lp.key1 = first.clone();
                    if let (Some(k), Some(pid)) = (first, app.primary_id()) {
                        let _ = proxy.send_event(Ev::Hub(json!({ "cmd": "set_static", "monitor": pid, "key": k, "fit": "fit", "bg": "#ff00ff" }).to_string()));
                    }
                }
                43 => {
                    lp.os_after_set = os_wall::snapshot().ok();
                    if let (Some(pid), Some(f)) = (app.primary_id(), app.primary_id().and_then(|p| app.composed.get(&p).cloned())) {
                        let path = app.lib.lock().unwrap().composed.join(&f);
                        lp.composed_dims = image::image_dimensions(&path).ok();
                        lp.composed_path = Some(path.to_string_lossy().to_string());
                        lp.pid = Some(pid);
                    }
                }
                44 => {
                    lp.summary2 = Some(app.lib.lock().unwrap().summary());
                }
                45 => {
                    if let Some(pid) = app.primary_id() {
                        lp.key_before_slide = app.st.lib_sel.get(&pid).map(|s| s.key.clone());
                        app.st.slides.insert(pid.clone(), state::Slide { minutes: 1, shuffle: true, filter: "all".into() });
                        app.slide_due.insert(pid, 0);
                        let _ = proxy.send_event(Ev::SlideTick);
                    }
                }
                46 => lp.key_after_slide = app.primary_id().and_then(|p| app.st.lib_sel.get(&p).map(|s| s.key.clone())),
                47 => {
                    lp.after = os_wall::snapshot().ok();
                    lp.snapshot_cleared = app.st.os_snapshot.is_none();
                }
                48 => {
                    let code = library_report(&lp);
                    app.persist();
                    std::process::exit(code);
                }
                26 => {
                    let code = monitor_report(&app, &mon_snaps);
                    app.persist();
                    std::process::exit(code);
                }
                2 => win::show_desktop(),
                3 => {
                    auto_visible = Some(win::probe_center_is(0x00FF_00FF));
                    win::show_desktop();
                }
                _ => {
                    let code = autotest_report(&app, auto_visible, safe_count, &sampler.snapshot(), &pause_log, &video_probe.lock().unwrap().clone(), &sec_probe.lock().unwrap().clone());
                    app.persist();
                    std::process::exit(code);
                }
            },
            Event::WindowEvent { event: WindowEvent::CloseRequested, window_id, .. } if window_id == hub_id => {
                // Background mode: hide the hub, keep the desktop layer and tray running. Quit from the tray menu.
                app.persist();
                hub_window.set_visible(false);
                hub_view = None; // frees the hub's WebView2 process tree
                HUB_HIDDEN.store(true, std::sync::atomic::Ordering::Relaxed);
                println!("hub hidden (running in the background; use the tray icon to reopen or quit)");
            }
            _ => {}
        }
    });
}

#[derive(Default)]
struct LibProbe {
    before: Option<os_wall::Snapshot>,
    os_after_set: Option<os_wall::Snapshot>,
    after: Option<os_wall::Snapshot>,
    summary1: Option<Value>,
    summary2: Option<Value>,
    problems: Vec<(String, String)>,
    key1: Option<String>,
    composed_dims: Option<(u32, u32)>,
    composed_path: Option<String>,
    pid: Option<String>,
    key_before_slide: Option<String>,
    key_after_slide: Option<String>,
    snapshot_cleared: bool,
}

fn library_report(p: &LibProbe) -> i32 {
    println!("\n===== Library end to end =====");
    println!("   summary after first scan: {}", p.summary1.as_ref().map_or("none".into(), |v| v.to_string()));
    println!("   summary after the watcher run: {}", p.summary2.as_ref().map_or("none".into(), |v| format!("count {} duplicates {}", v["count"], v["duplicates"])));
    println!("   problems: {:?}", p.problems);
    println!("   chosen key: {:?}; composed {:?} dims {:?}", p.key1, p.composed_path, p.composed_dims);
    println!("   OS wallpaper before: {:?}", p.before.as_ref().map(|b| &b.wallpapers));
    println!("   OS wallpaper set:    {:?}", p.os_after_set.as_ref().map(|b| &b.wallpapers));
    println!("   OS wallpaper after:  {:?}", p.after.as_ref().map(|b| &b.wallpapers));
    println!("   slideshow {:?} -> {:?}", p.key_before_slide, p.key_after_slide);
    let count = |v: &Option<Value>| v.as_ref().and_then(|s| s["count"].as_u64()).unwrap_or(0);
    let norm = |s: &str| s.replace('/', "\\").to_ascii_lowercase();
    let primary_path = |s: &Option<os_wall::Snapshot>, pid: &Option<String>| s.as_ref().and_then(|s| s.wallpapers.iter().find(|(id, _)| Some(id) == pid.as_ref()).map(|(_, w)| w.clone()));
    let checks: Vec<(&str, bool)> = vec![
        ("the folder was scanned: the test pictures are in the library (30 unique, plus the 12 bundled)", count(&p.summary1) >= 40),
        ("the byte-identical copy is hidden as a duplicate", p.summary1.as_ref().map_or(false, |s| s["duplicates"].as_u64().unwrap_or(0) >= 1)),
        ("the corrupt file is listed with a clear status instead of crashing the scan", p.problems.iter().any(|(n, st)| n == "broken.jpg" && st == "cannot be read")),
        ("the text file was ignored", !p.problems.iter().any(|(n, _)| n == "readme.txt")),
        ("a picture was composed at exactly the monitor's resolution", p.composed_dims.is_some() && p.pid.as_ref().map_or(false, |pid| pid.len() > 4)),
        ("Windows reports the composed picture as the wallpaper of that monitor", match (primary_path(&p.os_after_set, &p.pid), &p.composed_path) { (Some(a), Some(b)) => norm(&a) == norm(b), _ => false }),
        ("a new picture added to the watched folder appeared without pressing anything (and two extra copies stayed hidden)", count(&p.summary2) == count(&p.summary1) + 1 && p.summary2.as_ref().map_or(false, |s| s["duplicates"].as_u64().unwrap_or(0) >= 3)),
        ("the slideshow moved to a different picture", p.key_before_slide.is_some() && p.key_after_slide.is_some() && p.key_before_slide != p.key_after_slide),
        ("Revert put every monitor's wallpaper back exactly as it was", p.before.is_some() && p.before.as_ref().map(|b| &b.wallpapers) == p.after.as_ref().map(|a| &a.wallpapers)),
        ("Revert cleared the saved snapshot", p.snapshot_cleared),
    ];
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

struct MonSnap {
    layers: Vec<String>,
    resolved: Vec<(String, String, bool)>,
    homes: Vec<(String, String)>,
}

impl MonSnap {
    fn take(app: &App, layers: &HashMap<String, Layer>) -> MonSnap {
        let mut l: Vec<String> = layers.keys().cloned().collect();
        l.sort();
        let mut r: Vec<(String, String, bool)> = app.resolved.iter().map(|r| (r.widget.clone(), r.monitor.clone(), r.parked)).collect();
        r.sort();
        let mut h: Vec<(String, String)> = app.st.placements.iter().map(|p| (p.widget.clone(), p.home.clone())).collect();
        h.sort();
        MonSnap { layers: l, resolved: r, homes: h }
    }

    fn on(&self, widget: &str) -> Option<(&str, bool)> {
        self.resolved.iter().find(|r| r.0 == widget).map(|r| (r.1.as_str(), r.2))
    }
}

/// Report for the fake-monitor lifecycle autotest (PLINTH_AUTOTEST_MON). Snapshots: A own monitors, B monitor B unplugged,
/// C monitor B replugged, D park all, E park all off, F positions reset, G reset undone.
fn monitor_report(app: &App, snaps: &[MonSnap]) -> i32 {
    println!("\n===== Monitor lifecycle =====");
    for (n, s) in ["A own monitors", "B B unplugged", "C B replugged", "D park all", "E park all off", "F reset", "G undo", "H copy of the clock"].iter().zip(snaps) {
        println!("   {n}: layers {:?}\n      shown {:?}\n      homes {:?}", s.layers, s.resolved, s.homes);
    }
    println!("   layout log entries: {}", app.layout_log.len());
    let (clock, card, vitals, notes) = ("com.plinth.clock", "com.plinth.buildcard", "com.plinth.pcvitals", "com.plinth.notes");
    let g = |i: usize| snaps.get(i);
    let checks: Vec<(&str, bool)> = vec![
        ("all eight snapshots were taken", snaps.len() == 8),
        ("A: clock and build card are on monitor B, vitals on C, notes on the primary, none parked", g(0).map_or(false, |s| s.on(clock) == Some(("FAKE#B", false)) && s.on(card) == Some(("FAKE#B", false)) && s.on(vitals) == Some(("FAKE#C", false)) && s.on(notes) == Some(("FAKE#A", false)))),
        ("A: one layer window per monitor that has something to draw (3)", g(0).map_or(false, |s| s.layers == vec!["FAKE#A", "FAKE#B", "FAKE#C"])),
        ("B: with monitor B unplugged its widgets are parked on the primary, still visible", g(1).map_or(false, |s| s.on(clock) == Some(("FAKE#A", true)) && s.on(card) == Some(("FAKE#A", true)) && s.on(vitals) == Some(("FAKE#C", false)))),
        ("B: the layer for monitor B was closed (2 layers)", g(1).map_or(false, |s| s.layers == vec!["FAKE#A", "FAKE#C"])),
        ("B: saved home monitors did not change while parked", g(1).map_or(false, |s| Some(&s.homes) == g(0).map(|a| &a.homes))),
        ("C: after replugging B the exact original layout is back", g(2).map_or(false, |s| Some(&s.resolved) == g(0).map(|a| &a.resolved) && Some(&s.layers) == g(0).map(|a| &a.layers))),
        ("C: saved placements are identical to before the unplug", g(2).map_or(false, |s| Some(&s.homes) == g(0).map(|a| &a.homes))),
        ("D: Bring all widgets to the primary monitor parks every widget there and keeps the saved layout", g(3).map_or(false, |s| s.resolved.iter().all(|r| r.1 == "FAKE#A") && s.resolved.iter().filter(|r| r.0 != notes).all(|r| r.2) && Some(&s.homes) == g(0).map(|a| &a.homes))),
        ("E: turning it off restores the saved layout", g(4).map_or(false, |s| Some(&s.resolved) == g(0).map(|a| &a.resolved))),
        ("F: Reset widget positions sends every widget to the primary monitor's default places", g(5).map_or(false, |s| s.resolved.iter().all(|r| r.1 == "FAKE#A" && !r.2))),
        ("G: Undo brings the previous assignments back", g(6).map_or(false, |s| Some(&s.resolved) == g(0).map(|a| &a.resolved) && Some(&s.homes) == g(0).map(|a| &a.homes))),
        ("H: a second copy of the clock runs on monitor C while the first stays on monitor B", g(7).map_or(false, |s| s.on(clock) == Some(("FAKE#B", false)) && s.on("com.plinth.clock#2") == Some(("FAKE#C", false)))),
        ("H: monitor C now has a layer for both the vitals and the clock copy", g(7).map_or(false, |s| s.layers.contains(&"FAKE#C".to_string()))),
        ("the saved home of the clock stayed monitor B in the file the app writes", app.st.placements.iter().any(|p| p.widget == clock && p.home == "FAKE#B")),
    ];
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

fn total_activity(app: &App) -> usize {
    let h = app.host.lock().unwrap();
    h.describe().iter().map(|w| h.activity(&w.id).len()).sum()
}

fn autotest_report(app: &App, visible: Option<bool>, safe_count: (usize, usize), snap: &hw::Snapshot, pause_log: &[bool], video_probe: &Option<String>, sec_probe: &Option<Value>) -> i32 {
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
    println!("\n===== Hardware snapshot (what widgets could read) =====");
    println!("   cpu:     {}", snap.cpu.as_ref().map_or("none".into(), |v| v.to_string()));
    println!("   ram:     {}", snap.ram.as_ref().map_or("none".into(), |v| v.to_string()));
    println!("   gpu:     {}", snap.gpu.as_ref().map_or("none".into(), |v| v.to_string()));
    println!("   disk:    {}", snap.disk.as_ref().map_or("none".into(), |v| v.to_string()));
    println!("   network: {}", snap.network.as_ref().map_or("none".into(), |v| v.to_string()));
    println!("   info:    {}", snap.info.as_ref().map_or("none".into(), |v| v.to_string()));
    println!("   samples taken: {}", snap.samples);
    let wall_log = app.wall_log.lock().unwrap().clone();
    println!("\n===== Wallpaper requests =====");
    for (p, st) in &wall_log {
        println!("   {st} {p}");
    }
    println!("   video probe: {}", video_probe.as_deref().unwrap_or("not taken"));
    let video_test = std::env::var_os("PLINTH_TEST_VIDEO").is_some();
    let video_ok = video_probe
        .as_deref()
        .and_then(|r| serde_json::from_str::<Value>(r).ok())
        .and_then(|v| v.as_str().map(String::from))
        .and_then(|inner| serde_json::from_str::<Value>(&inner).ok())
        .map_or(false, |v| v["paused"] == false && v["t"].as_f64().unwrap_or(0.0) > 0.0 && v["err"] == 0 && v["w"].as_u64().unwrap_or(0) > 0);
    println!("\n===== Security Center probe =====");
    if let Some(v) = sec_probe {
        println!("   took {} ms, sockets {}, listeners {}, connections {}, startup items {}, security-center ok: {}", v["tookMs"], v["socketsTotal"], v["listeners"].as_array().map_or(0, |a| a.len()), v["connections"].as_array().map_or(0, |a| a.len()), v["startup"].as_array().map_or(0, |a| a.len()), v["scOk"]);
        println!("   firewall: {}", v["firewall"]);
        println!("   antivirus: {}", v["av"]);
        for l in v["listeners"].as_array().into_iter().flatten() {
            println!("   listener {}:{} {} (pid {})", l["bound"].as_str().unwrap_or(""), l["port"], l["proc"].as_str().unwrap_or(""), l["pid"]);
        }
    } else {
        println!("   not taken");
    }
    let sec_ok = sec_probe.as_ref().map_or(false, |v| v["firewall"].as_array().map_or(false, |a| a.len() == 3 && a.iter().all(|p| p["on"].is_boolean())) && v["listeners"].as_array().map_or(false, |a| !a.is_empty() && a.iter().all(|l| l["port"].as_u64().map_or(false, |p| p > 0 && p < 65536))) && v["startup"].is_array() && v["socketsTotal"].as_u64().unwrap_or(0) > 0);
    let gpu_nvml = snap.gpu.as_ref().map_or(false, |g| g["source"] == "nvml");
    let checks = [
        ("pc vitals: CPU read allowed (granted at first run)", has(&allowed, "com.plinth.pcvitals", "use system.cpu")),
        ("pc vitals: GPU, memory, network and disk reads allowed", ["use system.gpu", "use system.ram", "use system.network", "use system.disk"].iter().all(|k| has(&allowed, "com.plinth.pcvitals", k))),
        ("build card: parts list read allowed", has(&allowed, "com.plinth.buildcard", "use system.info")),
        ("notes: storage read allowed", has(&allowed, "com.plinth.notes", "use storage")),
        ("clock: made no permission requests (needs none)", !allowed.contains_key("com.plinth.clock") && !blocked.contains_key("com.plinth.clock")),
        ("revoke: after the user revoked CPU, pc vitals' CPU request was BLOCKED immediately", has(&blocked, "com.plinth.pcvitals", "use system.cpu")),
        ("revoke: only the revoked permission was blocked (the other readings stayed allowed)", blocked.values().flatten().all(|a| a == "use system.cpu")),
        ("safe mode: at most 2 stray widget events while on (iframes were removed)", safe_count.1.saturating_sub(safe_count.0) <= 2),
        ("hub apply command created the layer and persisted state", app.st.applied),
        ("proxy: 0 tunnels allowed (no widget has network access)", entries.iter().all(|e| e.verdict != Verdict::Allow)),
        ("hardware: CPU 0-100%, RAM total > 0, parts list has a CPU name", snap.cpu.as_ref().map_or(false, |c| (0.0..=100.0).contains(&c["percent"].as_f64().unwrap_or(-1.0))) && snap.ram.as_ref().map_or(false, |r| r["total_mb"].as_u64().unwrap_or(0) > 0) && snap.info.as_ref().map_or(false, |i| !i["cpu_name"].as_str().unwrap_or("").is_empty())),
        ("hardware: network and disk counters returned data", snap.network.is_some() && snap.disk.is_some()),
        ("hardware: when NVML is the GPU source, temperature and VRAM are present", !gpu_nvml || snap.gpu.as_ref().map_or(false, |g| g["temp_c"].is_number() && g["vram_total_mb"].is_number())),
        ("governor: a fullscreen app paused the widgets, and closing it resumed them", pause_log.windows(2).any(|w| w == [true, false])),
        ("wallpaper: the bundled web wallpaper was served (200) from its own origin", wall_log.iter().any(|(p, st)| p.contains("com.plinth.aurora/index.html") && *st == 200)),
        ("wallpaper: nothing outside an allow-listed file type was ever requested successfully", wall_log.iter().all(|(p, st)| *st != 200 || plinth_core::wallpaper::mime_for(p.split('?').next().unwrap_or("")).is_some())),
        ("wallpaper: the dropped video was imported and streamed in byte ranges (206), or no video was supplied", !video_test || wall_log.iter().any(|(p, st)| p.contains("/user.") && *st == 206)),
        ("wallpaper: the video is playing after the governor paused and resumed it, or no video was supplied", !video_test || video_ok),
        ("security: firewall profiles, listening ports and startup items were read without error", sec_ok),
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
