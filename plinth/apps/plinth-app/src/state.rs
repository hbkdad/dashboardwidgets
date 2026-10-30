//! Persistent user state in `%APPDATA%\Plinth\state.json`: selected and applied scene, safe mode, grants, widget storage.
//! Files are written atomically (temp file then rename). Corrupt or missing state falls back to defaults; the app never
//! refuses to start because of it.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;

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
        let default = State { scene: "graphite".into(), applied: false, safe: false, motion: false, wallpaper: String::new(), monitor_walls: HashMap::new(), placements: Vec::new(), fallback: String::new(), grants: HashMap::new(), storage: HashMap::new(), first_run: true };
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
        State { scene, wallpaper, monitor_walls, placements, fallback, applied: v.get("applied").and_then(Value::as_bool).unwrap_or(false), safe: v.get("safe").and_then(Value::as_bool).unwrap_or(false), motion: v.get("motion").and_then(Value::as_bool).unwrap_or(false), grants, storage, first_run: false }
    }

    pub fn save(&self) {
        let p = path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let v = json!({ "version": 1, "scene": self.scene, "applied": self.applied, "safe": self.safe, "motion": self.motion, "wallpaper": self.wallpaper, "monitor_walls": self.monitor_walls, "placements": self.placements, "fallback": self.fallback, "grants": self.grants, "storage": self.storage });
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec_pretty(&v).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }
}
