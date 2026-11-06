//! Widget skins (decision D37): how widgets LOOK, as data only. A skin is a handful of numbers, two colors and two names from closed
//! lists (ink, font); it never contains code, CSS text, URLs or font files, so a skin inside a shared setup card cannot do anything but
//! restyle the frame around a widget. Skins are validated here exactly like the rest of a card (strict ranges, closed vocabularies);
//! the CSS the layer uses is generated from the validated numbers, never from user text.

use serde_json::{json, Value};

pub const MAX_SKINS: usize = 16;
pub const MAX_CARD_SKINS: usize = 8;
pub const DEFAULT_ID: &str = "plinth-glass";

/// Text color on the widget: follow the Scene, or force light or dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ink {
    Auto,
    Light,
    Dark,
}

impl Ink {
    pub fn parse(s: &str) -> Option<Ink> {
        Some(match s {
            "auto" => Ink::Auto,
            "light" => Ink::Light,
            "dark" => Ink::Dark,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Ink::Auto => "auto",
            Ink::Light => "light",
            Ink::Dark => "dark",
        }
    }
}

/// A font choice from a closed list of fonts that ship with Windows. No web fonts, no files, no names typed by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Font {
    Default,
    Humanist,
    Rounded,
    Serif,
    Mono,
    Condensed,
}

pub const FONTS: [Font; 6] = [Font::Default, Font::Humanist, Font::Rounded, Font::Serif, Font::Mono, Font::Condensed];

