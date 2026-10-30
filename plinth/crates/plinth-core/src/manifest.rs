//! `manifest.json` parsing and validation (format 1). See architecture.md section 4.

use serde::Deserialize;
use std::fmt;

pub const SUPPORTED_FORMAT: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionId {
    SystemCpu,
    SystemRam,
    SystemGpu,
    SystemDisk,
    SystemNetwork,
    MediaControl,
    CalendarRead,
    NetFetch,
    Storage,
    Notifications,
}

impl PermissionId {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "system.cpu" => Self::SystemCpu,
            "system.ram" => Self::SystemRam,
            "system.gpu" => Self::SystemGpu,
            "system.disk" => Self::SystemDisk,
            "system.network" => Self::SystemNetwork,
            "media.control" => Self::MediaControl,
            "calendar.read" => Self::CalendarRead,
            "net.fetch" => Self::NetFetch,
            "storage" => Self::Storage,
            "notifications" => Self::Notifications,
            _ => return None,
        })
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SystemCpu => "system.cpu",
            Self::SystemRam => "system.ram",
            Self::SystemGpu => "system.gpu",
            Self::SystemDisk => "system.disk",
            Self::SystemNetwork => "system.network",
            Self::MediaControl => "media.control",
            Self::CalendarRead => "calendar.read",
            Self::NetFetch => "net.fetch",
            Self::Storage => "storage",
            Self::Notifications => "notifications",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Widget,
    Scene,
    Theme,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredPermission {
    pub id: PermissionId,
    /// Lower-cased exact hosts; only meaningful for `net.fetch`.
    pub hosts: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: Kind,
    pub permissions: Vec<DeclaredPermission>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ManifestError {
    Parse(String),
    UnsupportedFormat(u32),
    BadId(String),
    BadVersion(String),
    BadKind(String),
    UnknownPermission(String),
    MissingReason(String),
    HostsRequired,
    HostsNotAllowed(String),
    BadHost(String),
    DuplicatePermission(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ManifestError {}

#[derive(Deserialize)]
struct RawPermission {
    id: String,
    #[serde(default)]
    hosts: Vec<String>,
    #[serde(default)]
    reason: String,
}

#[derive(Deserialize)]
struct RawManifest {
    format: u32,
    id: String,
    name: String,
    version: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    permissions: Vec<RawPermission>,
}

pub(crate) fn valid_id(id: &str) -> bool {
    id.len() <= 128
        && id.contains('.')
        && !id.starts_with('.')
        && !id.ends_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

pub(crate) fn valid_version(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn valid_host(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 253
        && h.contains('.')
        && !h.starts_with('.')
        && !h.ends_with('.')
        && h.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Manifest, ManifestError> {
        let raw: RawManifest =
            serde_json::from_str(json).map_err(|e| ManifestError::Parse(e.to_string()))?;
        if raw.format != SUPPORTED_FORMAT {
            return Err(ManifestError::UnsupportedFormat(raw.format));
        }
        if !valid_id(&raw.id) {
            return Err(ManifestError::BadId(raw.id));
        }
        if !valid_version(&raw.version) {
            return Err(ManifestError::BadVersion(raw.version));
        }
        let kind = match raw.kind.as_str() {
            "widget" => Kind::Widget,
            "scene" => Kind::Scene,
            "theme" => Kind::Theme,
            other => return Err(ManifestError::BadKind(other.to_string())),
        };

        let mut permissions: Vec<DeclaredPermission> = Vec::new();
        for p in raw.permissions {
            let id = PermissionId::parse(&p.id)
                .ok_or_else(|| ManifestError::UnknownPermission(p.id.clone()))?;
            if permissions.iter().any(|d| d.id == id) {
                return Err(ManifestError::DuplicatePermission(p.id));
            }
            if p.reason.trim().is_empty() {
                return Err(ManifestError::MissingReason(p.id));
            }
            let hosts: Vec<String> = p.hosts.iter().map(|h| h.to_ascii_lowercase()).collect();
            if id == PermissionId::NetFetch {
                if hosts.is_empty() {
                    return Err(ManifestError::HostsRequired);
                }
                if let Some(bad) = hosts.iter().find(|h| !valid_host(h)) {
                    return Err(ManifestError::BadHost(bad.clone()));
                }
            } else if !hosts.is_empty() {
                return Err(ManifestError::HostsNotAllowed(p.id));
            }
            permissions.push(DeclaredPermission { id, hosts, reason: p.reason });
        }

        Ok(Manifest { id: raw.id, name: raw.name, version: raw.version, kind, permissions })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(perms: &str) -> String {
        format!(
            r#"{{"format":1,"id":"com.example.clock","name":"Clock","version":"1.0.0","type":"widget","permissions":{perms}}}"#
        )
    }

    #[test]
    fn accepts_valid_manifest() {
        let m = Manifest::parse(&base(
            r#"[{"id":"system.cpu","reason":"Show load"},{"id":"net.fetch","hosts":["API.Weather.example"],"reason":"Weather"}]"#,
        ))
        .unwrap();
        assert_eq!(m.permissions.len(), 2);
        assert_eq!(m.permissions[1].hosts, vec!["api.weather.example"]);
    }

    #[test]
    fn rejects_unknown_format_and_bad_fields() {
        assert_eq!(
            Manifest::parse(&base("[]").replace("\"format\":1", "\"format\":2")),
            Err(ManifestError::UnsupportedFormat(2))
        );
        assert!(matches!(
            Manifest::parse(&base("[]").replace("com.example.clock", "../evil")),
            Err(ManifestError::BadId(_))
        ));
        assert!(matches!(
            Manifest::parse(&base("[]").replace("1.0.0", "1.0")),
            Err(ManifestError::BadVersion(_))
        ));
        assert!(matches!(Manifest::parse("not json"), Err(ManifestError::Parse(_))));
    }

    #[test]
    fn rejects_dangerous_or_sloppy_permissions() {
        // No filesystem/process permission exists in the vocabulary.
        assert!(matches!(
            Manifest::parse(&base(r#"[{"id":"fs.read","reason":"x"}]"#)),
            Err(ManifestError::UnknownPermission(_))
        ));
        assert_eq!(
            Manifest::parse(&base(r#"[{"id":"net.fetch","reason":"x"}]"#)),
            Err(ManifestError::HostsRequired)
        );
        assert!(matches!(
            Manifest::parse(&base(r#"[{"id":"net.fetch","hosts":["*"],"reason":"x"}]"#)),
            Err(ManifestError::BadHost(_))
        ));
        assert!(matches!(
            Manifest::parse(&base(r#"[{"id":"net.fetch","hosts":["https://a.example/x"],"reason":"x"}]"#)),
            Err(ManifestError::BadHost(_))
        ));
        assert!(matches!(
            Manifest::parse(&base(r#"[{"id":"system.cpu","hosts":["a.example"],"reason":"x"}]"#)),
            Err(ManifestError::HostsNotAllowed(_))
        ));
        assert!(matches!(
            Manifest::parse(&base(r#"[{"id":"system.cpu","reason":"  "}]"#)),
            Err(ManifestError::MissingReason(_))
        ));
        assert!(matches!(
            Manifest::parse(&base(
                r#"[{"id":"system.cpu","reason":"a"},{"id":"system.cpu","reason":"b"}]"#
            )),
            Err(ManifestError::DuplicatePermission(_))
        ));
    }
}
