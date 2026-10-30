use crate::package::{normalize_path, Package};
use plinth_core::{ActivityEntry, Broker, PermissionId};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Soft cap on concurrent third-party widgets: each gets its own origin and WebView2 process
/// (about +29 MB each in Spike C). Revisit after measuring on low-end hardware.
pub const THIRD_PARTY_SLOTS: usize = 8;
pub const SDK_PATH: &str = "/__plinth/sdk.js";
const SDK_JS: &str = include_str!("sdk.js");


#[derive(Debug, PartialEq, Eq)]
pub enum HostError {
    SlotsFull,
    UnknownWidget,
    Denied(String),
}

#[derive(Debug, Clone)]
pub struct PermissionInfo {
    pub id: PermissionId,
    pub reason: String,
    pub hosts: Vec<String>,
    pub granted: bool,
}

#[derive(Debug, Clone)]
pub struct WidgetInfo {
    pub slot: usize,
    pub id: String,
    pub name: String,
    pub version: String,
    pub permissions: Vec<PermissionInfo>,
}

pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// Source of system data. The real implementation (Windows counters) lives in the app; tests use the mock.
pub trait DataProvider: Send {
    fn read(&self, perm: PermissionId) -> Option<Value>;
}

pub struct MockProvider;
impl DataProvider for MockProvider {
    fn read(&self, perm: PermissionId) -> Option<Value> {
        match perm {
            PermissionId::SystemCpu => Some(json!({ "percent": 12.5 })),
            PermissionId::SystemRam => Some(json!({ "used_mb": 8123, "total_mb": 16384 })),
            _ => None,
        }
    }
}

pub struct Host {
    pub(crate) broker: Broker,
    pub(crate) slots: Vec<Option<Package>>,
    pub(crate) provider: Box<dyn DataProvider>,
    pub(crate) storage: HashMap<String, HashMap<String, String>>,
    safe: bool,
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

impl Host {
    pub fn new(provider: Box<dyn DataProvider>) -> Self {
        Host { broker: Broker::new(2000), slots: (0..THIRD_PARTY_SLOTS).map(|_| None).collect(), provider, storage: HashMap::new(), safe: false }
    }

    pub fn scheme_for_slot(slot: usize) -> String {
        format!("pw{slot}")
    }

    /// Installs or updates a widget. Updating keeps the slot and the stored data; newly declared
    /// permissions are not granted (broker rule).
    pub fn install(&mut self, pkg: Package) -> Result<usize, HostError> {
        let id = pkg.manifest.id.clone();
        let existing = self.slots.iter().position(|s| s.as_ref().map_or(false, |p| p.manifest.id == id));
        let slot = match existing {
            Some(i) => i,
            None => self.slots.iter().position(|s| s.is_none()).ok_or(HostError::SlotsFull)?,
        };
        self.broker.register(&pkg.manifest);
        self.slots[slot] = Some(pkg);
        Ok(slot)
    }

    pub fn uninstall(&mut self, id: &str) {
        for s in self.slots.iter_mut() {
            if s.as_ref().map_or(false, |p| p.manifest.id == id) {
                *s = None;
            }
        }
        self.broker.unregister(id);
        self.storage.remove(id);
    }

    pub fn slot_of(&self, id: &str) -> Option<usize> {
        self.slots.iter().position(|s| s.as_ref().map_or(false, |p| p.manifest.id == id))
    }

