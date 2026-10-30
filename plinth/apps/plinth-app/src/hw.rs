//! Hardware sampler (decision D17: no kernel driver of our own). One background thread reads user-mode sources once per
//! tick and shares a snapshot with every widget, so cost does not grow with the number of widgets:
//! * CPU utilization: GetSystemTimes deltas
//! * RAM: GlobalMemoryStatusEx
//! * Network and disk throughput, CPU frequency, GPU engine utilization: Windows PDH performance counters
//! * NVIDIA GPU temperature, power, fan, clocks, VRAM: NVML (`nvml.dll`, loaded only from System32 or the fixed NVSMI path,
//!   never from PATH or the current directory)
//! * Parts list: registry
//! Not available in user mode on the tested PC (OBSERVED): CPU package temperature and fan RPM. They are reported as absent,
//! never invented.

use plinth_core::PermissionId;
use plinth_host::DataProvider;
use serde_json::{json, Value};
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Handle = isize;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryExW(name: *const u16, file: isize, flags: u32) -> Handle;
    fn GetProcAddress(lib: Handle, name: *const c_char) -> *const c_void;
    fn GetSystemTimes(idle: *mut u64, kernel: *mut u64, user: *mut u64) -> i32;
    fn GlobalMemoryStatusEx(buf: *mut MemoryStatus) -> i32;
}
#[link(name = "advapi32")]
extern "system" {
    fn RegGetValueW(key: isize, sub: *const u16, value: *const u16, flags: u32, typ: *mut u32, data: *mut c_void, size: *mut u32) -> i32;
}
#[link(name = "pdh")]
extern "system" {
    fn PdhOpenQueryW(src: *const u16, user: usize, q: *mut Handle) -> u32;
    fn PdhAddEnglishCounterW(q: Handle, path: *const u16, user: usize, c: *mut Handle) -> u32;
    fn PdhCollectQueryData(q: Handle) -> u32;
    fn PdhGetFormattedCounterValue(c: Handle, fmt: u32, typ: *mut u32, v: *mut FmtValue) -> u32;
    fn PdhGetFormattedCounterArrayW(c: Handle, fmt: u32, size: *mut u32, count: *mut u32, buf: *mut u8) -> u32;
}

#[repr(C)]
struct MemoryStatus {
    length: u32,
    load: u32,
    total_phys: u64,
    avail_phys: u64,
    total_page: u64,
    avail_page: u64,
    total_virt: u64,
    avail_virt: u64,
    avail_ext: u64,
}
#[repr(C)]
struct FmtValue {
    status: u32,
    _pad: u32,
    value: f64,
}
#[repr(C)]
struct FmtItem {
    name: *const u16,
    status: u32,
    _pad: u32,
    value: f64,
}

const PDH_FMT_DOUBLE: u32 = 0x200;
const PDH_MORE_DATA: u32 = 0x8000_07D2;
const HKLM: isize = 0x8000_0002u32 as isize;

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn reg_string(sub: &str, value: &str) -> Option<String> {
    unsafe {
        let mut size = 0u32;
        if RegGetValueW(HKLM, w(sub).as_ptr(), w(value).as_ptr(), 2, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) != 0 || size == 0 || size > 4096 {
            return None;
        }
        let mut buf = vec![0u16; (size as usize) / 2 + 1];
        if RegGetValueW(HKLM, w(sub).as_ptr(), w(value).as_ptr(), 2, std::ptr::null_mut(), buf.as_mut_ptr() as *mut c_void, &mut size) != 0 {
            return None;
        }
        let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]).trim().to_string())
    }
}

// ---------------- PDH ----------------
struct Pdh {
    query: Handle,
}
struct Counter(Handle);

