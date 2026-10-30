//! Security Center logic: pure classification of facts the app collects from Windows. This is visibility, not an antivirus:
//! nothing here decides that something is malware. It labels where a connection goes, what a listening port is exposed to,
//! and where a startup program runs from, and leaves the judgement to the person.
//!
//! The product-state decode follows community documentation of the Windows Security Center `productState` value, not an
//! official Microsoft page (UNVERIFIED); the app therefore shows the product name first and the decoded flags as "reports".

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpClass {
    /// 127.0.0.0/8 and ::1
    Loopback,
    /// RFC 1918, unique local (fc00::/7), CGNAT range 100.64.0.0/10
    Private,
    /// 169.254.0.0/16 and fe80::/10
    LinkLocal,
    Unspecified,
    Multicast,
    Public,
}

impl IpClass {
    pub fn label(self) -> &'static str {
        match self {
            IpClass::Loopback => "this PC",
            IpClass::Private => "local network",
            IpClass::LinkLocal => "link-local",
            IpClass::Unspecified => "any address",
            IpClass::Multicast => "multicast",
            IpClass::Public => "internet",
        }
    }
}

pub fn classify_ip(ip: IpAddr) -> IpClass {
    match ip {
        IpAddr::V4(a) => classify_v4(a),
        IpAddr::V6(a) => {
            if let Some(v4) = a.to_ipv4_mapped() {
                return classify_v4(v4);
            }
            classify_v6(a)
        }
    }
}

fn classify_v4(a: Ipv4Addr) -> IpClass {
    let o = a.octets();
    if a.is_unspecified() {
        IpClass::Unspecified
    } else if a.is_loopback() {
        IpClass::Loopback
    } else if a.is_multicast() || a.is_broadcast() {
        IpClass::Multicast
    } else if a.is_link_local() {
        IpClass::LinkLocal
    } else if a.is_private() || (o[0] == 100 && (64..=127).contains(&o[1])) {
        IpClass::Private
    } else {
        IpClass::Public
    }
}

fn classify_v6(a: Ipv6Addr) -> IpClass {
    let s = a.segments();
    if a.is_unspecified() {
        IpClass::Unspecified
    } else if a.is_loopback() {
        IpClass::Loopback
    } else if a.is_multicast() {
        IpClass::Multicast
    } else if s[0] & 0xffc0 == 0xfe80 {
        IpClass::LinkLocal
    } else if s[0] & 0xfe00 == 0xfc00 {
        IpClass::Private
    } else {
        IpClass::Public
    }
}

/// What a listening socket is reachable from, given the address it is bound to.
pub fn exposure(bound: IpAddr) -> &'static str {
    match classify_ip(bound) {
        IpClass::Loopback => "this PC only",
        IpClass::Unspecified => "every network this PC is on",
        IpClass::Private | IpClass::LinkLocal => "your local network",
        IpClass::Public => "the internet (public address)",
        IpClass::Multicast => "multicast",
    }
}

/// True when a listener can be reached from other machines (not loopback only).
pub fn is_reachable_from_network(bound: IpAddr) -> bool {
    !matches!(classify_ip(bound), IpClass::Loopback | IpClass::Multicast)
}

/// Pull the executable path out of a Run-key or shortcut command line: `"C:\A B\x.exe" -arg`, `C:\A\x.exe -arg`,
/// `C:\Program Files\A\x.exe /s` (unquoted with spaces: cut at the first `.exe`/`.com`/`.bat`/`.cmd`/`.lnk`/`.scr`).
pub fn command_path(cmd: &str) -> Option<String> {
    let c = cmd.trim();
    if c.is_empty() {
        return None;
    }
    if let Some(rest) = c.strip_prefix('"') {
        let end = rest.find('"')?;
        let p = &rest[..end];
        return if p.is_empty() { None } else { Some(p.to_string()) };
    }
    let lower = c.to_ascii_lowercase();
    let mut best: Option<usize> = None;
    for ext in [".exe", ".com", ".bat", ".cmd", ".lnk", ".scr"] {
        if let Some(i) = lower.find(ext) {
            let end = i + ext.len();
            if best.map_or(true, |b| end < b) {
                best = Some(end);
            }
        }
    }
    match best {
        Some(end) => Some(c[..end].to_string()),
        None => c.split_whitespace().next().map(String::from),
    }
}

