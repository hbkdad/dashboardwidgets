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
        let default = State { scene: "graphite".into(), applied: false, safe: false, motion: false, grants: HashMap::new(), storage: HashMap::new(), first_run: true };
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
        State { scene, applied: v.get("applied").and_then(Value::as_bool).unwrap_or(false), safe: v.get("safe").and_then(Value::as_bool).unwrap_or(false), motion: v.get("motion").and_then(Value::as_bool).unwrap_or(false), grants, storage, first_run: false }
    }

    pub fn save(&self) {
        let p = path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let v = json!({ "version": 1, "scene": self.scene, "applied": self.applied, "safe": self.safe, "motion": self.motion, "grants": self.grants, "storage": self.storage });
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec_pretty(&v).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }
}
