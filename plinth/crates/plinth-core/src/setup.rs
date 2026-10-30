//! Setup cards: a shareable, monitor-independent description of a whole desktop (scene + widget layout).
//! This is the "copy that setup" unit that Linux ricing culture shares as dotfiles, made safe and one-click:
//! a card references widgets by id and version, never contains code, and shows the combined permissions
//! before anything is installed or applied.

use crate::manifest::{valid_id, valid_version, Manifest};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const SETUP_FORMAT: u32 = 1;
pub const MAX_WIDGETS: usize = 24;
/// Layout coordinates are relative: the screen is 1000 x 1000 units, so one card fits any monitor.
pub const GRID: u32 = 1000;
pub const MIN_UNITS: u32 = 40;
pub const MAX_MONITORS: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub widget: String,
    pub version: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub monitor: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupCard {
    pub id: String,
    pub name: String,
    pub author: String,
    /// Attribution chain: the card this one was remixed from.
    pub remix_of: Option<String>,
    pub scene: String,
    pub accent: Option<String>,
    pub widgets: Vec<Placement>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SetupError {
    Parse(String),
    UnsupportedFormat(u32),
    BadId(String),
    BadVersion(String),
    BadAccent(String),
    NoWidgets,
    TooManyWidgets,
    OutOfBounds(String),
    TooSmall(String),
    BadMonitor(u8),
    BadName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Widgets the card needs that are not installed (id, version).
    pub missing: Vec<(String, String)>,
    /// permission id -> widgets that request it (only installed widgets can be inspected).
    pub permissions: BTreeMap<&'static str, Vec<String>>,
    /// Every network host any widget in the card may be allowed to reach.
    pub hosts: BTreeSet<String>,
}

#[derive(Deserialize)]
struct RawPlacement {
    widget: String,
    version: String,
    rect: [u32; 4],
    #[serde(default)]
    monitor: u8,
}

#[derive(Deserialize)]
struct RawCard {
    format: u32,
    id: String,
    name: String,
    author: String,
    #[serde(default)]
    remix_of: Option<String>,
    scene: String,
    #[serde(default)]
    accent: Option<String>,
    widgets: Vec<RawPlacement>,
}

fn valid_accent(a: &str) -> bool {
    a.len() == 7 && a.starts_with('#') && a[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn clean_text(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t.chars().count() <= 80 && !t.chars().any(|c| c.is_control())
}

/// One widget of the current desktop, positioned in device-independent pixels on a monitor, for export.
pub struct ExportItem {
    pub widget: String,
    pub version: String,
    /// Index of the monitor in the exporter's ordered list (0 = primary).
    pub monitor: u8,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// A stable card id from a name: lowercase letters, digits and hyphens.
pub fn slug_id(name: &str) -> String {
    let mut out = String::new();
    let mut dash = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    let t: String = out.trim_end_matches('-').chars().take(40).collect();
    let t = t.trim_end_matches('-').to_string();
    if t.is_empty() {
        "my-setup".into()
    } else {
        t
    }
}

impl SetupCard {
    /// Build a shareable card from the current layout. It contains no monitor names, ids, paths or user data: only the scene, the
    /// widget ids and versions, and where each widget sits as a fraction of its monitor (monitor 0 is the primary).
    pub fn export(name: &str, author: &str, scene: &str, accent: Option<&str>, monitor_sizes_dip: &[(f32, f32)], items: &[ExportItem]) -> Result<SetupCard, SetupError> {
        let mut widgets = Vec::new();
        for it in items.iter().take(MAX_WIDGETS) {
            let Some(&(mw, mh)) = monitor_sizes_dip.get(it.monitor as usize) else { continue };
            if mw < 1.0 || mh < 1.0 {
                continue;
            }
            let g = GRID as f32;
            let w = ((it.w / mw * g).round() as u32).clamp(MIN_UNITS, GRID);
            let h = ((it.h / mh * g).round() as u32).clamp(MIN_UNITS, GRID);
            let x = ((it.x / mw * g).round().max(0.0) as u32).min(GRID - w);
            let y = ((it.y / mh * g).round().max(0.0) as u32).min(GRID - h);
            widgets.push(Placement { widget: it.widget.clone(), version: it.version.clone(), x, y, w, h, monitor: it.monitor.min(MAX_MONITORS - 1) });
        }
        let card = SetupCard { id: format!("local.{}", slug_id(name)), name: name.trim().to_string(), author: author.trim().to_string(), remix_of: None, scene: scene.to_string(), accent: accent.map(String::from), widgets };
        // Round-trip through the strict parser so an exported card is always one that can be imported.
        SetupCard::parse(&card.to_json())?;
        Ok(card)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&serde_json::json!({
            "format": SETUP_FORMAT, "id": self.id, "name": self.name, "author": self.author, "remix_of": self.remix_of, "scene": self.scene, "accent": self.accent,
            "widgets": self.widgets.iter().map(|p| serde_json::json!({ "widget": p.widget, "version": p.version, "rect": [p.x, p.y, p.w, p.h], "monitor": p.monitor })).collect::<Vec<_>>(),
        }))
        .unwrap_or_default()
    }

    pub fn parse(json: &str) -> Result<SetupCard, SetupError> {
        let raw: RawCard = serde_json::from_str(json).map_err(|e| SetupError::Parse(e.to_string()))?;
        if raw.format != SETUP_FORMAT {
            return Err(SetupError::UnsupportedFormat(raw.format));
        }
        if !valid_id(&raw.id) {
            return Err(SetupError::BadId(raw.id));
        }
        if let Some(r) = &raw.remix_of {
            if !valid_id(r) {
                return Err(SetupError::BadId(r.clone()));
            }
        }
        if !clean_text(&raw.name) || !clean_text(&raw.author) {
            return Err(SetupError::BadName);
        }
        if raw.scene.is_empty() || raw.scene.len() > 64 || !raw.scene.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return Err(SetupError::BadId(raw.scene));
        }
        if let Some(a) = &raw.accent {
            if !valid_accent(a) {
                return Err(SetupError::BadAccent(a.clone()));
            }
        }
        if raw.widgets.is_empty() {
            return Err(SetupError::NoWidgets);
        }
        if raw.widgets.len() > MAX_WIDGETS {
            return Err(SetupError::TooManyWidgets);
        }
        let mut widgets = Vec::new();
        for p in raw.widgets {
            if !valid_id(&p.widget) {
                return Err(SetupError::BadId(p.widget));
            }
            if !valid_version(&p.version) {
                return Err(SetupError::BadVersion(p.version));
            }
            if p.monitor >= MAX_MONITORS {
                return Err(SetupError::BadMonitor(p.monitor));
            }
            let [x, y, w, h] = p.rect;
            if w < MIN_UNITS || h < MIN_UNITS {
                return Err(SetupError::TooSmall(p.widget));
            }
            if x.checked_add(w).map_or(true, |e| e > GRID) || y.checked_add(h).map_or(true, |e| e > GRID) {
                return Err(SetupError::OutOfBounds(p.widget));
            }
            widgets.push(Placement { widget: p.widget, version: p.version, x, y, w, h, monitor: p.monitor });
        }
        Ok(SetupCard { id: raw.id, name: raw.name, author: raw.author, remix_of: raw.remix_of, scene: raw.scene, accent: raw.accent, widgets })
    }

    /// What applying this card would require and allow. `installed` maps widget id to its manifest.
    pub fn summary(&self, installed: &HashMap<String, &Manifest>) -> Summary {
        let mut s = Summary::default();
        let mut seen = BTreeSet::new();
        for p in &self.widgets {
            if !seen.insert(p.widget.clone()) {
                continue;
            }
            match installed.get(&p.widget) {
                None => s.missing.push((p.widget.clone(), p.version.clone())),
                Some(m) => {
                    for d in &m.permissions {
                        s.permissions.entry(d.id.as_str()).or_default().push(p.widget.clone());
                        s.hosts.extend(d.hosts.iter().cloned());
                    }
                }
            }
        }
        s
    }

    /// Maps relative placements to pixels for one monitor. Small widgets are enlarged to a legible minimum
    /// (scaled by `dpi_scale`) and every rect is kept fully on screen.
    pub fn fit(&self, monitor: u8, screen_w: u32, screen_h: u32, dpi_scale: f32) -> Vec<(String, PixelRect)> {
        let min = (96.0 * dpi_scale.clamp(0.5, 4.0)) as u32;
        self.widgets
            .iter()
            .filter(|p| p.monitor == monitor)
            .map(|p| {
                let w = (p.w * screen_w / GRID).max(min).min(screen_w);
                let h = (p.h * screen_h / GRID).max(min).min(screen_h);
                let x = (p.x * screen_w / GRID).min(screen_w - w);
                let y = (p.y * screen_h / GRID).min(screen_h - h);
                (p.widget.clone(), PixelRect { x, y, w, h })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(widgets: &str) -> String {
        format!(
            r##"{{"format":1,"id":"com.author.dusk-focus","name":"Dusk Focus","author":"Ana","scene":"dusk","accent":"#ffb27a","widgets":{widgets}}}"##
        )
    }
    const OK: &str = r#"[{"widget":"com.plinth.clock","version":"1.2.0","rect":[70,120,300,160]},{"widget":"com.plinth.sys","version":"1.4.1","rect":[700,100,250,200]}]"#;

    #[test]
    fn parses_valid_card() {
        let c = SetupCard::parse(&card(OK)).unwrap();
        assert_eq!(c.widgets.len(), 2);
        assert_eq!(c.remix_of, None);
        assert_eq!(c.widgets[0].monitor, 0);
    }

    #[test]
    fn rejects_bad_cards() {
        let p = |j: String| SetupCard::parse(&j);
        assert!(matches!(p(card("[]")), Err(SetupError::NoWidgets)));
        assert!(matches!(p(card(r#"[{"widget":"com.a.b","version":"1.0.0","rect":[900,0,200,100]}]"#)), Err(SetupError::OutOfBounds(_))));
        assert!(matches!(p(card(r#"[{"widget":"com.a.b","version":"1.0.0","rect":[0,0,10,100]}]"#)), Err(SetupError::TooSmall(_))));
        assert!(matches!(p(card(r#"[{"widget":"com.a.b","version":"1.0.0","rect":[0,0,100,100],"monitor":9}]"#)), Err(SetupError::BadMonitor(9))));
        assert!(matches!(p(card(r#"[{"widget":"../x","version":"1.0.0","rect":[0,0,100,100]}]"#)), Err(SetupError::BadId(_))));
        assert!(matches!(p(card(r#"[{"widget":"com.a.b","version":"1","rect":[0,0,100,100]}]"#)), Err(SetupError::BadVersion(_))));
        assert!(matches!(p(card(OK).replace("#ffb27a", "red")), Err(SetupError::BadAccent(_))));
        assert!(matches!(p(card(OK).replace(r#""format":1"#, r#""format":2"#)), Err(SetupError::UnsupportedFormat(2))));
        assert!(matches!(p(card(OK).replace("Dusk Focus", "")), Err(SetupError::BadName)));
        assert!(matches!(p(card(OK).replace("Dusk Focus", "bad\\u0007name")), Err(SetupError::BadName)));
        assert!(matches!(p(card(OK).replace(r#""scene":"dusk""#, r#""scene":"../dusk""#)), Err(SetupError::BadId(_))));
        // overflow-safe: huge numbers must not wrap
        assert!(matches!(p(card(r#"[{"widget":"com.a.b","version":"1.0.0","rect":[4294967295,0,100,100]}]"#)), Err(SetupError::OutOfBounds(_))));
        let many = (0..=MAX_WIDGETS).map(|_| r#"{"widget":"com.a.b","version":"1.0.0","rect":[0,0,100,100]}"#).collect::<Vec<_>>().join(",");
        assert!(matches!(p(card(&format!("[{many}]"))), Err(SetupError::TooManyWidgets)));
    }

    #[test]
    fn remix_attribution_is_validated() {
        let j = card(OK).replace(r#""scene""#, r#""remix_of":"com.ana.original","scene""#);
        assert_eq!(SetupCard::parse(&j).unwrap().remix_of.as_deref(), Some("com.ana.original"));
        assert!(matches!(SetupCard::parse(&card(OK).replace(r#""scene""#, r#""remix_of":"X Y","scene""#)), Err(SetupError::BadId(_))));
    }

    #[test]
    fn summary_aggregates_permissions_hosts_and_missing() {
        let weather = Manifest::parse(r#"{"format":1,"id":"com.plinth.weather","name":"W","version":"1.0.0","type":"widget","permissions":[{"id":"net.fetch","hosts":["api.weather.example"],"reason":"x"},{"id":"storage","reason":"y"}]}"#).unwrap();
        let clock = Manifest::parse(r#"{"format":1,"id":"com.plinth.clock","name":"C","version":"1.2.0","type":"widget"}"#).unwrap();
        let mut inst: HashMap<String, &Manifest> = HashMap::new();
        inst.insert(weather.id.clone(), &weather);
        inst.insert(clock.id.clone(), &clock);
        let c = SetupCard::parse(&card(
            r#"[{"widget":"com.plinth.clock","version":"1.2.0","rect":[0,0,100,100]},{"widget":"com.plinth.weather","version":"1.0.0","rect":[200,0,100,100]},{"widget":"com.plinth.weather","version":"1.0.0","rect":[400,0,100,100]},{"widget":"com.vendor.new","version":"0.1.0","rect":[600,0,100,100]}]"#,
        ))
        .unwrap();
        let s = c.summary(&inst);
        assert_eq!(s.missing, vec![("com.vendor.new".to_string(), "0.1.0".to_string())]);
        assert_eq!(s.permissions["net.fetch"], vec!["com.plinth.weather".to_string()], "duplicates counted once");
        assert!(s.permissions.contains_key("storage"));
        assert_eq!(s.hosts.iter().collect::<Vec<_>>(), vec!["api.weather.example"]);
    }

    #[test]
    fn fit_scales_to_any_monitor_and_stays_on_screen() {
        let c = SetupCard::parse(&card(OK)).unwrap();
        let hd = c.fit(0, 1920, 1080, 1.0);
        assert_eq!(hd[0].1, PixelRect { x: 134, y: 129, w: 576, h: 172 });
        let small = c.fit(0, 800, 600, 1.0);
        for (_, r) in &small {
            assert!(r.x + r.w <= 800 && r.y + r.h <= 600 && r.w >= 96 && r.h >= 96, "{r:?}");
        }
        let hidpi = c.fit(0, 3840, 2160, 2.0);
        assert!(hidpi.iter().all(|(_, r)| r.w >= 192 && r.h >= 192));
        assert!(c.fit(1, 1920, 1080, 1.0).is_empty(), "monitor filter");
        // tiny screen smaller than min size still stays inside
        let tiny = c.fit(0, 80, 60, 1.0);
        assert!(tiny.iter().all(|(_, r)| r.x + r.w <= 80 && r.y + r.h <= 60));
    }

    #[test]
    fn export_round_trips_and_keeps_widgets_on_their_monitors() {
        let items = vec![
            ExportItem { widget: "com.plinth.clock".into(), version: "1.0.0".into(), monitor: 0, x: 72.0, y: 72.0, w: 360.0, h: 118.0 },
            ExportItem { widget: "com.plinth.pcvitals".into(), version: "1.0.0".into(), monitor: 1, x: 10.0, y: 10.0, w: 400.0, h: 250.0 },
        ];
        let card = SetupCard::export("My Rig: night!", "Me", "dusk", Some("#ffb27a"), &[(1920.0, 1080.0), (1280.0, 400.0)], &items).unwrap();
        assert_eq!(card.id, "local.my-rig-night");
        let back = SetupCard::parse(&card.to_json()).unwrap();
        assert_eq!(back, card);
        assert_eq!((back.widgets[0].monitor, back.widgets[1].monitor), (0, 1));
        assert!(!card.to_json().to_ascii_lowercase().contains("display#"), "no hardware ids in a shared card");
        // a widget wider than its monitor is clamped, never rejected
        let big = vec![ExportItem { widget: "a.b".into(), version: "1.0.0".into(), monitor: 0, x: 5000.0, y: -50.0, w: 9000.0, h: 9000.0 }];
        let c2 = SetupCard::export("Big", "Me", "graphite", None, &[(1920.0, 1080.0)], &big).unwrap();
        assert!(c2.widgets[0].x + c2.widgets[0].w <= GRID && c2.widgets[0].y + c2.widgets[0].h <= GRID);
        assert!(SetupCard::export("None", "Me", "graphite", None, &[(1920.0, 1080.0)], &[]).is_err());
        assert_eq!(slug_id("  "), "my-setup");
    }
}