/// Where a startup program lives, as a neutral description (not a verdict).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    /// Under the Windows folder or Program Files: normally needs administrator rights to write.
    ProtectedFolder,
    /// Under the user profile (AppData, Downloads, Temp, Desktop...): any program the user runs can write there.
    UserFolder,
    Other,
}

impl Location {
    pub fn label(self) -> &'static str {
        match self {
            Location::ProtectedFolder => "installed (protected folder)",
            Location::UserFolder => "runs from your user folder",
            Location::Other => "other location",
        }
    }
}

pub fn classify_location(path: &str) -> Location {
    let p = path.replace('/', "\\").to_ascii_lowercase();
    // Environment variables are expanded by the caller; handle the common unexpanded forms defensively.
    for prefix in ["%programfiles%", "%programfiles(x86)%", "%windir%", "%systemroot%", "%programw6432%"] {
        if p.starts_with(prefix) {
            return Location::ProtectedFolder;
        }
    }
    for prefix in ["%appdata%", "%localappdata%", "%userprofile%", "%temp%", "%tmp%"] {
        if p.starts_with(prefix) {
            return Location::UserFolder;
        }
    }
    let after_drive = p.get(1..).unwrap_or("");
    if after_drive.starts_with(":\\windows\\") || after_drive.starts_with(":\\program files\\") || after_drive.starts_with(":\\program files (x86)\\") {
        return Location::ProtectedFolder;
    }
    if after_drive.starts_with(":\\users\\") || after_drive.starts_with(":\\programdata\\") {
        return Location::UserFolder;
    }
    Location::Other
}

/// Decoded Windows Security Center `productState` (community-documented bit layout, UNVERIFIED against Microsoft docs):
/// the 0xF000 nibble is the product state (1 = on, 2 = snoozed, 3 = expired, 0 = off) and the 0x00F0 nibble is the
/// signature state (0x10 = out of date). Example values often quoted for Windows Defender: 0x061100 on and current,
/// 0x060100 off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductState {
    pub on: bool,
    pub snoozed_or_expired: bool,
    pub signatures_current: bool,
}

pub fn decode_product_state(state: u32) -> ProductState {
    let product = (state >> 12) & 0xF;
    ProductState { on: product == 1, snoozed_or_expired: product == 2 || product == 3, signatures_current: state & 0xF0 == 0 }
}