impl Pdh {
    fn new() -> Option<Pdh> {
        let mut q = 0;
        (unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut q) } == 0).then_some(Pdh { query: q })
    }
    fn add(&self, path: &str) -> Option<Counter> {
        let mut c = 0;
        (unsafe { PdhAddEnglishCounterW(self.query, w(path).as_ptr(), 0, &mut c) } == 0).then_some(Counter(c))
    }
    fn collect(&self) {
        unsafe { PdhCollectQueryData(self.query) };
    }
}
impl Counter {
    fn value(&self) -> Option<f64> {
        let mut v = FmtValue { status: 0, _pad: 0, value: 0.0 };
        let r = unsafe { PdhGetFormattedCounterValue(self.0, PDH_FMT_DOUBLE, std::ptr::null_mut(), &mut v) };
        (r == 0 && v.status == 0).then_some(v.value)
    }
    /// All instances as (name, value).
    fn array(&self) -> Vec<(String, f64)> {
        unsafe {
            let (mut size, mut count) = (0u32, 0u32);
            if PdhGetFormattedCounterArrayW(self.0, PDH_FMT_DOUBLE, &mut size, &mut count, std::ptr::null_mut()) != PDH_MORE_DATA || size == 0 || size > 4 * 1024 * 1024 {
                return vec![];
            }
            let mut buf = vec![0u8; size as usize];
            if PdhGetFormattedCounterArrayW(self.0, PDH_FMT_DOUBLE, &mut size, &mut count, buf.as_mut_ptr()) != 0 {
                return vec![];
            }
            let items = buf.as_ptr() as *const FmtItem;
            (0..count as usize)
                .filter_map(|i| {
                    let it = &*items.add(i);
                    if it.status != 0 || it.name.is_null() {
                        return None;
                    }
                    let mut len = 0;
                    while len < 512 && *it.name.add(len) != 0 {
                        len += 1;
                    }
                    Some((String::from_utf16_lossy(std::slice::from_raw_parts(it.name, len)), it.value))
                })
                .collect()
        }
    }
}

// ---------------- NVML ----------------
#[repr(C)]
#[derive(Default)]
struct NvmlUtil {
    gpu: u32,
    memory: u32,
}
#[repr(C)]
#[derive(Default)]
struct NvmlMem {
    total: u64,
    free: u64,
    used: u64,
}

struct Nvml {
    device: *mut c_void,
    temp: unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> i32,
    util: unsafe extern "C" fn(*mut c_void, *mut NvmlUtil) -> i32,
    power: unsafe extern "C" fn(*mut c_void, *mut u32) -> i32,
    fan: unsafe extern "C" fn(*mut c_void, *mut u32) -> i32,
    clock: unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> i32,
    mem: unsafe extern "C" fn(*mut c_void, *mut NvmlMem) -> i32,
    name: String,
}
unsafe impl Send for Nvml {}

impl Nvml {
    fn load() -> Option<Nvml> {
        unsafe {
            // Only trusted locations: System32 (flag 0x800) or the legacy fixed NVSMI path. Never PATH or the current directory.
            let mut lib = LoadLibraryExW(w("nvml.dll").as_ptr(), 0, 0x0000_0800);
            if lib == 0 {
                let legacy = w("C:\\Program Files\\NVIDIA Corporation\\NVSMI\\nvml.dll");
                lib = LoadLibraryExW(legacy.as_ptr(), 0, 0x0000_0008);
            }
            if lib == 0 {
                return None;
            }
            macro_rules! sym {
                ($n:expr) => {{
                    let p = GetProcAddress(lib, concat!($n, "\0").as_ptr() as *const c_char);
                    if p.is_null() {
                        return None;
                    }
                    std::mem::transmute(p)
                }};
            }
            let init: unsafe extern "C" fn() -> i32 = sym!("nvmlInit_v2");
            let handle_by_index: unsafe extern "C" fn(u32, *mut *mut c_void) -> i32 = sym!("nvmlDeviceGetHandleByIndex_v2");
            let get_name: unsafe extern "C" fn(*mut c_void, *mut c_char, u32) -> i32 = sym!("nvmlDeviceGetName");
            if init() != 0 {
                return None;
            }
            let mut device: *mut c_void = std::ptr::null_mut();
            if handle_by_index(0, &mut device) != 0 || device.is_null() {
                return None;
            }
            let mut name = [0 as c_char; 96];
            let name = if get_name(device, name.as_mut_ptr(), 96) == 0 { CStr::from_ptr(name.as_ptr()).to_string_lossy().into_owned() } else { "NVIDIA GPU".into() };
            Some(Nvml { device, temp: sym!("nvmlDeviceGetTemperature"), util: sym!("nvmlDeviceGetUtilizationRates"), power: sym!("nvmlDeviceGetPowerUsage"), fan: sym!("nvmlDeviceGetFanSpeed"), clock: sym!("nvmlDeviceGetClockInfo"), mem: sym!("nvmlDeviceGetMemoryInfo"), name })
        }
    }