impl Font {
    pub fn parse(s: &str) -> Option<Font> {
        FONTS.iter().copied().find(|f| f.as_str() == s)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Font::Default => "default",
            Font::Humanist => "humanist",
            Font::Rounded => "rounded",
            Font::Serif => "serif",
            Font::Mono => "mono",
            Font::Condensed => "condensed",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Font::Default => "Windows default",
            Font::Humanist => "Humanist",
            Font::Rounded => "Rounded",
            Font::Serif => "Serif",
            Font::Mono => "Monospace",
            Font::Condensed => "Condensed",
        }
    }
    /// The CSS font stack. Fixed text from this table, never from user input.
    pub fn stack(self) -> &'static str {
        match self {
            Font::Default => "'Segoe UI Variable Display','Segoe UI',system-ui,sans-serif",
            Font::Humanist => "Calibri,'Segoe UI',sans-serif",
            Font::Rounded => "'Trebuchet MS','Segoe UI',sans-serif",
            Font::Serif => "Georgia,Cambria,serif",
            Font::Mono => "'Cascadia Mono',Consolas,monospace",
            Font::Condensed => "'Bahnschrift SemiCondensed','Arial Narrow',sans-serif",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skin {
    pub id: String,
    pub name: String,
    /// Glass color; None = the Scene's own glass color.
    pub tint: Option<(u8, u8, u8)>,
    /// How solid the glass is, 0 (clear) to 100 (opaque); None = the Scene's own.
    pub opacity: Option<u8>,
    /// How much the picture behind is blurred, 0 to 100 (55 is the classic look).
    pub frost: u8,
    /// Corner radius in DIP, 0 to 48.
    pub radius: u8,
    /// Strength of the light edge, 0 to 100.
    pub border: u8,
    /// Strength of the drop shadow, 0 to 100.
    pub shadow: u8,
    /// Accent color handed to widgets; None = the Scene's.
    pub accent: Option<(u8, u8, u8)>,
    pub ink: Ink,
    pub font: Font,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkinError {
    NotAnObject,
    BadId,
    Reserved,
    BadName,
    /// A field that is missing the right type or lies outside its allowed range or list.
    Field(&'static str),
    TooMany,
}

/// Skin ids: 3 to 40 lowercase letters, digits and hyphens.
pub fn valid_id(id: &str) -> bool {
    (3..=40).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') && !id.starts_with('-') && !id.ends_with('-')
}

fn hex(c: (u8, u8, u8)) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

fn color(v: &Value, field: &'static str) -> Result<Option<(u8, u8, u8)>, SkinError> {
    match v {
        Value::Null => Ok(None),
        Value::String(s) => crate::fit::parse_color(s).map(Some).ok_or(SkinError::Field(field)),
        _ => Err(SkinError::Field(field)),
    }
}

fn num(o: &serde_json::Map<String, Value>, k: &'static str, default: u8, max: u8) -> Result<u8, SkinError> {
    match o.get(k) {
        None => Ok(default),
        Some(v) => v.as_u64().filter(|n| *n <= max as u64).map(|n| n as u8).ok_or(SkinError::Field(k)),
    }
}

impl Skin {
    /// The look Plinth has always had. Cannot be edited or removed.
    pub fn plinth_glass() -> Skin {
        Skin { id: DEFAULT_ID.into(), name: "Plinth Glass".into(), tint: None, opacity: None, frost: 55, radius: 22, border: 32, shadow: 50, accent: None, ink: Ink::Auto, font: Font::Default }
    }

    /// Skins that ship with Plinth. Their ids are reserved.
    pub fn builtins() -> Vec<Skin> {
        let base = Skin::plinth_glass();
        vec![
            base.clone(),
            Skin { id: "frosted".into(), name: "Frosted".into(), opacity: Some(30), frost: 90, radius: 26, border: 40, shadow: 40, ..base.clone() },
            Skin { id: "clear-glass".into(), name: "Clear glass".into(), opacity: Some(8), frost: 20, radius: 18, border: 55, shadow: 20, ..base.clone() },
            Skin { id: "solid-slate".into(), name: "Solid slate".into(), tint: Some((0x1c, 0x21, 0x2e)), opacity: Some(96), frost: 0, radius: 10, border: 20, shadow: 60, ink: Ink::Light, ..base.clone() },
            Skin { id: "paper-card".into(), name: "Paper card".into(), tint: Some((0xf4, 0xf1, 0xe8)), opacity: Some(92), frost: 10, radius: 14, border: 10, shadow: 35, ink: Ink::Dark, font: Font::Serif, ..base.clone() },
            Skin { id: "terminal".into(), name: "Terminal".into(), tint: Some((0x05, 0x0a, 0x08)), opacity: Some(88), frost: 15, radius: 4, border: 70, shadow: 30, accent: Some((0x4a, 0xf6, 0x26)), ink: Ink::Light, font: Font::Mono, ..base.clone() },
            Skin { id: "soft-pill".into(), name: "Soft pill".into(), opacity: Some(45), frost: 70, radius: 48, border: 25, shadow: 45, font: Font::Rounded, ..base },
        ]
    }

    pub fn is_builtin_id(id: &str) -> bool {
        Skin::builtins().iter().any(|s| s.id == id)
    }

    /// Validate a skin from untrusted JSON (a hub message, saved state or a setup card). Strict: out-of-range numbers, unknown fonts
    /// or inks, bad colors and reserved ids are errors, not clamped. Unknown keys are ignored.
    pub fn from_value(v: &Value) -> Result<Skin, SkinError> {
        let o = v.as_object().ok_or(SkinError::NotAnObject)?;
        let id = o.get("id").and_then(Value::as_str).ok_or(SkinError::BadId)?;
        if !valid_id(id) {
            return Err(SkinError::BadId);
        }
        if Skin::is_builtin_id(id) {
            return Err(SkinError::Reserved);
        }
        Skin::from_object(o, id)
    }

    fn from_object(o: &serde_json::Map<String, Value>, id: &str) -> Result<Skin, SkinError> {
        let name = o.get("name").and_then(Value::as_str).ok_or(SkinError::BadName)?.trim();
        if name.is_empty() || name.chars().count() > 40 || name.chars().any(|c| c.is_control()) {
            return Err(SkinError::BadName);
        }
        let opacity = match o.get("opacity") {
            None | Some(Value::Null) => None,
            Some(v) => Some(v.as_u64().filter(|n| *n <= 100).map(|n| n as u8).ok_or(SkinError::Field("opacity"))?),
        };
        Ok(Skin {
            id: id.to_string(),
            name: name.to_string(),
            tint: color(o.get("tint").unwrap_or(&Value::Null), "tint")?,
            opacity,
            frost: num(o, "frost", 55, 100)?,
            radius: num(o, "radius", 22, 48)?,
            border: num(o, "border", 32, 100)?,
            shadow: num(o, "shadow", 50, 100)?,
            accent: color(o.get("accent").unwrap_or(&Value::Null), "accent")?,
            ink: match o.get("ink") {
                None => Ink::Auto,
                Some(v) => v.as_str().and_then(Ink::parse).ok_or(SkinError::Field("ink"))?,
            },
            font: match o.get("font") {
                None => Font::Default,
                Some(v) => v.as_str().and_then(Font::parse).ok_or(SkinError::Field("font"))?,
            },
        })
    }

    pub fn to_value(&self) -> Value {
        json!({ "id": self.id, "name": self.name, "tint": self.tint.map(hex), "opacity": self.opacity, "frost": self.frost, "radius": self.radius,
            "border": self.border, "shadow": self.shadow, "accent": self.accent.map(hex), "ink": self.ink.as_str(), "font": self.font.as_str() })
    }

    /// The CSS custom properties for the frame of a widget. Every value is built from validated numbers. `scene` is the Scene's glass
    /// color and alpha (0 to 1), used where the skin leaves the choice to the Scene.
    pub fn vars(&self, scene: ((u8, u8, u8), f32)) -> Vec<(&'static str, String)> {
        let rgb = self.tint.unwrap_or(scene.0);
        let alpha = self.opacity.map_or(scene.1, |o| o as f32 / 100.0);
        vec![
            ("--glass", format!("rgba({},{},{},{:.2})", rgb.0, rgb.1, rgb.2, alpha)),
            ("--blur", format!("{}px", (self.frost as f32 * 0.4).round() as u32)),
            ("--radius", format!("{}px", self.radius)),
            ("--bd", format!("rgba(255,255,255,{:.2})", self.border as f32 / 100.0 * 0.5)),
            ("--shadow", if self.shadow == 0 { "none".to_string() } else { format!("0 20px 60px rgba(0,0,0,{:.2})", self.shadow as f32 / 100.0 * 0.6) }),
        ]
    }

    /// The ink the widget gets: the skin's own, or the Scene's (`scene_ink` is "light" or "dark").
    pub fn effective_ink(&self, scene_ink: &str) -> &'static str {
        match self.ink {
            Ink::Light => "light",
            Ink::Dark => "dark",
            Ink::Auto => {
                if scene_ink == "dark" {
                    "dark"
                } else {
                    "light"
                }
            }
        }
    }

    /// True when text would be hard to read: the glass is mostly solid (so the picture behind cannot rescue it) and the text color
    /// against the glass color is below the WCAG AA ratio for normal text (4.5).
    pub fn hard_to_read(&self, scene: ((u8, u8, u8), f32), scene_ink: &str) -> bool {
        let alpha = self.opacity.map_or(scene.1, |o| o as f32 / 100.0);
        if alpha < 0.5 {
            return false; // mostly see-through: depends on the picture, so no verdict
        }
        let glass = luminance(self.tint.unwrap_or(scene.0));
        let ink = if self.effective_ink(scene_ink) == "dark" { luminance((0x10, 0x13, 0x1c)) } else { 1.0 };
        let (hi, lo) = if ink > glass { (ink, glass) } else { (glass, ink) };
        (hi + 0.05) / (lo + 0.05) < 4.5
    }
}

/// WCAG relative luminance of an sRGB color.
fn luminance(c: (u8, u8, u8)) -> f32 {
    let f = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.0) + 0.7152 * f(c.1) + 0.0722 * f(c.2)
}

/// What a skin choice means for one widget: its own skin if set (and known), else the global one, else the default.
pub fn resolve<'a>(skins: &'a [Skin], global: &str, widget: Option<&str>) -> &'a Skin {
    let find = |id: &str| skins.iter().find(|s| s.id == id);
    widget.and_then(find).or_else(|| find(global)).or_else(|| find(DEFAULT_ID)).unwrap_or(&skins[0])
}