/// Plain-language status for a security product given its decoded state.
pub fn product_summary(p: &ProductState) -> &'static str {
    match (p.on, p.snoozed_or_expired, p.signatures_current) {
        (true, false, true) => "On, up to date",
        (true, false, false) => "On, definitions may be out of date",
        (_, true, _) => "Snoozed or expired",
        (false, false, _) => "Off",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn classifies_addresses() {
        assert_eq!(classify_ip(ip("127.0.0.1")), IpClass::Loopback);
        assert_eq!(classify_ip(ip("::1")), IpClass::Loopback);
        assert_eq!(classify_ip(ip("192.168.1.20")), IpClass::Private);
        assert_eq!(classify_ip(ip("10.0.0.5")), IpClass::Private);
        assert_eq!(classify_ip(ip("172.16.4.4")), IpClass::Private);
        assert_eq!(classify_ip(ip("172.32.0.1")), IpClass::Public);
        assert_eq!(classify_ip(ip("100.64.1.1")), IpClass::Private);
        assert_eq!(classify_ip(ip("100.128.0.1")), IpClass::Public);
        assert_eq!(classify_ip(ip("169.254.9.9")), IpClass::LinkLocal);
        assert_eq!(classify_ip(ip("fe80::1")), IpClass::LinkLocal);
        assert_eq!(classify_ip(ip("fd12:3456::1")), IpClass::Private);
        assert_eq!(classify_ip(ip("0.0.0.0")), IpClass::Unspecified);
        assert_eq!(classify_ip(ip("::")), IpClass::Unspecified);
        assert_eq!(classify_ip(ip("224.0.0.251")), IpClass::Multicast);
        assert_eq!(classify_ip(ip("8.8.8.8")), IpClass::Public);
        assert_eq!(classify_ip(ip("2606:4700::1111")), IpClass::Public);
        assert_eq!(classify_ip(ip("::ffff:192.168.0.1")), IpClass::Private);
        assert_eq!(classify_ip(ip("::ffff:8.8.8.8")), IpClass::Public);
    }

    #[test]
    fn exposure_wording() {
        assert_eq!(exposure(ip("127.0.0.1")), "this PC only");
        assert_eq!(exposure(ip("0.0.0.0")), "every network this PC is on");
        assert!(is_reachable_from_network(ip("0.0.0.0")));
        assert!(is_reachable_from_network(ip("192.168.1.5")));
        assert!(!is_reachable_from_network(ip("::1")));
    }

    #[test]
    fn command_paths() {
        assert_eq!(command_path(r#""C:\Program Files\A B\app.exe" --minimized"#).as_deref(), Some(r"C:\Program Files\A B\app.exe"));
        assert_eq!(command_path(r"C:\Tools\x.exe -s").as_deref(), Some(r"C:\Tools\x.exe"));
        assert_eq!(command_path(r"C:\Program Files\Some App\run.exe /silent").as_deref(), Some(r"C:\Program Files\Some App\run.exe"));
        assert_eq!(command_path(r"C:\a\b.CMD /c x").as_deref(), Some(r"C:\a\b.CMD"));
        assert_eq!(command_path(r#""""#), None);
        assert_eq!(command_path("   "), None);
        assert_eq!(command_path(r#""unterminated"#), None);
        assert_eq!(command_path("notepad").as_deref(), Some("notepad"));
    }

    #[test]
    fn locations() {
        assert_eq!(classify_location(r"C:\Windows\System32\x.exe"), Location::ProtectedFolder);
        assert_eq!(classify_location(r"C:\Program Files\App\a.exe"), Location::ProtectedFolder);
        assert_eq!(classify_location(r"D:\Program Files (x86)\App\a.exe"), Location::ProtectedFolder);
        assert_eq!(classify_location(r"C:\Users\me\AppData\Local\Temp\a.exe"), Location::UserFolder);
        assert_eq!(classify_location(r"C:\Users\me\Downloads\a.exe"), Location::UserFolder);
        assert_eq!(classify_location(r"C:\ProgramData\x\a.exe"), Location::UserFolder);
        assert_eq!(classify_location("%APPDATA%\\x\\a.exe"), Location::UserFolder);
        assert_eq!(classify_location("%ProgramFiles%\\x\\a.exe"), Location::ProtectedFolder);
        assert_eq!(classify_location(r"E:\tools\a.exe"), Location::Other);
        assert_eq!(classify_location("C:/Users/me/a.exe"), Location::UserFolder);
        assert_eq!(classify_location(""), Location::Other);
    }

    #[test]
    fn product_state_decoding() {
        // Values commonly reported for Windows Defender: 397568 (0x061100) on and current, 393472 (0x060100) off.
        let on = decode_product_state(0x061100);
        assert!(on.on && !on.snoozed_or_expired && on.signatures_current);
        assert_eq!(product_summary(&on), "On, up to date");
        let off = decode_product_state(0x060100);
        assert!(!off.on);
        assert_eq!(product_summary(&off), "Off");
        let old = decode_product_state(0x061110);
        assert_eq!(product_summary(&old), "On, definitions may be out of date");
        let snoozed = decode_product_state(0x062100);
        assert_eq!(product_summary(&snoozed), "Snoozed or expired");
        assert_eq!(product_summary(&decode_product_state(0x063100)), "Snoozed or expired");
    }
}
