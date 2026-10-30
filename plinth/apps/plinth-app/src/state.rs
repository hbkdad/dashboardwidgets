//! Persistent user state in `%APPDATA%\Plinth\state.json`: selected and applied scene, safe mode, grants, widget storage.
//! Files are written atomically (temp file then rename). Corrupt or missing state falls back to defaults; the app never
//! refuses to start because of it.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Sel {
    pub key: String,
    pub fit: String,
    pub bg: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Slide {
    pub minutes: u32,
    pub shuffle: bool,
    pub filter: String,
}

pub struct State {
    pub scene: String,
    pub applied: bool,
    pub safe: bool,
    /// Live motion wallpaper (animated). Off by default: it dominated idle CPU in measurements.
    pub motion: bool,
    /// Selected wallpaper id (video or web); empty means the Scene's own gradient.
    pub wallpaper: String,
    /// Wallpapers of non-primary monitors: monitor id -> wallpaper id (the primary monitor uses `wallpaper`).
    pub monitor_walls: HashMap<String, String>,
    /// Where each widget lives (home monitor, anchor, offsets, size). Only `home` data is ever saved (D26).
    pub placements: Vec<plinth_core::monitors::Placement>,
    /// Monitor id widgets park on when their home is unplugged; empty means the primary.
    pub fallback: String,
    /// Static image wallpaper per monitor id (a library item, fit mode and letterbox color).
    pub lib_sel: HashMap<String, Sel>,
    pub slides: HashMap<String, Slide>,
    /// Legibility assist behind widgets: none, dim or blur.
    pub scrim: String,
    /// Named profiles: scene, wallpapers, per-monitor selections and widget placements saved together.
    pub profiles: std::collections::BTreeMap<String, Value>,
    /// What the desktop wallpaper looked like before Plinth first changed it (for Revert).
    pub os_snapshot: Option<crate::os_wall::Snapshot>,
    /// widget id -> granted permission ids
    pub grants: HashMap<String, Vec<String>>,
    pub storage: HashMap<String, HashMap<String, String>>,
    pub first_run: bool,
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Plinth")
}

pub fn local_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Plinth")
}

fn path() -> PathBuf {
    if let Some(p) = std::env::var_os("PLINTH_STATE") {
        return PathBuf::from(p);
    }
    data_dir().join("state.json")
}

