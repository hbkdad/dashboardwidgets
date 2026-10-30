//! Security Center collectors (Windows, user mode, read-only, raw FFI like win.rs and hw.rs).
//!
//! Visibility only: firewall switches from the local policy registry values, antivirus products and Defender status from
//! Windows Security Center via one fixed PowerShell command, TCP connections and listeners from the IP Helper API, and
//! startup programs from the Run keys and Startup folders. Nothing here changes the system, and nothing claims to detect
//! malware. Everything collected is untrusted text (process names, command lines): the hub escapes all of it.

use plinth_core::security as sc;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MAX_ROWS: usize = 400;
const MAX_TEXT: usize = 260;

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(hkey: isize, sub: *const u16, opts: u32, sam: u32, out: *mut isize) -> i32;
    fn RegQueryValueExW(hkey: isize, name: *const u16, reserved: *mut u32, ty: *mut u32, data: *mut u8, len: *mut u32) -> i32;
    fn RegEnumValueW(hkey: isize, idx: u32, name: *mut u16, name_len: *mut u32, reserved: *mut u32, ty: *mut u32, data: *mut u8, data_len: *mut u32) -> i32;
    fn RegCloseKey(hkey: isize) -> i32;
}
#[link(name = "iphlpapi")]
extern "system" {
    fn GetExtendedTcpTable(table: *mut u8, size: *mut u32, order: i32, af: u32, class: i32, reserved: u32) -> u32;
}
#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
    fn CloseHandle(h: isize) -> i32;
    fn QueryFullProcessImageNameW(h: isize, flags: u32, name: *mut u16, len: *mut u32) -> i32;
    fn ExpandEnvironmentStringsW(src: *const u16, dst: *mut u16, size: u32) -> u32;
}

const HKCU: isize = 0x8000_0001u32 as isize;
const HKLM: isize = 0x8000_0002u32 as isize;
const KEY_READ: u32 = 0x20019;
const REG_SZ: u32 = 1;
const REG_EXPAND_SZ: u32 = 2;
const REG_DWORD: u32 = 4;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn clip(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(MAX_TEXT).collect()
}

fn open(root: isize, sub: &str) -> Option<isize> {
    let mut h = 0isize;
    (unsafe { RegOpenKeyExW(root, w(sub).as_ptr(), 0, KEY_READ, &mut h) } == 0).then_some(h)
}

fn reg_dword(root: isize, sub: &str, name: &str) -> Option<u32> {
    let h = open(root, sub)?;
    let (mut ty, mut v, mut len) = (0u32, 0u32, 4u32);
    let ok = unsafe { RegQueryValueExW(h, w(name).as_ptr(), std::ptr::null_mut(), &mut ty, &mut v as *mut u32 as *mut u8, &mut len) } == 0 && ty == REG_DWORD;
    unsafe { RegCloseKey(h) };
    ok.then_some(v)
}

fn expand(s: &str) -> String {
    let src = w(s);
    let mut buf = vec![0u16; 2048];
    let n = unsafe { ExpandEnvironmentStringsW(src.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if n == 0 || n > buf.len() {
        return s.to_string();
    }
    String::from_utf16_lossy(&buf[..n.saturating_sub(1)])
}

/// All string values under a key: (name, expanded value).
fn reg_values(root: isize, sub: &str) -> Vec<(String, String)> {
    let Some(h) = open(root, sub) else { return Vec::new() };
    let mut out = Vec::new();
    for idx in 0..256u32 {
        let mut name = vec![0u16; 512];
        let mut data = vec![0u8; 4096];
        let (mut nl, mut dl, mut ty) = (name.len() as u32, data.len() as u32, 0u32);
        let r = unsafe { RegEnumValueW(h, idx, name.as_mut_ptr(), &mut nl, std::ptr::null_mut(), &mut ty, data.as_mut_ptr(), &mut dl) };
        if r != 0 {
            break; // ERROR_NO_MORE_ITEMS or an error: stop either way
        }
        if ty != REG_SZ && ty != REG_EXPAND_SZ {
            continue;
        }
        let units: Vec<u16> = data[..dl as usize].chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).take_while(|&c| c != 0).collect();
        let raw = String::from_utf16_lossy(&units);
        out.push((String::from_utf16_lossy(&name[..nl as usize]), if ty == REG_EXPAND_SZ { expand(&raw) } else { raw }));
    }
    unsafe { RegCloseKey(h) };
    out
}

// ---------------------------------------------------------------- firewall, antivirus, Defender

fn firewall() -> Value {
    let base = r"SYSTEM\CurrentControlSet\Services\SharedAccess\Parameters\FirewallPolicy";
    let rows: Vec<Value> = [("Domain network", "DomainProfile"), ("Private network", "StandardProfile"), ("Public network", "PublicProfile")]
        .iter()
        .map(|(label, key)| {
            let v = reg_dword(HKLM, &format!(r"{base}\{key}"), "EnableFirewall");
            json!({ "profile": label, "on": v.map(|x| x != 0) })
        })
        .collect();
    json!(rows)
}

/// One fixed, read-only PowerShell command (no user input is ever interpolated). Returns None if PowerShell is missing,
/// fails or takes longer than the timeout.
fn security_center() -> Option<Value> {
    const SCRIPT: &str = "$ErrorActionPreference='SilentlyContinue'; \
        $av=@(Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntiVirusProduct | ForEach-Object { @{name=$_.displayName; state=[int]$_.productState} }); \
        $mp=Get-MpComputerStatus; $d=$null; \
        if($mp){ $d=@{av=$mp.AntivirusEnabled; rtp=$mp.RealTimeProtectionEnabled; sigAgeDays=$mp.AntivirusSignatureAge; tamper=$mp.IsTamperProtected; service=$mp.AMServiceEnabled} }; \
        @{av=$av; defender=$d} | ConvertTo-Json -Depth 4 -Compress";
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let exe = root.join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    if !exe.is_file() {
        return None;
    }
    let mut child = std::process::Command::new(exe)
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < Duration::from_secs(20) => std::thread::sleep(Duration::from_millis(100)),
            _ => {
                let _ = child.kill();
                return None;
            }
        }
    }
    let out = child.wait_with_output().ok()?;
    serde_json::from_slice::<Value>(&out.stdout).ok()
}

