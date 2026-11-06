use crate::package::{normalize_path, Package};
use plinth_core::{ActivityEntry, Broker, PermissionId};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Soft cap on concurrent third-party widgets: each gets its own origin and WebView2 process
/// (about +29 MB each in Spike C). Revisit after measuring on low-end hardware.
pub const THIRD_PARTY_SLOTS: usize = 8;
/// Origin shared by all trusted (first-party) widgets: one process instead of one per widget. Paths are /<widget id>/...
pub const TRUSTED_SCHEME: &str = "pwt";
pub const MAX_TRUSTED: usize = 16;
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
    pub trusted: bool,
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
    pub(crate) trusted: Vec<Package>,
    pub(crate) provider: Box<dyn DataProvider>,
    pub(crate) storage: HashMap<String, HashMap<String, String>>,
    /// Storage namespace for the message being handled: the widget id, or `id#n` for another copy of the same widget.
    pub(crate) cur_ns: String,
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
        Host { broker: Broker::new(2000), slots: (0..THIRD_PARTY_SLOTS).map(|_| None).collect(), trusted: Vec::new(), provider, storage: HashMap::new(), cur_ns: String::new(), safe: false }
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
        self.trusted.retain(|p| p.manifest.id != id);
        self.broker.unregister(id);
        self.storage.retain(|k, _| k != id && !k.starts_with(&format!("{id}#")));
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
            .chain(self.trusted.iter())
            .flat_map(|p| self.broker.granted_fetch_hosts(&p.manifest.id))
            .map(|h| (h, 443u16))
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// Everything a UI needs to render the Permission Center: installed widgets with declared permissions and grant state.
    pub fn describe(&self) -> Vec<WidgetInfo> {
        let info = |slot: usize, trusted: bool, p: &Package| {
            let m = &p.manifest;
            WidgetInfo {
                slot,
                trusted,
                id: m.id.clone(),
                name: m.name.clone(),
                version: m.version.clone(),
                permissions: m
                    .permissions
                    .iter()
                    .map(|d| PermissionInfo { id: d.id, reason: d.reason.clone(), hosts: d.hosts.clone(), granted: self.broker.is_granted(&m.id, d.id) })
                    .collect(),
            }
        };
        let mut v: Vec<WidgetInfo> = self.trusted.iter().map(|p| info(0, true, p)).collect();
        v.extend(self.slots.iter().enumerate().filter_map(|(slot, p)| p.as_ref().map(|p| info(slot, false, p))));
        v
    }

    /// Installs (or updates) a trusted first-party widget into the shared origin. The caller decides trust: only call
    /// this for packages shipped with the app, never for downloaded content.
    pub fn install_trusted(&mut self, pkg: Package) -> Result<(), HostError> {
        let id = pkg.manifest.id.clone();
        if self.slots.iter().flatten().any(|p| p.manifest.id == id) {
            return Err(HostError::Denied("id already installed as third-party".into()));
        }
        match self.trusted.iter().position(|p| p.manifest.id == id) {
            Some(i) => {
                self.broker.register(&pkg.manifest);
                self.trusted[i] = pkg;
            }
            None if self.trusted.len() >= MAX_TRUSTED => return Err(HostError::SlotsFull),
            None => {
                self.broker.register(&pkg.manifest);
                self.trusted.push(pkg);
            }
        }
        Ok(())
    }

    pub fn trusted_ids(&self) -> Vec<String> {
        self.trusted.iter().map(|p| p.manifest.id.clone()).collect()
    }

    /// Widget storage for persistence by the app (per widget id: key -> value).
    pub fn storage_snapshot(&self) -> HashMap<String, HashMap<String, String>> {
        self.storage.clone()
    }

    /// Restores storage for installed widgets only; values over the quotas are dropped.
    pub fn restore_storage(&mut self, data: HashMap<String, HashMap<String, String>>) {
        for (id, map) in data {
            let base = id.split('#').next().unwrap_or(&id).to_string();
            if (base == id || valid_copy(&id)) && (self.slots.iter().flatten().any(|p| p.manifest.id == base) || self.trusted.iter().any(|p| p.manifest.id == base)) {
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
        if host.split(':').next() == Some(&format!("{TRUSTED_SCHEME}.localhost")) {
            return self.serve_trusted(path);
        }
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


    fn serve_trusted(&mut self, path: &str) -> Response {
        let strict = "default-src 'none'";
        if path.split('?').next() == Some(SDK_PATH) {
            return self.respond(200, "text/javascript; charset=utf-8", strict, SDK_JS.as_bytes().to_vec());
        }
        let p = path.split(['?', '#']).next().unwrap_or("").trim_start_matches('/');
        let Some((id, rest)) = p.split_once('/') else {
            return self.respond(404, "text/plain", strict, b"not found".to_vec());
        };
        let Some(pkg) = self.trusted.iter().find(|pk| pk.manifest.id == id) else {
            return self.respond(404, "text/plain", strict, b"not found".to_vec());
        };
        let csp = self.csp_for(id);
        if self.safe {
            return self.respond(403, "text/html; charset=utf-8", &csp, b"<p>Safe mode: widgets are disabled.</p>".to_vec());
        }
        let Some(norm) = normalize_path(&format!("/{rest}")) else {
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

    /// Like `bridge`, but the trusted layer page names the widget (it created and tracks every iframe). Works for trusted
    /// and third-party widgets alike; the widget id never comes from the message.
    pub fn bridge_widget(&mut self, id: &str, msg: &str) -> String {
        self.bridge_instance(id, msg)
    }

    /// `instance` is a widget id, or `id#n` (2 to 99) for another copy of the same widget. Permissions, grants and the activity
    /// log belong to the widget; each copy has its own storage. As everywhere, the trusted layer page names the instance.
    pub fn bridge_instance(&mut self, instance: &str, msg: &str) -> String {
        if instance != instance.split('#').next().unwrap_or("") && !valid_copy(instance) {
            return serde_json::json!({ "id": null, "error": "unknown_widget" }).to_string();
        }
        let id = instance.split('#').next().unwrap_or(instance).to_string();
        self.cur_ns = instance.to_string();
        let out = self.bridge_widget_base(&id, msg);
        self.cur_ns.clear();
        out
    }

    fn bridge_widget_base(&mut self, id: &str, msg: &str) -> String {
        if let Some(slot) = self.slot_of(id) {
            return self.bridge(slot, msg);
        }
        if self.trusted.iter().any(|p| p.manifest.id == id) {
            return self.bridge_as(id, msg);
        }
        serde_json::json!({ "id": null, "error": "unknown_widget" }).to_string()
    }
}

/// `id#n` with n from 2 to 99 (copy 1 is the plain widget id).
pub fn valid_copy(instance: &str) -> bool {
    match instance.split_once('#') {
        Some((base, n)) => !base.is_empty() && !n.is_empty() && n.len() <= 2 && !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit()) && matches!(n.parse::<u32>(), Ok(v) if (2..=99).contains(&v)),
        None => false,
    }
}

#[cfg(test)]
mod sdk_tests {
    use super::SDK_JS;

    #[test]
    fn skin_messages_are_applied_only_after_the_sender_is_checked_and_values_are_validated() {
        let parent_check = SDK_JS.find("e.source !== parent").expect("the SDK checks who sent a message");
        let skin = SDK_JS.find("plinthSkin").expect("the SDK understands skin messages");
        assert!(parent_check < skin, "the sender check must come before any skin handling");
        assert!(SDK_JS.contains("^#[0-9a-fA-F]{6}$"), "the accent must be a hex color");
        assert!(SDK_JS.contains("k.font.length < 160"), "the font stack is length-limited");
        for banned in ["eval(", "innerHTML", "Function(", "document.write", "importScripts", "fetch("] {
            assert!(!SDK_JS.contains(banned), "the SDK must not contain {banned}");
        }
    }
}