    pub fn installed(&self) -> Vec<(usize, String)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| s.as_ref().map(|p| (i, p.manifest.id.clone()))).collect()
    }

    pub fn grant(&mut self, id: &str, perm: PermissionId) -> Result<(), HostError> {
        self.broker.grant(id, perm).map_err(|r| HostError::Denied(format!("{r:?}")))
    }

    pub fn revoke(&mut self, id: &str, perm: PermissionId) {
        self.broker.revoke(id, perm);
    }

    pub fn set_safe_mode(&mut self, on: bool) {
        self.safe = on;
        self.broker.set_safe_mode(on);
    }

    /// Every (host, port) any installed widget may currently reach: the union of granted `net.fetch` hosts, port 443.
    /// Feeds the network proxy allow-list. Per-widget attribution is the CSP's job; the proxy enforces the union.
    pub fn allowed_destinations(&self) -> Vec<(String, u16)> {
        let mut v: Vec<(String, u16)> = self
            .slots
            .iter()
            .flatten()
            .flat_map(|p| self.broker.granted_fetch_hosts(&p.manifest.id))
            .map(|h| (h, 443u16))
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// Everything a UI needs to render the Permission Center: installed widgets with declared permissions and grant state.
    pub fn describe(&self) -> Vec<WidgetInfo> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(slot, p)| {
                let p = p.as_ref()?;
                let m = &p.manifest;
                Some(WidgetInfo {
                    slot,
                    id: m.id.clone(),
                    name: m.name.clone(),
                    version: m.version.clone(),
                    permissions: m
                        .permissions
                        .iter()
                        .map(|d| PermissionInfo { id: d.id, reason: d.reason.clone(), hosts: d.hosts.clone(), granted: self.broker.is_granted(&m.id, d.id) })
                        .collect(),
                })
            })
            .collect()
    }

    /// Widget storage for persistence by the app (per widget id: key -> value).
    pub fn storage_snapshot(&self) -> HashMap<String, HashMap<String, String>> {
        self.storage.clone()
    }

    /// Restores storage for installed widgets only; values over the quotas are dropped.
    pub fn restore_storage(&mut self, data: HashMap<String, HashMap<String, String>>) {
        for (id, map) in data {
            if self.slots.iter().flatten().any(|p| p.manifest.id == id) {
                let ok: HashMap<String, String> = map.into_iter().filter(|(k, v)| k.len() <= 64 && v.len() <= 8 * 1024).take(256).collect();
                self.storage.insert(id, ok);
            }
        }
    }

    pub fn activity(&self, id: &str) -> Vec<&ActivityEntry> {
        self.broker.activity(id)
    }

    /// CSP built from the manifest and current grants. Widgets cannot alter it (HTTP header).
    /// Changing grants takes effect for the connect-src part on the widget's next load.
    pub fn csp_for(&self, id: &str) -> String {
        let hosts = self.broker.granted_fetch_hosts(id);
        let connect = if hosts.is_empty() {
            "'none'".to_string()
        } else {
            hosts.iter().map(|h| format!("https://{h}")).collect::<Vec<_>>().join(" ")
        };
        format!(
            "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; \
             connect-src {connect}; base-uri 'none'; form-action 'none'"
        )
    }

    fn slot_from_host(host: &str) -> Option<usize> {
        let h = host.split(':').next()?;
        let n = h.strip_prefix("pw")?.strip_suffix(".localhost")?;
        n.parse::<usize>().ok().filter(|i| *i < THIRD_PARTY_SLOTS)
    }

    fn respond(&self, status: u16, ctype: &str, csp: &str, body: Vec<u8>) -> Response {
        Response {
            status,
            headers: vec![
                ("Content-Type".into(), ctype.into()),
                ("Content-Security-Policy".into(), csp.into()),
                ("X-Content-Type-Options".into(), "nosniff".into()),
                ("Referrer-Policy".into(), "no-referrer".into()),
                ("Cache-Control".into(), "no-store".into()),
            ],
            body,
        }
    }

    /// Handles a request to `http://pw<slot>.localhost<path>`.
    pub fn serve(&mut self, host: &str, path: &str) -> Response {
        let strict = "default-src 'none'";
        let Some(slot) = Self::slot_from_host(host) else {
            return self.respond(404, "text/plain", strict, b"not found".to_vec());
        };
        let Some(pkg) = self.slots[slot].as_ref() else {
            return self.respond(404, "text/plain", strict, b"not found".to_vec());
        };
        let id = pkg.manifest.id.clone();
        let csp = self.csp_for(&id);
        if self.safe {
            return self.respond(403, "text/html; charset=utf-8", &csp, b"<p>Safe mode: third-party widgets are disabled.</p>".to_vec());
        }
        if path.split('?').next() == Some(SDK_PATH) {
            return self.respond(200, "text/javascript; charset=utf-8", &csp, SDK_JS.as_bytes().to_vec());
        }
        let Some(norm) = normalize_path(path) else {
            return self.respond(404, "text/plain", &csp, b"not found".to_vec());
        };
        match pkg.get(&norm) {
            Some(bytes) => {
                let body = bytes.to_vec();
                self.respond(200, content_type(&norm), &csp, body)
            }
            None => self.respond(404, "text/plain", &csp, b"not found".to_vec()),
        }
    }

}