fn antivirus(sc_json: &Option<Value>) -> (Value, Value) {
    let Some(v) = sc_json else { return (Value::Null, Value::Null) };
    let list = match v.get("av") {
        Some(Value::Array(a)) => a.clone(),
        Some(o @ Value::Object(_)) => vec![o.clone()],
        _ => Vec::new(),
    };
    let products: Vec<Value> = list
        .iter()
        .filter_map(|p| {
            let name = p.get("name")?.as_str()?;
            let st = sc::decode_product_state(p.get("state")?.as_i64()? as u32);
            Some(json!({ "name": clip(name), "summary": sc::product_summary(&st), "on": st.on }))
        })
        .collect();
    let defender = v.get("defender").cloned().unwrap_or(Value::Null);
    (json!(products), defender)
}

// ---------------------------------------------------------------- connections

fn state_name(s: u32) -> &'static str {
    match s {
        1 => "closed",
        2 => "listening",
        3 => "connecting",
        4 => "connecting",
        5 => "established",
        6 | 7 => "closing",
        8 => "close-wait",
        9 | 10 => "closing",
        11 => "time-wait",
        _ => "other",
    }
}

fn port(raw: u32) -> u16 {
    let p = (raw & 0xFFFF) as u16;
    p.swap_bytes()
}

struct Sock {
    local: IpAddr,
    local_port: u16,
    remote: IpAddr,
    remote_port: u16,
    state: u32,
    pid: u32,
}

fn tcp_table(af: u32) -> Vec<Sock> {
    let mut size = 0u32;
    unsafe { GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, af, 5, 0) };
    for _ in 0..4 {
        let mut buf = vec![0u8; size as usize + 4096];
        size = buf.len() as u32;
        let r = unsafe { GetExtendedTcpTable(buf.as_mut_ptr(), &mut size, 0, af, 5, 0) };
        if r == 122 {
            continue; // ERROR_INSUFFICIENT_BUFFER: the table grew, size now holds the needed length
        }
        if r != 0 || buf.len() < 4 {
            return Vec::new();
        }
        let n = u32::from_le_bytes(buf[0..4].try_into().unwrap()) as usize;
        let u32_at = |o: usize| u32::from_le_bytes(buf[o..o + 4].try_into().unwrap());
        let mut out = Vec::new();
        if af == 2 {
            // MIB_TCPROW_OWNER_PID: state, localAddr, localPort, remoteAddr, remotePort, owningPid (24 bytes)
            for i in 0..n.min(20000) {
                let o = 4 + i * 24;
                if o + 24 > buf.len() {
                    break;
                }
                out.push(Sock {
                    state: u32_at(o),
                    local: IpAddr::V4(Ipv4Addr::from(u32_at(o + 4).to_le_bytes())),
                    local_port: port(u32_at(o + 8)),
                    remote: IpAddr::V4(Ipv4Addr::from(u32_at(o + 12).to_le_bytes())),
                    remote_port: port(u32_at(o + 16)),
                    pid: u32_at(o + 20),
                });
            }
        } else {
            // MIB_TCP6ROW_OWNER_PID: localAddr[16], localScope, localPort, remoteAddr[16], remoteScope, remotePort, state, pid (56 bytes)
            for i in 0..n.min(20000) {
                let o = 4 + i * 56;
                if o + 56 > buf.len() {
                    break;
                }
                let a16 = |at: usize| -> [u8; 16] { buf[at..at + 16].try_into().unwrap() };
                out.push(Sock {
                    local: IpAddr::V6(Ipv6Addr::from(a16(o))),
                    local_port: port(u32_at(o + 20)),
                    remote: IpAddr::V6(Ipv6Addr::from(a16(o + 24))),
                    remote_port: port(u32_at(o + 44)),
                    state: u32_at(o + 48),
                    pid: u32_at(o + 52),
                });
            }
        }
        return out;
    }
    Vec::new()
}