/// Validate the custom skins of a setup card: at most [`MAX_CARD_SKINS`], unique ids, and every skin valid.
pub fn parse_list(v: &Value, max: usize) -> Result<Vec<Skin>, SkinError> {
    let a = v.as_array().ok_or(SkinError::NotAnObject)?;
    if a.len() > max {
        return Err(SkinError::TooMany);
    }
    let mut out: Vec<Skin> = Vec::new();
    for x in a {
        let s = Skin::from_value(x)?;
        if out.iter().any(|o| o.id == s.id) {
            return Err(SkinError::BadId);
        }
        out.push(s);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCENE: ((u8, u8, u8), f32) = ((20, 22, 32), 0.42);

    fn custom() -> Value {
        json!({ "id": "my-glow", "name": "My glow", "tint": "#102030", "opacity": 60, "frost": 80, "radius": 30, "border": 50, "shadow": 40, "accent": "#ff8800", "ink": "light", "font": "mono" })
    }

    #[test]
    fn a_good_skin_round_trips() {
        let s = Skin::from_value(&custom()).unwrap();
        assert_eq!((s.tint, s.opacity, s.frost, s.radius, s.font, s.ink), (Some((16, 32, 48)), Some(60), 80, 30, Font::Mono, Ink::Light));
        assert_eq!(Skin::from_value(&s.to_value()).unwrap(), s);
    }

    #[test]
    fn missing_numbers_take_the_classic_look() {
        let s = Skin::from_value(&json!({ "id": "plain-one", "name": "Plain" })).unwrap();
        let d = Skin::plinth_glass();
        assert_eq!((s.frost, s.radius, s.border, s.shadow, s.font, s.ink, s.tint, s.opacity), (d.frost, d.radius, d.border, d.shadow, d.font, d.ink, None, None));
    }

    #[test]
    fn the_default_skin_reproduces_the_classic_frame_exactly() {
        let v: std::collections::HashMap<_, _> = Skin::plinth_glass().vars(SCENE).into_iter().collect();
        assert_eq!(v["--glass"], "rgba(20,22,32,0.42)");
        assert_eq!(v["--blur"], "22px");
        assert_eq!(v["--radius"], "22px");
        assert_eq!(v["--bd"], "rgba(255,255,255,0.16)");
        assert_eq!(v["--shadow"], "0 20px 60px rgba(0,0,0,0.30)");
    }

    #[test]
    fn out_of_range_values_and_unknown_names_are_refused_not_clamped() {
        let bad = |k: &str, v: Value| {
            let mut s = custom();
            s[k] = v;
            Skin::from_value(&s)
        };
        assert_eq!(bad("frost", json!(101)), Err(SkinError::Field("frost")));
        assert_eq!(bad("radius", json!(49)), Err(SkinError::Field("radius")));
        assert_eq!(bad("radius", json!(-1)), Err(SkinError::Field("radius")));
        assert_eq!(bad("opacity", json!(250)), Err(SkinError::Field("opacity")));
        assert_eq!(bad("border", json!(1.5)), Err(SkinError::Field("border")));
        assert_eq!(bad("shadow", json!("high")), Err(SkinError::Field("shadow")));
        assert_eq!(bad("tint", json!("red")), Err(SkinError::Field("tint")));
        assert_eq!(bad("tint", json!("#12345")), Err(SkinError::Field("tint")));
        assert_eq!(bad("tint", json!("url(http://x)")), Err(SkinError::Field("tint")));
        assert_eq!(bad("accent", json!(5)), Err(SkinError::Field("accent")));
        assert_eq!(bad("ink", json!("purple")), Err(SkinError::Field("ink")));
        assert_eq!(bad("font", json!("Comic Sans MS")), Err(SkinError::Field("font")));
        assert_eq!(bad("font", json!("url(x.woff2)")), Err(SkinError::Field("font")));
        assert_eq!(bad("name", json!("")), Err(SkinError::BadName));
        assert_eq!(bad("name", json!("x".repeat(41))), Err(SkinError::BadName));
        assert_eq!(bad("name", json!("a\u{7}b")), Err(SkinError::BadName));
        assert_eq!(bad("id", json!("../x")), Err(SkinError::BadId));
        assert_eq!(bad("id", json!("Has Space")), Err(SkinError::BadId));
        assert_eq!(bad("id", json!("ab")), Err(SkinError::BadId));
        assert_eq!(Skin::from_value(&json!([1])), Err(SkinError::NotAnObject));
    }

    #[test]
    fn built_in_ids_are_reserved_so_a_card_cannot_replace_them() {
        for b in Skin::builtins() {
            let mut s = custom();
            s["id"] = json!(b.id);
            assert_eq!(Skin::from_value(&s), Err(SkinError::Reserved), "{}", b.id);
        }
    }

    #[test]
    fn built_in_skins_are_valid_unique_and_start_with_the_default() {
        let b = Skin::builtins();
        assert_eq!(b[0].id, DEFAULT_ID);
        for s in &b {
            assert!(valid_id(&s.id), "{}", s.id);
            let mut again = s.to_value();
            again["id"] = json!("zz-copy-of-builtin");
            assert!(Skin::from_value(&again).is_ok(), "{} must satisfy its own validator", s.id);
        }
        let mut ids: Vec<_> = b.iter().map(|s| s.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), b.len());
    }

    #[test]
    fn generated_css_holds_only_validated_characters() {
        for s in Skin::builtins().into_iter().chain(Skin::from_value(&custom())) {
            for (k, v) in s.vars(SCENE) {
                assert!(k.starts_with("--"));
                assert!(v.bytes().all(|b| b.is_ascii_alphanumeric() || b" (),.%-".contains(&b)), "{k}: {v}");
            }
        }
        for f in FONTS {
            assert!(f.stack().bytes().all(|b| b.is_ascii_alphanumeric() || b" ,'-".contains(&b)), "{}", f.stack());
            assert_eq!(Font::parse(f.as_str()), Some(f));
        }
    }

    #[test]
    fn numbers_map_to_sensible_css() {
        let mut s = Skin::plinth_glass();
        s.frost = 100;
        s.shadow = 0;
        s.radius = 0;
        s.opacity = Some(100);
        s.tint = Some((255, 0, 128));
        let v: std::collections::HashMap<_, _> = s.vars(SCENE).into_iter().collect();
        assert_eq!((v["--blur"].as_str(), v["--shadow"].as_str(), v["--radius"].as_str(), v["--glass"].as_str()), ("40px", "none", "0px", "rgba(255,0,128,1.00)"));
    }

    #[test]
    fn a_skin_can_leave_color_and_opacity_to_the_scene() {
        let mut s = Skin::plinth_glass();
        s.opacity = Some(80);
        let v: std::collections::HashMap<_, _> = s.vars(SCENE).into_iter().collect();
        assert_eq!(v["--glass"], "rgba(20,22,32,0.80)");
        s.tint = Some((1, 2, 3));
        s.opacity = None;
        let v: std::collections::HashMap<_, _> = s.vars(SCENE).into_iter().collect();
        assert_eq!(v["--glass"], "rgba(1,2,3,0.42)");
    }

    #[test]
    fn per_widget_skin_beats_global_beats_default() {
        let mut all = Skin::builtins();
        all.push(Skin::from_value(&custom()).unwrap());
        assert_eq!(resolve(&all, "frosted", None).id, "frosted");
        assert_eq!(resolve(&all, "frosted", Some("my-glow")).id, "my-glow");
        assert_eq!(resolve(&all, "frosted", Some("deleted-skin")).id, "frosted", "an unknown per-widget skin falls back to the global one");
        assert_eq!(resolve(&all, "gone", None).id, DEFAULT_ID, "an unknown global skin falls back to the default");
    }

    #[test]
    fn ink_follows_the_scene_unless_the_skin_forces_it() {
        let mut s = Skin::plinth_glass();
        assert_eq!((s.effective_ink("light"), s.effective_ink("dark")), ("light", "dark"));
        s.ink = Ink::Dark;
        assert_eq!(s.effective_ink("light"), "dark");
    }

    #[test]
    fn a_solid_light_panel_with_light_text_is_flagged_and_a_see_through_one_is_not() {
        let mut s = Skin::plinth_glass();
        s.tint = Some((0xf0, 0xf0, 0xf0));
        s.opacity = Some(95);
        s.ink = Ink::Light;
        assert!(s.hard_to_read(SCENE, "light"));
        s.ink = Ink::Dark;
        assert!(!s.hard_to_read(SCENE, "light"), "dark text on a light panel is fine");
        s.ink = Ink::Light;
        s.opacity = Some(20);
        assert!(!s.hard_to_read(SCENE, "light"), "mostly clear glass gets no verdict");
        for b in Skin::builtins() {
            assert!(!b.hard_to_read(SCENE, "light"), "built-in {} must be readable on the dark scenes", b.id);
        }
    }

    #[test]
    fn card_skin_lists_are_bounded_and_unique() {
        let one = custom();
        assert_eq!(parse_list(&json!([one.clone()]), MAX_CARD_SKINS).unwrap().len(), 1);
        assert_eq!(parse_list(&json!([one.clone(), one.clone()]), MAX_CARD_SKINS), Err(SkinError::BadId));
        let many: Vec<Value> = (0..=MAX_CARD_SKINS).map(|i| json!({ "id": format!("skin-{i}0"), "name": "S" })).collect();
        assert_eq!(parse_list(&Value::Array(many), MAX_CARD_SKINS), Err(SkinError::TooMany));
        assert_eq!(parse_list(&json!("x"), MAX_CARD_SKINS), Err(SkinError::NotAnObject));
    }
}
