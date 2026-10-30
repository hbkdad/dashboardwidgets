//! Permission broker: the core of the Permission Center (architecture.md section 3.2).
//! Rules: deny by default; a permission must be BOTH declared in the manifest AND granted by the user;
//! `net.fetch` additionally requires the exact host to be declared; every check is logged.

use crate::manifest::{DeclaredPermission, Manifest, PermissionId};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyReason {
    UnknownWidget,
    SafeMode,
    NotDeclared,
    NotGranted,
    HostNotDeclared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allowed,
    Denied(DenyReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityEntry {
    pub seq: u64,
    pub widget: String,
    pub action: String,
    pub allowed: bool,
}

pub struct Broker {
    declared: HashMap<String, Vec<DeclaredPermission>>,
    granted: HashMap<String, HashSet<PermissionId>>,
    log: VecDeque<ActivityEntry>,
    log_cap: usize,
    seq: u64,
    safe_mode: bool,
}

impl Broker {
    pub fn new(log_cap: usize) -> Self {
        Self {
            declared: HashMap::new(),
            granted: HashMap::new(),
            log: VecDeque::new(),
            log_cap: log_cap.max(1),
            seq: 0,
            safe_mode: false,
        }
    }

    /// Registering (or re-registering on update) resets grants: an update never inherits
    /// grants for permissions it newly declares, and drops grants it no longer declares.
    pub fn register(&mut self, m: &Manifest) {
        let kept: HashSet<PermissionId> = match self.granted.get(&m.id) {
            Some(old) => old
                .iter()
                .copied()
                .filter(|p| m.permissions.iter().any(|d| d.id == *p))
                .collect(),
            None => HashSet::new(),
        };
        self.declared.insert(m.id.clone(), m.permissions.clone());
        self.granted.insert(m.id.clone(), kept);
    }

    pub fn unregister(&mut self, widget: &str) {
        self.declared.remove(widget);
        self.granted.remove(widget);
    }

    pub fn set_safe_mode(&mut self, on: bool) {
        self.safe_mode = on;
    }

    /// User grants a permission. Fails if the widget never declared it.
    pub fn grant(&mut self, widget: &str, perm: PermissionId) -> Result<(), DenyReason> {
        let declared = self.declared.get(widget).ok_or(DenyReason::UnknownWidget)?;
        if !declared.iter().any(|d| d.id == perm) {
            return Err(DenyReason::NotDeclared);
        }
        self.granted.entry(widget.to_string()).or_default().insert(perm);
        Ok(())
    }

    /// Takes effect on the next check: there is no cached allow.
    pub fn revoke(&mut self, widget: &str, perm: PermissionId) {
        if let Some(g) = self.granted.get_mut(widget) {
            g.remove(&perm);
        }
    }

    pub fn revoke_all(&mut self, widget: &str) {
        if let Some(g) = self.granted.get_mut(widget) {
            g.clear();
        }
    }

    pub fn check(&mut self, widget: &str, perm: PermissionId) -> Decision {
        let d = self.decide(widget, perm, None);
        self.record(widget, format!("use {}", perm.as_str()), d);
        d
    }

    pub fn check_fetch(&mut self, widget: &str, host: &str) -> Decision {
        let host = host.to_ascii_lowercase();
        let d = self.decide(widget, PermissionId::NetFetch, Some(&host));
        self.record(widget, format!("fetch {host}"), d);
        d
    }

    fn decide(&self, widget: &str, perm: PermissionId, host: Option<&str>) -> Decision {
        if self.safe_mode {
            return Decision::Denied(DenyReason::SafeMode);
        }
        let Some(declared) = self.declared.get(widget) else {
            return Decision::Denied(DenyReason::UnknownWidget);
        };
        let Some(decl) = declared.iter().find(|d| d.id == perm) else {
            return Decision::Denied(DenyReason::NotDeclared);
        };
        let granted = self.granted.get(widget).map_or(false, |g| g.contains(&perm));
        if !granted {
            return Decision::Denied(DenyReason::NotGranted);
        }
        if let Some(h) = host {
            // Exact match only: no wildcard, no implicit subdomains.
            if !decl.hosts.iter().any(|d| d == h) {
                return Decision::Denied(DenyReason::HostNotDeclared);
            }
        }
        Decision::Allowed
    }

    fn record(&mut self, widget: &str, action: String, d: Decision) {
        self.seq += 1;
        if self.log.len() == self.log_cap {
            self.log.pop_front();
        }
        self.log.push_back(ActivityEntry {
            seq: self.seq,
            widget: widget.to_string(),
            action,
            allowed: d == Decision::Allowed,
        });
    }

    pub fn is_granted(&self, widget: &str, perm: PermissionId) -> bool {
        self.granted.get(widget).map_or(false, |g| g.contains(&perm))
    }

    pub fn is_registered(&self, widget: &str) -> bool {
        self.declared.contains_key(widget)
    }

    /// Hosts the widget may currently reach: declared `net.fetch` hosts, only if the user granted it and
    /// Safe mode is off. Does not write to the activity log (used to build the CSP).
    pub fn granted_fetch_hosts(&self, widget: &str) -> Vec<String> {
        if self.safe_mode || !self.granted.get(widget).map_or(false, |g| g.contains(&PermissionId::NetFetch)) {
            return Vec::new();
        }
        self.declared
            .get(widget)
            .and_then(|d| d.iter().find(|p| p.id == PermissionId::NetFetch))
            .map(|p| p.hosts.clone())
            .unwrap_or_default()
    }

    /// Records a widget-supplied diagnostic note in the activity log (always "allowed", text is truncated).
    pub fn note(&mut self, widget: &str, text: &str) {
        let t: String = text.chars().take(200).collect();
        self.record(widget, format!("note {t}"), Decision::Allowed);
    }

    pub fn activity(&self, widget: &str) -> Vec<&ActivityEntry> {
        self.log.iter().filter(|e| e.widget == widget).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(id: &str, perms: &str) -> Manifest {
        Manifest::parse(&format!(
            r#"{{"format":1,"id":"{id}","name":"W","version":"1.0.0","type":"widget","permissions":{perms}}}"#
        ))
        .unwrap()
    }

    const CPU_AND_NET: &str = r#"[{"id":"system.cpu","reason":"load"},{"id":"net.fetch","hosts":["api.weather.example"],"reason":"weather"}]"#;

    #[test]
    fn deny_by_default_until_user_grants() {
        let mut b = Broker::new(100);
        b.register(&manifest("com.example.w", CPU_AND_NET));
        assert_eq!(b.check("com.example.w", PermissionId::SystemCpu), Decision::Denied(DenyReason::NotGranted));
        b.grant("com.example.w", PermissionId::SystemCpu).unwrap();
        assert_eq!(b.check("com.example.w", PermissionId::SystemCpu), Decision::Allowed);
    }

    #[test]
    fn cannot_use_or_be_granted_undeclared_permission() {
        let mut b = Broker::new(100);
        b.register(&manifest("com.example.w", CPU_AND_NET));
        assert_eq!(b.grant("com.example.w", PermissionId::Storage), Err(DenyReason::NotDeclared));
        assert_eq!(b.check("com.example.w", PermissionId::Storage), Decision::Denied(DenyReason::NotDeclared));
        assert_eq!(b.check("nope.nope", PermissionId::SystemCpu), Decision::Denied(DenyReason::UnknownWidget));
    }

    #[test]
    fn fetch_requires_exact_declared_host() {
        let mut b = Broker::new(100);
        b.register(&manifest("com.example.w", CPU_AND_NET));
        b.grant("com.example.w", PermissionId::NetFetch).unwrap();
        assert_eq!(b.check_fetch("com.example.w", "API.weather.example"), Decision::Allowed);
        for evil in ["evil.example", "weather.example", "x.api.weather.example", "api.weather.example.evil.example"] {
            assert_eq!(b.check_fetch("com.example.w", evil), Decision::Denied(DenyReason::HostNotDeclared), "{evil}");
        }
    }

    #[test]
    fn revoke_is_immediate_and_safe_mode_overrides_grants() {
        let mut b = Broker::new(100);
        b.register(&manifest("com.example.w", CPU_AND_NET));
        b.grant("com.example.w", PermissionId::SystemCpu).unwrap();
        b.revoke("com.example.w", PermissionId::SystemCpu);
        assert_eq!(b.check("com.example.w", PermissionId::SystemCpu), Decision::Denied(DenyReason::NotGranted));
        b.grant("com.example.w", PermissionId::SystemCpu).unwrap();
        b.set_safe_mode(true);
        assert_eq!(b.check("com.example.w", PermissionId::SystemCpu), Decision::Denied(DenyReason::SafeMode));
        b.set_safe_mode(false);
        assert_eq!(b.check("com.example.w", PermissionId::SystemCpu), Decision::Allowed);
    }

    #[test]
    fn update_does_not_inherit_new_grants_and_drops_removed_ones() {
        let mut b = Broker::new(100);
        b.register(&manifest("com.example.w", r#"[{"id":"system.cpu","reason":"a"}]"#));
        b.grant("com.example.w", PermissionId::SystemCpu).unwrap();
        // v2 adds net.fetch and keeps cpu: cpu stays granted, net.fetch is NOT granted.
        b.register(&manifest("com.example.w", CPU_AND_NET));
        assert_eq!(b.check("com.example.w", PermissionId::SystemCpu), Decision::Allowed);
        assert_eq!(b.check_fetch("com.example.w", "api.weather.example"), Decision::Denied(DenyReason::NotGranted));
        // v3 drops cpu, then v4 re-adds it: the old grant must be gone.
        b.register(&manifest("com.example.w", r#"[{"id":"storage","reason":"s"}]"#));
        b.register(&manifest("com.example.w", r#"[{"id":"system.cpu","reason":"a"}]"#));
        assert_eq!(b.check("com.example.w", PermissionId::SystemCpu), Decision::Denied(DenyReason::NotGranted));
    }

    #[test]
    fn granted_fetch_hosts_requires_grant_and_respects_safe_mode() {
        let mut b = Broker::new(100);
        b.register(&manifest("com.example.w", CPU_AND_NET));
        assert!(b.granted_fetch_hosts("com.example.w").is_empty());
        b.grant("com.example.w", PermissionId::NetFetch).unwrap();
        assert_eq!(b.granted_fetch_hosts("com.example.w"), vec!["api.weather.example"]);
        b.set_safe_mode(true);
        assert!(b.granted_fetch_hosts("com.example.w").is_empty());
        assert!(b.granted_fetch_hosts("nope.nope").is_empty());
        assert!(b.activity("com.example.w").is_empty(), "building a CSP must not log");
    }

    #[test]
    fn notes_are_logged_and_truncated() {
        let mut b = Broker::new(100);
        b.register(&manifest("com.example.w", "[]"));
        b.note("com.example.w", &"x".repeat(500));
        let a = b.activity("com.example.w");
        assert_eq!(a.len(), 1);
        assert!(a[0].action.len() <= "note ".len() + 200);
    }

    #[test]
    fn activity_log_is_per_widget_and_capped() {
        let mut b = Broker::new(3);
        b.register(&manifest("com.example.a", CPU_AND_NET));
        b.register(&manifest("com.example.b", CPU_AND_NET));
        for _ in 0..5 {
            b.check("com.example.a", PermissionId::SystemCpu);
        }
        b.check("com.example.b", PermissionId::SystemCpu);
        assert_eq!(b.activity("com.example.a").len(), 2); // cap 3 shared: last 3 = a,a,b
        assert_eq!(b.activity("com.example.b").len(), 1);
        assert!(b.activity("com.example.a").iter().all(|e| !e.allowed));
    }
}