impl State {
    pub fn load() -> State {
        let default = State { scene: "graphite".into(), applied: false, safe: false, motion: false, wallpaper: String::new(), monitor_walls: HashMap::new(), placements: Vec::new(), fallback: String::new(), lib_sel: HashMap::new(), slides: HashMap::new(), scrim: "none".into(), profiles: Default::default(), os_snapshot: None, grants: HashMap::new(), storage: HashMap::new(), first_run: true };
        let Ok(text) = std::fs::read_to_string(path()) else { return default };
        let Ok(v) = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) else { return default }; // tolerate a UTF-8 BOM added by other tools
        let scene = v.get("scene").and_then(Value::as_str).filter(|s| ["dusk", "graphite", "paper"].contains(s)).unwrap_or("graphite").to_string();
        let grants = v
            .get("grants")
            .and_then(Value::as_object)
            .map(|o| o.iter().map(|(k, a)| (k.clone(), a.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default())).collect())
            .unwrap_or_default();
        let storage = v
            .get("storage")
            .and_then(Value::as_object)
            .map(|o| o.iter().map(|(k, m)| (k.clone(), m.as_object().map(|m| m.iter().filter_map(|(k2, v2)| v2.as_str().map(|s| (k2.clone(), s.to_string()))).collect()).unwrap_or_default())).collect())
            .unwrap_or_default();
        let wallpaper = v.get("wallpaper").and_then(Value::as_str).filter(|w| plinth_core::wallpaper::valid_id(w)).unwrap_or("").to_string();
        let monitor_walls = v
            .get("monitor_walls")
            .and_then(Value::as_object)
            .map(|o| o.iter().filter_map(|(k, w)| w.as_str().filter(|w| plinth_core::wallpaper::valid_id(w)).map(|w| (k.clone(), w.to_string()))).collect())
            .unwrap_or_default();
        let placements = v
            .get("placements")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|p| serde_json::from_value::<plinth_core::monitors::Placement>(p.clone()).ok()).filter(|p| p.w > 0.0 && p.h > 0.0 && p.w < 8000.0 && p.h < 8000.0 && p.dx.is_finite() && p.dy.is_finite()).collect())
            .unwrap_or_default();
        let fallback = v.get("fallback").and_then(Value::as_str).unwrap_or("").to_string();
        let lib_sel = v
            .get("lib_sel")
            .and_then(Value::as_object)
            .map(|o| {
                o.iter()
                    .filter_map(|(k, x)| {
                        let key = x.get("key")?.as_str()?.to_string();
                        if !plinth_core::library::valid_key(&key) {
                            return None;
                        }
                        let fit = x.get("fit").and_then(Value::as_str).filter(|f| plinth_core::fit::FitMode::parse(f).is_some()).unwrap_or("fill").to_string();
                        let bg = x.get("bg").and_then(Value::as_str).filter(|c| plinth_core::fit::parse_color(c).is_some()).unwrap_or("#000000").to_string();
                        Some((k.clone(), Sel { key, fit, bg }))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let slides = v
            .get("slides")
            .and_then(Value::as_object)
            .map(|o| o.iter().map(|(k, x)| (k.clone(), Slide { minutes: x.get("minutes").and_then(Value::as_u64).unwrap_or(30).clamp(1, 1440) as u32, shuffle: x.get("shuffle").and_then(Value::as_bool).unwrap_or(false), filter: x.get("filter").and_then(Value::as_str).filter(|f| ["all", "favorites"].contains(f)).unwrap_or("all").to_string() })).collect())
            .unwrap_or_default();
        let scrim = v.get("scrim").and_then(Value::as_str).filter(|m| ["none", "dim", "blur"].contains(m)).unwrap_or("none").to_string();
        let profiles = v.get("profiles").and_then(Value::as_object).map(|o| o.iter().filter(|(k, x)| !k.is_empty() && k.chars().count() <= 40 && x.is_object()).take(32).map(|(k, x)| (k.clone(), x.clone())).collect()).unwrap_or_default();
        let os_snapshot = v.get("os_snapshot").and_then(|o| {
            Some(crate::os_wall::Snapshot {
                wallpapers: o.get("wallpapers")?.as_array()?.iter().filter_map(|p| Some((p.get(0)?.as_str()?.to_string(), p.get(1)?.as_str()?.to_string()))).collect(),
                position: o.get("position")?.as_i64()? as i32,
                color: o.get("color")?.as_u64()? as u32,
            })
        });
        State { scene, wallpaper, monitor_walls, placements, fallback, lib_sel, slides, scrim, profiles, os_snapshot, applied: v.get("applied").and_then(Value::as_bool).unwrap_or(false), safe: v.get("safe").and_then(Value::as_bool).unwrap_or(false), motion: v.get("motion").and_then(Value::as_bool).unwrap_or(false), grants, storage, first_run: false }
    }

    pub fn save(&self) {
        let p = path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let v = json!({ "version": 1, "scene": self.scene, "applied": self.applied, "safe": self.safe, "motion": self.motion, "wallpaper": self.wallpaper, "monitor_walls": self.monitor_walls, "placements": self.placements, "fallback": self.fallback, "lib_sel": self.lib_sel.iter().map(|(k, x)| (k.clone(), json!({ "key": x.key, "fit": x.fit, "bg": x.bg }))).collect::<serde_json::Map<_, _>>(), "slides": self.slides.iter().map(|(k, x)| (k.clone(), json!({ "minutes": x.minutes, "shuffle": x.shuffle, "filter": x.filter }))).collect::<serde_json::Map<_, _>>(), "scrim": self.scrim, "profiles": self.profiles, "os_snapshot": self.os_snapshot.as_ref().map(|o| json!({ "wallpapers": o.wallpapers.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>(), "position": o.position, "color": o.color })), "grants": self.grants, "storage": self.storage });
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec_pretty(&v).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }
}
