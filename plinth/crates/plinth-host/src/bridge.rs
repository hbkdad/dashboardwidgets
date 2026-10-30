//! Widget -> host bridge. Every call is checked by the permission broker and logged.

use crate::host::Host;
use plinth_core::{Decision, PermissionId};
use serde_json::{json, Value};

const STORAGE_KEY_MAX: usize = 64;
const STORAGE_VALUE_MAX: usize = 8 * 1024;
const STORAGE_TOTAL_MAX: usize = 64 * 1024;

fn reply(id: &Value, r: Result<Value, String>) -> String {
    match r {
        Ok(v) => json!({ "id": id, "ok": v }),
        Err(e) => json!({ "id": id, "error": e }),
    }
    .to_string()
}

impl Host {
    /// Handles one widget message from the trusted host page. `slot` comes from the host page's own
    /// bookkeeping (which iframe the message event came from), never from the message itself.
    /// Returns the JSON response to deliver back to that widget.
    pub fn bridge(&mut self, slot: usize, msg: &str) -> String {
        let Some(Some(pkg)) = self.slots.get(slot) else {
            return reply(&Value::Null, Err("unknown_slot".into()));
        };
        let wid = pkg.manifest.id.clone();
        let v: Value = match serde_json::from_str(msg) {
            Ok(v) => v,
            Err(_) => return reply(&Value::Null, Err("bad_json".into())),
        };
        let id = v.get("id").cloned().unwrap_or(Value::Null);
        if !(id.is_number() || id.is_null()) {
            return reply(&Value::Null, Err("bad_id".into()));
        }
        let method = v.get("method").and_then(Value::as_str).unwrap_or("");
        let params = v.get("params").cloned().unwrap_or(Value::Null);
        let r = self.dispatch(&wid, method, &params);
        reply(&id, r)
    }

    fn check(&mut self, wid: &str, perm: PermissionId) -> Result<(), String> {
        match self.broker.check(wid, perm) {
            Decision::Allowed => Ok(()),
            Decision::Denied(r) => Err(format!("denied:{r:?}")),
        }
    }