    fn read(&self) -> Value {
        unsafe {
            let opt = |r: i32, v: u32| (r == 0).then_some(v);
            let mut t = 0u32;
            let temp = opt((self.temp)(self.device, 0, &mut t), t);
            let mut u = NvmlUtil::default();
            let util = ((self.util)(self.device, &mut u) == 0).then_some((u.gpu, u.memory));
            let mut p = 0u32;
            let power = opt((self.power)(self.device, &mut p), p);
            let mut f = 0u32;
            let fan = opt((self.fan)(self.device, &mut f), f);
            let mut c = 0u32;
            let clock = opt((self.clock)(self.device, 0, &mut c), c);
            let mut mc = 0u32;
            let mem_clock = opt((self.clock)(self.device, 2, &mut mc), mc);
            let mut m = NvmlMem::default();
            let mem = ((self.mem)(self.device, &mut m) == 0).then_some((m.used, m.total));
            json!({
                "name": self.name, "source": "nvml",
                "util_pct": util.map(|x| x.0), "mem_ctrl_pct": util.map(|x| x.1),
                "temp_c": temp, "power_w": power.map(|mw| (mw as f64 / 100.0).round() / 10.0),
                "fan_pct": fan, "clock_mhz": clock, "mem_clock_mhz": mem_clock,
                "vram_used_mb": mem.map(|x| x.0 / (1024 * 1024)), "vram_total_mb": mem.map(|x| x.1 / (1024 * 1024)),
            })
        }
    }
}

// ---------------- sampler ----------------
#[derive(Default, Clone)]
pub struct Snapshot {
    pub cpu: Option<Value>,
    pub ram: Option<Value>,
    pub gpu: Option<Value>,
    pub disk: Option<Value>,
    pub network: Option<Value>,
    pub info: Option<Value>,
    pub samples: u64,
}

pub struct Sampler {
    snap: Arc<Mutex<Snapshot>>,
    active: Arc<AtomicBool>,
}

