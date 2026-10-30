//! Plinth network proxy (decision D8 in design/decisions.md): widgets never reach the network directly.
//! All WebView2 traffic is forced through this local proxy, which allows only HTTPS `CONNECT host:port` tunnels to
//! destinations that some widget declared in its manifest AND the user granted. Everything else is denied and logged.
//!
//! Why a proxy and not only CSP: a widget can make the browser open a connection by navigating its own frame, which
//! CSP does not stop (host-demo, 4 of 4 runs). The proxy sits below the browser, so even a bare connection attempt is
//! refused before any DNS lookup or connection to the destination happens.
//!
//! Per-widget attribution is NOT solved here: HTTPS tunnels carry no origin. The per-widget CSP `connect-src` (generated
//! by plinth-host) limits what each widget may try; this proxy enforces the union as a backstop.

use std::collections::{HashSet, VecDeque};
use std::io::{Read, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MAX_HEADER: usize = 8192;
const MAX_CONNECTIONS: usize = 64;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const LOG_CAP: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Dest {
    pub host: String,
    pub port: u16,
}

impl Dest {
    pub fn https(host: &str) -> Dest {
        Dest { host: host.to_ascii_lowercase(), port: 443 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    NotAllowListed,
    PlainHttp,
    BadRequest,
    SafeMode,
    PrivateAddress,
    Unsupported,
    TooManyConnections,
    UpstreamFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Deny(Reason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub seq: u64,
    pub method: String,
    pub target: String,
    pub verdict: Verdict,
}

#[derive(Default)]
pub struct Log {
    seq: u64,
    entries: VecDeque<Entry>,
}

impl Log {
    fn push(&mut self, method: &str, target: &str, verdict: Verdict) {
        self.seq += 1;
        if self.entries.len() == LOG_CAP {
            self.entries.pop_front();
        }
        let t: String = target.chars().filter(|c| !c.is_control()).take(120).collect();
        self.entries.push_back(Entry { seq: self.seq, method: method.chars().take(16).collect(), target: t, verdict });
    }
    pub fn entries(&self) -> Vec<Entry> {
        self.entries.iter().cloned().collect()
    }
}

#[derive(Default)]
pub struct Policy {
    allowed: HashSet<Dest>,
    safe: bool,
    /// Tests only: allow tunnels to loopback/private addresses. Production keeps this false so an allow-listed name
    /// that resolves to an internal address (DNS rebinding, SSRF) is refused.
    pub allow_private: bool,
}

/// Hostname or IPv4 literal. Rejects userinfo tricks, paths, spaces, IPv6 brackets and anything unusual.
fn parse_host(h: &str) -> Option<String> {
    let h = h.trim_end_matches('.').to_ascii_lowercase();
    if h.is_empty() || h.len() > 253 || !h.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
        return None;
    }
    if h.starts_with('.') || h.starts_with('-') || h.contains("..") {
        return None;
    }
    Some(h)
}

pub fn parse_connect_target(t: &str) -> Option<Dest> {
    let (h, p) = t.rsplit_once(':')?;
    let port: u16 = p.parse().ok().filter(|p| *p != 0)?;
    Some(Dest { host: parse_host(h)?, port })
}

impl Policy {
    pub fn new() -> Self {
        Policy::default()
    }
    pub fn set_allowed(&mut self, v: Vec<Dest>) {
        self.allowed = v.into_iter().collect();
    }
    pub fn set_safe_mode(&mut self, on: bool) {
        self.safe = on;
    }
    pub fn decide_connect(&self, target: &str) -> (Verdict, Option<Dest>) {
        if self.safe {
            return (Verdict::Deny(Reason::SafeMode), None);
        }
        let Some(d) = parse_connect_target(target) else { return (Verdict::Deny(Reason::BadRequest), None) };
        if self.allowed.contains(&d) {
            (Verdict::Allow, Some(d))
        } else {
            (Verdict::Deny(Reason::NotAllowListed), Some(d))
        }
    }
}

fn is_private(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_loopback() || v.is_private() || v.is_link_local() || v.is_unspecified() || v.is_broadcast() || v.octets()[0] == 0 || (v.octets()[0] == 100 && (64..128).contains(&v.octets()[1])),
        IpAddr::V6(v) => v.is_loopback() || v.is_unspecified() || (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80 || v.to_ipv4_mapped().map_or(false, |m| is_private(&IpAddr::V4(m))),
    }
}

fn respond(s: &mut TcpStream, code: u16, text: &str, reason: &str) {
    let body = format!("HTTP/1.1 {code} {text}\r\nContent-Length: 0\r\nConnection: close\r\nX-Plinth: {reason}\r\n\r\n");
    let _ = s.write_all(body.as_bytes());
    let _ = s.shutdown(Shutdown::Both);
}

fn read_head(s: &mut TcpStream) -> Result<String, Reason> {
    let mut buf = Vec::with_capacity(512);
    let mut b = [0u8; 512];
    loop {
        let n = s.read(&mut b).map_err(|_| Reason::BadRequest)?;
        if n == 0 {
            return Err(Reason::BadRequest);
        }
        buf.extend_from_slice(&b[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if buf.len() > MAX_HEADER {
            return Err(Reason::BadRequest);
        }
    }
    String::from_utf8(buf).map_err(|_| Reason::BadRequest)
}

fn tunnel(client: TcpStream, upstream: TcpStream) {
    let _ = client.set_read_timeout(Some(IDLE_TIMEOUT));
    let _ = upstream.set_read_timeout(Some(IDLE_TIMEOUT));
    let (mut c1, mut u1) = match (client.try_clone(), upstream.try_clone()) {
        (Ok(a), Ok(b)) => (a, b),
        _ => return,
    };
    let (mut c2, mut u2) = (client, upstream);
    let t = std::thread::spawn(move || {
        let _ = std::io::copy(&mut c1, &mut u1);
        let _ = u1.shutdown(Shutdown::Write);
    });
    let _ = std::io::copy(&mut u2, &mut c2);
    let _ = c2.shutdown(Shutdown::Write);
    let _ = t.join();
}

fn handle(mut s: TcpStream, policy: Arc<Mutex<Policy>>, log: Arc<Mutex<Log>>) {
    let _ = s.set_read_timeout(Some(IO_TIMEOUT));
    let head = match read_head(&mut s) {
        Ok(h) => h,
        Err(r) => {
            // Browsers open speculative connections (preconnect) that send nothing; label them so the log stays readable.
            log.lock().unwrap().push("?", "<empty or unreadable connection>", Verdict::Deny(r));
            return respond(&mut s, 400, "Bad Request", "bad-request");
        }
    };
    let first = head.lines().next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let (method, target, version) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    if !version.starts_with("HTTP/1.") || method.is_empty() || target.is_empty() {
        log.lock().unwrap().push(method, target, Verdict::Deny(Reason::BadRequest));
        return respond(&mut s, 400, "Bad Request", "bad-request");
    }
    if !method.eq_ignore_ascii_case("CONNECT") {
        let r = if target.to_ascii_lowercase().starts_with("http://") { Reason::PlainHttp } else { Reason::Unsupported };
        log.lock().unwrap().push(method, target, Verdict::Deny(r));
        return respond(&mut s, 403, "Forbidden", if r == Reason::PlainHttp { "plain-http-not-allowed" } else { "unsupported" });
    }
    let (verdict, dest) = policy.lock().unwrap().decide_connect(target);
    let allow_private = policy.lock().unwrap().allow_private;
    if let Verdict::Deny(r) = verdict {
        log.lock().unwrap().push(method, target, verdict);
        return respond(&mut s, if r == Reason::BadRequest { 400 } else { 403 }, "Forbidden", "not-allow-listed");
    }
    let d = dest.unwrap();
    // Resolve ourselves and refuse internal addresses even for allow-listed names.
    let addrs: Vec<SocketAddr> = match (d.host.as_str(), d.port).to_socket_addrs() {
        Ok(a) => a.collect(),
        Err(_) => {
            log.lock().unwrap().push(method, target, Verdict::Deny(Reason::UpstreamFailed));
            return respond(&mut s, 502, "Bad Gateway", "upstream-failed");
        }
    };
    let Some(addr) = addrs.iter().find(|a| allow_private || !is_private(&a.ip())) else {
        log.lock().unwrap().push(method, target, Verdict::Deny(Reason::PrivateAddress));
        return respond(&mut s, 403, "Forbidden", "private-address");
    };
    match TcpStream::connect_timeout(addr, IO_TIMEOUT) {
        Ok(up) => {
            log.lock().unwrap().push(method, target, Verdict::Allow);
            if s.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").is_ok() {
                tunnel(s, up);
            }
        }
        Err(_) => {
            log.lock().unwrap().push(method, target, Verdict::Deny(Reason::UpstreamFailed));
            respond(&mut s, 502, "Bad Gateway", "upstream-failed");
        }
    }
}

/// Starts the proxy on 127.0.0.1 (random port). Returns the port.
pub fn start(policy: Arc<Mutex<Policy>>, log: Arc<Mutex<Log>>) -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let active = Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(s) = conn else { continue };
            let (policy, log, active) = (policy.clone(), log.clone(), active.clone());
            if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                active.fetch_sub(1, Ordering::SeqCst);
                let mut s = s;
                log.lock().unwrap().push("?", "<over capacity>", Verdict::Deny(Reason::TooManyConnections));
                respond(&mut s, 503, "Service Unavailable", "too-many-connections");
                continue;
            }
            std::thread::spawn(move || {
                handle(s, policy, log);
                active.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};

    fn pol(allowed: &[(&str, u16)]) -> Policy {
        let mut p = Policy::new();
        p.set_allowed(allowed.iter().map(|(h, p)| Dest { host: h.to_string(), port: *p }).collect());
        p
    }

    #[test]
    fn parses_connect_targets_strictly() {
        assert_eq!(parse_connect_target("API.Weather.example:443"), Some(Dest::https("api.weather.example")));
        assert_eq!(parse_connect_target("host.example.:443"), Some(Dest::https("host.example")), "trailing dot normalized");
        for bad in ["host.example", "host.example:0", "host.example:99999", ":443", "a b.example:443", "user@evil.example:443", "evil.example/x:443", "[::1]:443", "..example:443", "-x.example:443", "a..b:443", "host.example:abc", ""] {
            assert_eq!(parse_connect_target(bad), None, "{bad}");
        }
    }

    #[test]
    fn policy_is_deny_by_default_exact_and_port_aware() {
        let p = pol(&[("api.weather.example", 443)]);
        assert_eq!(p.decide_connect("api.weather.example:443").0, Verdict::Allow);
        assert_eq!(p.decide_connect("API.WEATHER.EXAMPLE:443").0, Verdict::Allow);
        assert_eq!(p.decide_connect("api.weather.example:8443").0, Verdict::Deny(Reason::NotAllowListed), "other port");
        assert_eq!(p.decide_connect("evil.example:443").0, Verdict::Deny(Reason::NotAllowListed));
        assert_eq!(p.decide_connect("x.api.weather.example:443").0, Verdict::Deny(Reason::NotAllowListed), "no implicit subdomains");
        assert_eq!(p.decide_connect("api.weather.example.evil.example:443").0, Verdict::Deny(Reason::NotAllowListed));
        assert_eq!(p.decide_connect("garbage").0, Verdict::Deny(Reason::BadRequest));
        assert_eq!(Policy::new().decide_connect("api.weather.example:443").0, Verdict::Deny(Reason::NotAllowListed), "empty list denies all");
    }

    #[test]
    fn safe_mode_denies_even_allow_listed() {
        let mut p = pol(&[("api.weather.example", 443)]);
        p.set_safe_mode(true);
        assert_eq!(p.decide_connect("api.weather.example:443").0, Verdict::Deny(Reason::SafeMode));
        p.set_safe_mode(false);
        assert_eq!(p.decide_connect("api.weather.example:443").0, Verdict::Allow);
    }

    #[test]
    fn private_address_classifier() {
        for ip in ["127.0.0.1", "10.1.2.3", "192.168.0.5", "172.16.0.1", "169.254.1.1", "0.0.0.0", "100.64.0.1", "::1", "fe80::1", "fc00::1", "::ffff:10.0.0.1"] {
            assert!(is_private(&ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "93.184.216.34", "2606:4700:4700::1111"] {
            assert!(!is_private(&ip.parse().unwrap()), "{ip}");
        }
    }

    fn request(port: u16, raw: &str) -> String {
        let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
        c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        c.write_all(raw.as_bytes()).unwrap();
        let mut r = BufReader::new(c);
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        line
    }

    fn setup(allow: Vec<Dest>, allow_private: bool) -> (u16, Arc<Mutex<Policy>>, Arc<Mutex<Log>>) {
        let mut p = Policy::new();
        p.set_allowed(allow);
        p.allow_private = allow_private;
        let policy = Arc::new(Mutex::new(p));
        let log = Arc::new(Mutex::new(Log::default()));
        let port = start(policy.clone(), log.clone()).unwrap();
        (port, policy, log)
    }

    #[test]
    fn denies_plain_http_other_methods_garbage_and_oversized_requests() {
        let (port, _p, log) = setup(vec![], false);
        assert!(request(port, "GET http://127.0.0.1:9/probe HTTP/1.1\r\nHost: x\r\n\r\n").contains("403"));
        assert!(request(port, "POST /x HTTP/1.1\r\n\r\n").contains("403"));
        assert!(request(port, "CONNECT evil.example:443 HTTP/1.1\r\n\r\n").contains("403"));
        assert!(request(port, "nonsense\r\n\r\n").contains("400"));
        assert!(request(port, &format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(9000))).contains("400"));
        let l = log.lock().unwrap().entries();
        assert!(l.iter().all(|e| matches!(e.verdict, Verdict::Deny(_))) && l.len() >= 5, "{l:?}");
        assert!(l.iter().any(|e| e.verdict == Verdict::Deny(Reason::PlainHttp)));
    }

    #[test]
    fn tunnels_only_to_allow_listed_destination_and_logs() {
        // Upstream echo server on loopback, allowed explicitly (allow_private is for tests only).
        let up = TcpListener::bind("127.0.0.1:0").unwrap();
        let up_port = up.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for c in up.incoming() {
                let mut c = c.unwrap();
                std::thread::spawn(move || {
                    let mut b = [0u8; 64];
                    while let Ok(n) = c.read(&mut b) {
                        if n == 0 || c.write_all(&b[..n]).is_err() {
                            break;
                        }
                    }
                });
            }
        });
        let (port, _p, log) = setup(vec![Dest { host: "127.0.0.1".into(), port: up_port }], true);
        let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
        c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        c.write_all(format!("CONNECT 127.0.0.1:{up_port} HTTP/1.1\r\nHost: 127.0.0.1:{up_port}\r\n\r\n").as_bytes()).unwrap();
        let mut head = [0u8; 39];
        c.read_exact(&mut head).unwrap();
        assert!(String::from_utf8_lossy(&head).starts_with("HTTP/1.1 200 Connection Established"));
        c.write_all(b"ping").unwrap();
        let mut echo = [0u8; 4];
        c.read_exact(&mut echo).unwrap();
        assert_eq!(&echo, b"ping");
        // a different port on the same host is not allowed
        assert!(request(port, &format!("CONNECT 127.0.0.1:{} HTTP/1.1\r\n\r\n", up_port.wrapping_add(1).max(2))).contains("403"));
        let l = log.lock().unwrap().entries();
        assert!(l.iter().any(|e| e.verdict == Verdict::Allow) && l.iter().any(|e| e.verdict == Verdict::Deny(Reason::NotAllowListed)));
    }

    #[test]
    fn allow_listed_name_resolving_to_internal_address_is_refused() {
        // "localhost" resolves to loopback; with allow_private=false the tunnel must be refused (SSRF / DNS rebinding guard).
        let (port, _p, log) = setup(vec![Dest { host: "localhost".into(), port: 443 }], false);
        assert!(request(port, "CONNECT localhost:443 HTTP/1.1\r\n\r\n").contains("403"));
        assert_eq!(log.lock().unwrap().entries()[0].verdict, Verdict::Deny(Reason::PrivateAddress));
    }

    #[test]
    fn grant_changes_apply_to_the_next_request() {
        let (port, policy, _l) = setup(vec![], true);
        assert!(request(port, "CONNECT 127.0.0.1:1 HTTP/1.1\r\n\r\n").contains("403"));
        policy.lock().unwrap().set_safe_mode(true);
        assert!(request(port, "CONNECT 127.0.0.1:1 HTTP/1.1\r\n\r\n").contains("403"));
    }
}