    fn dispatch(&mut self, wid: &str, method: &str, params: &Value) -> Result<Value, String> {
        match method {
            "data.get" => {
                let key = params.get("key").and_then(Value::as_str).unwrap_or("");
                // Only system.* data keys are readable through this call.
                let perm = PermissionId::parse(key).filter(|_| key.starts_with("system.")).ok_or("bad_key")?;
                self.check(wid, perm)?;
                self.provider.read(perm).ok_or_else(|| "unavailable".to_string())
            }
            "storage.get" | "storage.set" => {
                self.check(wid, PermissionId::Storage)?;
                let key = params.get("key").and_then(Value::as_str).ok_or("bad_key")?.to_string();
                if key.is_empty() || key.len() > STORAGE_KEY_MAX {
                    return Err("bad_key".into());
                }
                if method == "storage.get" {
                    return Ok(self.storage.get(wid).and_then(|m| m.get(&key)).map(|v| json!(v)).unwrap_or(Value::Null));
                }
                let value = params.get("value").and_then(Value::as_str).ok_or("bad_value")?.to_string();
                if value.len() > STORAGE_VALUE_MAX {
                    return Err("quota:value_too_large".into());
                }
                let map = self.storage.entry(wid.to_string()).or_default();
                let used: usize = map.iter().filter(|(k, _)| **k != key).map(|(k, v)| k.len() + v.len()).sum();
                if used + key.len() + value.len() > STORAGE_TOTAL_MAX {
                    return Err("quota:total_exceeded".into());
                }
                map.insert(key, value);
                Ok(Value::Bool(true))
            }
            "diag.log" => {
                let text = params.get("text").and_then(Value::as_str).unwrap_or("");
                self.broker.note(wid, text);
                Ok(Value::Bool(true))
            }
            _ => Err("unknown_method".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::host::{Host, HostError, MockProvider, SDK_PATH, THIRD_PARTY_SLOTS};
    use crate::package::Package;
    use plinth_core::PermissionId;
    use serde_json::{json, Value};

    fn pkg(id: &str, perms: &str) -> Package {
        let m = format!(r#"{{"format":1,"id":"{id}","name":"W","version":"1.0.0","type":"widget","permissions":{perms}}}"#);
        Package::from_files(&m, vec![("index.html".into(), b"<html>ok</html>".to_vec()), ("app.js".into(), b"1".to_vec())]).unwrap()
    }
    const ALL: &str = r#"[{"id":"system.cpu","reason":"c"},{"id":"storage","reason":"s"},{"id":"net.fetch","hosts":["api.weather.example"],"reason":"w"}]"#;
    fn host() -> Host {
        Host::new(Box::new(MockProvider))
    }
    fn call(h: &mut Host, slot: usize, method: &str, params: Value) -> Value {
        serde_json::from_str(&h.bridge(slot, &json!({ "id": 1, "method": method, "params": params }).to_string())).unwrap()
    }

    #[test]
    fn slots_allocate_free_and_cap() {
        let mut h = host();
        for i in 0..THIRD_PARTY_SLOTS {
            assert_eq!(h.install(pkg(&format!("com.example.w{i}"), "[]")), Ok(i));
        }
        assert_eq!(h.install(pkg("com.example.extra", "[]")), Err(HostError::SlotsFull));
        h.uninstall("com.example.w3");
        assert_eq!(h.install(pkg("com.example.extra", "[]")), Ok(3));
        assert_eq!(h.install(pkg("com.example.w5", "[]")), Ok(5), "update keeps slot");
    }

    #[test]
    fn serves_package_files_with_security_headers_only_for_valid_hosts() {
        let mut h = host();
        let slot = h.install(pkg("com.example.w", ALL)).unwrap();
        let hn = format!("pw{slot}.localhost");
        let r = h.serve(&hn, "/");
        assert_eq!(r.status, 200);
        assert_eq!(r.body, b"<html>ok</html>");
        assert_eq!(r.header("x-content-type-options"), Some("nosniff"));
        let csp = r.header("content-security-policy").unwrap();
        assert!(csp.contains("default-src 'none'") && csp.contains("connect-src 'none'") && csp.contains("script-src 'self'"));
        assert!(!csp.contains("unsafe-eval"));
        assert_eq!(h.serve(&hn, "/app.js").header("content-type"), Some("text/javascript; charset=utf-8"));
        assert_eq!(h.serve(&hn, "/manifest.json").status, 404);
        assert_eq!(h.serve(&hn, "/../../etc/passwd").status, 404);
        assert_eq!(h.serve(&hn, SDK_PATH).status, 200);
        assert_eq!(h.serve("pw7.localhost", "/").status, 404, "empty slot");
        assert_eq!(h.serve("pw99.localhost", "/").status, 404);
        assert_eq!(h.serve("evil.example", "/").status, 404);
        assert_eq!(h.serve("pw0.localhost.evil.example", "/").status, 404);
    }

    #[test]
    fn csp_connect_src_follows_grants_and_safe_mode() {
        let mut h = host();
        let slot = h.install(pkg("com.example.w", ALL)).unwrap();
        let hn = format!("pw{slot}.localhost");
        h.grant("com.example.w", PermissionId::NetFetch).unwrap();
        assert!(h.serve(&hn, "/").header("content-security-policy").unwrap().contains("connect-src https://api.weather.example;"));
        h.revoke("com.example.w", PermissionId::NetFetch);
        assert!(h.serve(&hn, "/").header("content-security-policy").unwrap().contains("connect-src 'none'"));
        h.set_safe_mode(true);
        assert_eq!(h.serve(&hn, "/").status, 403);
    }

    #[test]
    fn bridge_denies_by_default_then_allows_after_grant_and_logs() {
        let mut h = host();
        let slot = h.install(pkg("com.example.w", ALL)).unwrap();
        assert_eq!(call(&mut h, slot, "data.get", json!({ "key": "system.cpu" }))["error"], "denied:NotGranted");
        h.grant("com.example.w", PermissionId::SystemCpu).unwrap();
        assert_eq!(call(&mut h, slot, "data.get", json!({ "key": "system.cpu" }))["ok"]["percent"], 12.5);
        h.revoke("com.example.w", PermissionId::SystemCpu);
        assert_eq!(call(&mut h, slot, "data.get", json!({ "key": "system.cpu" }))["error"], "denied:NotGranted");
        let log: Vec<bool> = h.activity("com.example.w").iter().map(|e| e.allowed).collect();
        assert_eq!(log, vec![false, true, false]);
    }

    #[test]
    fn bridge_rejects_undeclared_permissions_bad_input_and_unknown_slots() {
        let mut h = host();
        let slot = h.install(pkg("com.example.w", r#"[{"id":"system.cpu","reason":"c"}]"#)).unwrap();
        assert_eq!(call(&mut h, slot, "storage.set", json!({ "key": "k", "value": "v" }))["error"], "denied:NotDeclared");
        assert_eq!(call(&mut h, slot, "data.get", json!({ "key": "net.fetch" }))["error"], "bad_key");
        assert_eq!(call(&mut h, slot, "data.get", json!({ "key": "../../x" }))["error"], "bad_key");
        assert_eq!(call(&mut h, slot, "nope", json!({}))["error"], "unknown_method");
        assert_eq!(serde_json::from_str::<Value>(&h.bridge(slot, "not json")).unwrap()["error"], "bad_json");
        assert_eq!(serde_json::from_str::<Value>(&h.bridge(slot, r#"{"id":"str","method":"diag.log"}"#)).unwrap()["error"], "bad_id");
        assert_eq!(serde_json::from_str::<Value>(&h.bridge(5, "{}")).unwrap()["error"], "unknown_slot");
        assert_eq!(serde_json::from_str::<Value>(&h.bridge(999, "{}")).unwrap()["error"], "unknown_slot");
    }

    #[test]
    fn storage_is_per_widget_with_quotas_and_cleared_on_uninstall() {
        let mut h = host();
        let a = h.install(pkg("com.example.a", ALL)).unwrap();
        let b = h.install(pkg("com.example.b", ALL)).unwrap();
        for id in ["com.example.a", "com.example.b"] {
            h.grant(id, PermissionId::Storage).unwrap();
        }
        assert_eq!(call(&mut h, a, "storage.set", json!({ "key": "k", "value": "secret" }))["ok"], true);
        assert_eq!(call(&mut h, a, "storage.get", json!({ "key": "k" }))["ok"], "secret");
        assert_eq!(call(&mut h, b, "storage.get", json!({ "key": "k" }))["ok"], Value::Null, "no cross-widget read");
        let big = "x".repeat(8 * 1024 + 1);
        assert_eq!(call(&mut h, a, "storage.set", json!({ "key": "big", "value": big }))["error"], "quota:value_too_large");
        let mut last = Value::Null;
        for i in 0..9 {
            last = call(&mut h, a, "storage.set", json!({ "key": format!("n{i}"), "value": "y".repeat(8 * 1024) }));
        }
        assert_eq!(last["error"], "quota:total_exceeded");
        h.uninstall("com.example.a");
        let a2 = h.install(pkg("com.example.a", ALL)).unwrap();
        h.grant("com.example.a", PermissionId::Storage).unwrap();
        assert_eq!(call(&mut h, a2, "storage.get", json!({ "key": "k" }))["ok"], Value::Null);
    }

    #[test]
    fn update_with_new_permission_is_not_granted() {
        let mut h = host();
        let slot = h.install(pkg("com.example.w", r#"[{"id":"system.cpu","reason":"c"}]"#)).unwrap();
        h.grant("com.example.w", PermissionId::SystemCpu).unwrap();
        h.install(pkg("com.example.w", r#"[{"id":"system.cpu","reason":"c"},{"id":"system.ram","reason":"r"}]"#)).unwrap();
        assert_eq!(call(&mut h, slot, "data.get", json!({ "key": "system.cpu" }))["ok"]["percent"], 12.5);
        assert_eq!(call(&mut h, slot, "data.get", json!({ "key": "system.ram" }))["error"], "denied:NotGranted");
    }

    #[test]
    fn diag_log_is_recorded_and_truncated_and_safe_mode_denies() {
        let mut h = host();
        let slot = h.install(pkg("com.example.w", ALL)).unwrap();
        call(&mut h, slot, "diag.log", json!({ "text": "z".repeat(1000) }));
        let a = h.activity("com.example.w");
        assert_eq!(a.len(), 1);
        assert!(a[0].action.len() <= 205);
        h.grant("com.example.w", PermissionId::SystemCpu).unwrap();
        h.set_safe_mode(true);
        assert_eq!(call(&mut h, slot, "data.get", json!({ "key": "system.cpu" }))["error"], "denied:SafeMode");
    }

    #[test]
    fn allowed_destinations_is_the_union_of_granted_hosts() {
        let mut h = host();
        h.install(pkg("com.example.a", ALL)).unwrap();
        h.install(pkg("com.example.b", r#"[{"id":"net.fetch","hosts":["api.weather.example","cdn.example"],"reason":"x"}]"#)).unwrap();
        assert!(h.allowed_destinations().is_empty(), "nothing granted yet");
        h.grant("com.example.a", PermissionId::NetFetch).unwrap();
        h.grant("com.example.b", PermissionId::NetFetch).unwrap();
        assert_eq!(h.allowed_destinations(), vec![("api.weather.example".to_string(), 443), ("cdn.example".to_string(), 443)]);
        h.revoke("com.example.b", PermissionId::NetFetch);
        assert_eq!(h.allowed_destinations(), vec![("api.weather.example".to_string(), 443)]);
        h.set_safe_mode(true);
        assert!(h.allowed_destinations().is_empty());
    }

    #[test]
    fn describe_reports_declared_permissions_and_grant_state() {
        let mut h = host();
        h.install(pkg("com.example.w", ALL)).unwrap();
        h.grant("com.example.w", PermissionId::SystemCpu).unwrap();
        let d = h.describe();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].slot, 0);
        let cpu = d[0].permissions.iter().find(|p| p.id == PermissionId::SystemCpu).unwrap();
        let net = d[0].permissions.iter().find(|p| p.id == PermissionId::NetFetch).unwrap();
        assert!(cpu.granted && !net.granted);
        assert_eq!(net.hosts, vec!["api.weather.example"]);
        h.revoke("com.example.w", PermissionId::SystemCpu);
        assert!(!h.describe()[0].permissions.iter().find(|p| p.id == PermissionId::SystemCpu).unwrap().granted);
    }
}