fn process_path(pid: u32) -> Option<String> {
    if pid == 0 || pid == 4 {
        return None;
    }
    unsafe {
        let h = OpenProcess(0x1000, 0, pid); // PROCESS_QUERY_LIMITED_INFORMATION
        if h == 0 {
            return None;
        }
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len) != 0;
        CloseHandle(h);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

fn file_name(p: &str) -> String {
    p.rsplit(['\\', '/']).next().unwrap_or(p).to_string()
}

fn connections() -> (Vec<Value>, Vec<Value>, usize) {
    let mut socks = tcp_table(2);
    socks.extend(tcp_table(23));
    let total = socks.len();
    let mut names: HashMap<u32, (String, String)> = HashMap::new();
    let mut who = |pid: u32| -> (String, String) {
        names
            .entry(pid)
            .or_insert_with(|| match pid {
                0 => ("(no owning process)".into(), String::new()),
                4 => ("System".into(), String::new()),
                _ => match process_path(pid) {
                    Some(p) => (clip(&file_name(&p)), clip(&p)),
                    None => (format!("process {pid} (not readable)"), String::new()),
                },
            })
            .clone()
    };
    let (mut conns, mut listeners) = (Vec::new(), Vec::new());
    for s in &socks {
        if s.state == 5 && !matches!(sc::classify_ip(s.remote), sc::IpClass::Loopback | sc::IpClass::Unspecified) && conns.len() < MAX_ROWS {
            let (name, path) = who(s.pid);
            conns.push(json!({ "proc": name, "path": path, "pid": s.pid, "remote": format!("{}:{}", s.remote, s.remote_port), "class": sc::classify_ip(s.remote).label(), "internet": sc::classify_ip(s.remote) == sc::IpClass::Public }));
        } else if s.state == 2 && listeners.len() < MAX_ROWS {
            let (name, path) = who(s.pid);
            listeners.push(json!({ "proc": name, "path": path, "pid": s.pid, "port": s.local_port, "bound": s.local.to_string(), "exposure": sc::exposure(s.local), "reachable": sc::is_reachable_from_network(s.local) }));
        }
    }
    let _ = state_name; // kept for diagnostics output
    conns.sort_by(|a, b| a["proc"].as_str().cmp(&b["proc"].as_str()));
    listeners.sort_by(|a, b| a["port"].as_u64().cmp(&b["port"].as_u64()));
    (conns, listeners, total)
}

// ---------------------------------------------------------------- startup items

fn startup_item(name: &str, command: &str, scope: &str, source: &str) -> Value {
    let path = sc::command_path(command).map(|p| expand(&p)).unwrap_or_default();
    let exists = !path.is_empty() && Path::new(&path).exists();
    let loc = sc::classify_location(&path);
    json!({ "name": clip(name), "command": clip(command), "path": clip(&path), "exists": exists, "scope": scope, "source": source, "where": loc.label(), "userFolder": loc == sc::Location::UserFolder })
}

fn startup() -> Vec<Value> {
    let mut out = Vec::new();
    let run = r"Software\Microsoft\Windows\CurrentVersion\Run";
    let once = r"Software\Microsoft\Windows\CurrentVersion\RunOnce";
    for (root, scope) in [(HKCU, "Current user"), (HKLM, "All users")] {
        for (key, label) in [(run, "Run key"), (once, "RunOnce key")] {
            for (n, v) in reg_values(root, key) {
                out.push(startup_item(&n, &v, scope, label));
            }
        }
    }
    for (n, v) in reg_values(HKLM, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run") {
        out.push(startup_item(&n, &v, "All users (32-bit)", "Run key"));
    }
    let dirs = [
        (std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs\Startup")), "Current user"),
        (std::env::var_os("ProgramData").map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs\StartUp")), "All users"),
    ];
    for (dir, scope) in dirs {
        let Some(dir) = dir else { continue };
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.filter_map(|e| e.ok()).take(100) {
            let p = e.path();
            if p.file_name().map_or(false, |n| n.to_string_lossy().eq_ignore_ascii_case("desktop.ini")) {
                continue;
            }
            out.push(startup_item(&e.file_name().to_string_lossy(), &p.to_string_lossy(), scope, "Startup folder"));
        }
    }
    out.truncate(MAX_ROWS);
    out
}

/// Gather everything. Runs on a worker thread: the PowerShell step can take a few seconds.
pub fn gather() -> Value {
    let started = Instant::now();
    let sc_json = security_center();
    let (products, defender) = antivirus(&sc_json);
    let (conns, listeners, total) = connections();
    json!({
        "firewall": firewall(),
        "av": products,
        "defender": defender,
        "scOk": sc_json.is_some(),
        "connections": conns,
        "listeners": listeners,
        "socketsTotal": total,
        "startup": startup(),
        "tookMs": started.elapsed().as_millis() as u64,
    })
}