fn static_info(gpu_name: Option<String>) -> Value {
    let cpu = reg_string("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0", "ProcessorNameString").unwrap_or_else(|| "Unknown CPU".into());
    let gpu = gpu_name.or_else(|| {
        (0..8).find_map(|i| {
            let d = reg_string(&format!("SYSTEM\\CurrentControlSet\\Control\\Class\\{{4d36e968-e325-11cd-b163-806bf22efdd1}}\\{i:04}"), "DriverDesc")?;
            (!d.to_ascii_lowercase().contains("basic")).then_some(d)
        })
    });
    let build: u32 = reg_string("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "CurrentBuild").and_then(|b| b.parse().ok()).unwrap_or(0);
    let ver = reg_string("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "DisplayVersion").unwrap_or_default();
    let mut m = MemoryStatus { length: std::mem::size_of::<MemoryStatus>() as u32, load: 0, total_phys: 0, avail_phys: 0, total_page: 0, avail_page: 0, total_virt: 0, avail_virt: 0, avail_ext: 0 };
    let ram_gb = if unsafe { GlobalMemoryStatusEx(&mut m) } != 0 { (m.total_phys as f64 / (1024.0 * 1024.0 * 1024.0)).round() as u64 } else { 0 };
    json!({
        "cpu_name": cpu.split_whitespace().collect::<Vec<_>>().join(" "),
        "threads": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
        "gpu_name": gpu, "ram_gb": ram_gb,
        "os": format!("{} {}", if build >= 22000 { "Windows 11" } else { "Windows 10" }, ver).trim().to_string(), "os_build": build,
    })
}

impl Sampler {
    pub fn start() -> Sampler {
        let snap = Arc::new(Mutex::new(Snapshot::default()));
        let active = Arc::new(AtomicBool::new(true));
        let (s2, a2) = (snap.clone(), active.clone());
        std::thread::spawn(move || {
            let nvml = Nvml::load();
            let pdh = Pdh::new();
            let c = |p: &str| pdh.as_ref().and_then(|q| q.add(p));
            let (net_rx, net_tx) = (c("\\Network Interface(*)\\Bytes Received/sec"), c("\\Network Interface(*)\\Bytes Sent/sec"));
            let (d_busy, d_read, d_write) = (c("\\PhysicalDisk(_Total)\\% Disk Time"), c("\\PhysicalDisk(_Total)\\Disk Read Bytes/sec"), c("\\PhysicalDisk(_Total)\\Disk Write Bytes/sec"));
            let cpu_base_mhz = c("\\Processor Information(_Total)\\Processor Frequency");
            let cpu_perf = c("\\Processor Information(_Total)\\% Processor Performance");
            // The GPU Engine counter has hundreds of instances (per process and engine): add it only when NVML is absent.
            let gpu_eng = if nvml.is_none() { c("\\GPU Engine(*)\\Utilization Percentage") } else { None };
            if let Some(q) = &pdh {
                q.collect();
            }
            s2.lock().unwrap().info = Some(static_info(nvml.as_ref().map(|n| n.name.clone())));
            let mut prev: Option<(u64, u64, u64)> = None;
            loop {
                let (mut idle, mut kernel, mut user) = (0u64, 0u64, 0u64);
                let cpu_pct = if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } != 0 {
                    let p = prev.map(|(pi, pk, pu)| {
                        let total = kernel.saturating_sub(pk) + user.saturating_sub(pu);
                        if total == 0 { 0.0 } else { 100.0 * total.saturating_sub(idle.saturating_sub(pi)) as f64 / total as f64 }
                    });
                    prev = Some((idle, kernel, user));
                    p
                } else {
                    None
                };
                if let Some(q) = &pdh {
                    q.collect();
                }
                let mut m = MemoryStatus { length: std::mem::size_of::<MemoryStatus>() as u32, load: 0, total_phys: 0, avail_phys: 0, total_page: 0, avail_page: 0, total_virt: 0, avail_virt: 0, avail_ext: 0 };
                let ram = (unsafe { GlobalMemoryStatusEx(&mut m) } != 0).then(|| json!({ "used_mb": (m.total_phys - m.avail_phys) / (1024 * 1024), "total_mb": m.total_phys / (1024 * 1024) }));
                let sum = |c: &Option<Counter>| c.as_ref().map(|c| c.array().iter().map(|(_, v)| *v).sum::<f64>());
                let (rx, tx) = (sum(&net_rx), sum(&net_tx));
                let network = rx.zip(tx).map(|(r, t)| json!({ "down_mbps": (r * 8.0 / 1e5).round() / 10.0, "up_mbps": (t * 8.0 / 1e5).round() / 10.0 }));
                let g = |c: &Option<Counter>| c.as_ref().and_then(|c| c.value());
                let disk = g(&d_busy).map(|b| json!({ "busy_pct": b.min(100.0).round(), "read_mbs": (g(&d_read).unwrap_or(0.0) / 1e5).round() / 10.0, "write_mbs": (g(&d_write).unwrap_or(0.0) / 1e5).round() / 10.0 }));
                let mut gpu = nvml.as_ref().map(|n| n.read());
                if gpu.is_none() {
                    // Cross-vendor fallback: Windows GPU engine counters (utilization only; no temperature).
                    if let Some(c) = &gpu_eng {
                        let total: f64 = c.array().iter().filter(|(n, _)| n.contains("engtype_3D") || n.contains("engtype_3d")).map(|(_, v)| *v).sum();
                        gpu = Some(json!({ "name": s2.lock().unwrap().info.as_ref().and_then(|i| i["gpu_name"].as_str().map(String::from)), "source": "windows-counters", "util_pct": total.min(100.0).round(), "temp_c": null, "power_w": null, "fan_pct": null }));
                    }
                }
                {
                    let mut s = s2.lock().unwrap();
                    s.cpu = cpu_pct.map(|p| json!({ "percent": (p * 10.0).round() / 10.0, "freq_mhz": g(&cpu_base_mhz).zip(g(&cpu_perf)).map(|(b, p)| (b * p / 100.0).round()), "base_mhz": g(&cpu_base_mhz).map(|f| f.round()) }));
                    s.ram = ram;
                    s.gpu = gpu;
                    s.disk = disk;
                    s.network = network;
                    s.samples += 1;
                }
                // Cheap when nothing is visible: slow down while the layer is hidden or paused.
                std::thread::sleep(Duration::from_millis(if a2.load(Ordering::Relaxed) { 1000 } else { 5000 }));
            }
        });
        Sampler { snap, active }
    }

    pub fn set_active(&self, on: bool) {
        self.active.store(on, Ordering::Relaxed);
    }

    pub fn provider(&self) -> SamplerProvider {
        SamplerProvider { snap: self.snap.clone() }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snap.lock().unwrap().clone()
    }
}

pub struct SamplerProvider {
    snap: Arc<Mutex<Snapshot>>,
}

impl DataProvider for SamplerProvider {
    fn read(&self, perm: PermissionId) -> Option<Value> {
        let s = self.snap.lock().ok()?;
        match perm {
            PermissionId::SystemCpu => s.cpu.clone(),
            PermissionId::SystemRam => s.ram.clone(),
            PermissionId::SystemGpu => s.gpu.clone(),
            PermissionId::SystemDisk => s.disk.clone(),
            PermissionId::SystemNetwork => s.network.clone(),
            PermissionId::SystemInfo => s.info.clone(),
            _ => None,
        }
    }
}
