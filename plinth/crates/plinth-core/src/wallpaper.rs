//! Wallpaper packages: a video file or a small web page that paints the desktop layer's background.
//!
//! Security model (design/decisions.md D23): a wallpaper never gets the widget bridge, never gets network access and never
//! gets the permission broker. Video is passive data played by the trusted layer page. A web wallpaper runs in a sandboxed
//! iframe with an opaque origin and a CSP that forbids connections. This module only validates metadata and paths; serving
//! lives in plinth-host.

use serde_json::Value;

pub const MAX_MANIFEST_BYTES: usize = 16 * 1024;
pub const VIDEO_EXTENSIONS: [&str; 3] = ["mp4", "webm", "m4v"];
pub const MAX_IMPORT_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallpaperKind {
    Video,
    Web,
    /// A transparent web page drawn above a still picture or the Scene (rain, particles...). Same sandbox as `Web` (D23, D36).
    Overlay,
}

impl WallpaperKind {
    pub fn as_str(self) -> &'static str {
        match self {
            WallpaperKind::Video => "video",
            WallpaperKind::Web => "web",
            WallpaperKind::Overlay => "overlay",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    Cover,
    Contain,
}

impl Fit {
    pub fn css(self) -> &'static str {
        match self {
            Fit::Cover => "cover",
            Fit::Contain => "contain",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wallpaper {
    pub id: String,
    pub name: String,
    pub kind: WallpaperKind,
    /// Path of the video file or the HTML page, relative to the wallpaper's folder.
    pub entry: String,
    pub fit: Fit,
    pub author: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallpaperError {
    TooLarge,
    Json,
    Field(&'static str),
    BadId,
    BadEntry,
}

pub fn valid_id(id: &str) -> bool {
    (3..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-') && !id.starts_with('.') && !id.ends_with('.') && !id.contains("..")
}

/// A relative path that stays inside the wallpaper folder: no drive letters, no leading slash, no `..`, no backslashes,
/// no empty or dot segments, no control characters.
pub fn safe_rel_path(p: &str) -> bool {
    if p.is_empty() || p.len() > 200 || p.starts_with('/') || p.contains('\\') || p.contains(':') || p.chars().any(|c| c.is_control()) {
        return false;
    }
    p.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

pub fn extension(path: &str) -> Option<String> {
    let name = path.rsplit('/').next()?;
    let (_, ext) = name.rsplit_once('.')?;
    Some(ext.to_ascii_lowercase())
}

pub fn is_video_file(path: &str) -> bool {
    extension(path).map_or(false, |e| VIDEO_EXTENSIONS.contains(&e.as_str()))
}

/// Content types a wallpaper folder may serve. Anything else is refused (no .exe, .dll, .json, .js outside web wallpapers...).
pub fn mime_for(path: &str) -> Option<&'static str> {
    Some(match extension(path)?.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "woff2" => "font/woff2",
        _ => return None,
    })
}

pub fn parse(json: &str) -> Result<Wallpaper, WallpaperError> {
    if json.len() > MAX_MANIFEST_BYTES {
        return Err(WallpaperError::TooLarge);
    }
    let v: Value = serde_json::from_str(json.trim_start_matches('\u{feff}')).map_err(|_| WallpaperError::Json)?;
    let s = |k: &'static str| v.get(k).and_then(Value::as_str);
    let id = s("id").ok_or(WallpaperError::Field("id"))?;
    if !valid_id(id) {
        return Err(WallpaperError::BadId);
    }
    let name = s("name").ok_or(WallpaperError::Field("name"))?.trim();
    if name.is_empty() || name.chars().count() > 60 || name.chars().any(|c| c.is_control()) {
        return Err(WallpaperError::Field("name"));
    }
    let kind = match s("kind") {
        Some("video") => WallpaperKind::Video,
        Some("web") => WallpaperKind::Web,
        Some("overlay") => WallpaperKind::Overlay,
        _ => return Err(WallpaperError::Field("kind")),
    };
    let entry = s("entry").ok_or(WallpaperError::Field("entry"))?;
    let entry_ok = safe_rel_path(entry)
        && entry.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/'))
        && match kind {
            WallpaperKind::Video => is_video_file(entry),
            WallpaperKind::Web | WallpaperKind::Overlay => extension(entry).as_deref() == Some("html"),
        };
    if !entry_ok {
        return Err(WallpaperError::BadEntry);
    }
    let fit = match s("fit") {
        None | Some("cover") => Fit::Cover,
        Some("contain") => Fit::Contain,
        _ => return Err(WallpaperError::Field("fit")),
    };
    let author: String = s("author").unwrap_or("").chars().filter(|c| !c.is_control()).take(60).collect();
    Ok(Wallpaper { id: id.to_string(), name: name.to_string(), kind, entry: entry.to_string(), fit, author })
}

/// Build the manifest JSON for a video the user imported.
pub fn video_manifest(id: &str, name: &str, entry: &str) -> String {
    serde_json::json!({ "id": id, "name": name, "kind": "video", "entry": entry, "fit": "cover", "author": "You" }).to_string()
}

/// A readable folder/id slug from a file name: lowercase alphanumerics and hyphens, at most 40 characters.
pub fn slug(file_stem: &str) -> String {
    let mut out = String::new();
    let mut dash = true;
    for c in file_stem.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    let t = out.trim_end_matches('-');
    let t: String = t.chars().take(40).collect();
    let t = t.trim_end_matches('-').to_string();
    if t.is_empty() {
        "clip".to_string()
    } else {
        t
    }
}

/// Parse an HTTP `Range: bytes=a-b` header for a body of `len` bytes. Returns an inclusive (start, end) or None when the
/// header is absent, malformed, multi-range or unsatisfiable (the caller then serves the whole file or a 416).
pub fn parse_range(header: &str, len: u64) -> Option<(u64, u64)> {
    if len == 0 {
        return None;
    }
    let spec = header.trim().strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (start, end) = if a.is_empty() {
        let n: u64 = b.parse().ok()?; // suffix: last n bytes
        if n == 0 {
            return None;
        }
        (len.saturating_sub(n), len - 1)
    } else {
        let start: u64 = a.parse().ok()?;
        let end: u64 = if b.is_empty() { len - 1 } else { b.parse().ok()? };
        (start, end.min(len - 1))
    };
    if start > end || start >= len {
        return None;
    }
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{"id":"com.plinth.aurora","name":"Aurora","kind":"web","entry":"index.html","author":"Plinth"}"#;

    #[test]
    fn parses_a_web_wallpaper() {
        let w = parse(GOOD).unwrap();
        assert_eq!(w.kind, WallpaperKind::Web);
        assert_eq!(w.fit, Fit::Cover);
        assert_eq!(w.entry, "index.html");
    }

    #[test]
    fn parses_an_overlay_package_and_requires_an_html_entry() {
        let o = parse(r#"{"id":"com.plinth.fx-rain","name":"Rain","kind":"overlay","entry":"index.html"}"#).unwrap();
        assert_eq!(o.kind, WallpaperKind::Overlay);
        assert_eq!(o.kind.as_str(), "overlay");
        assert_eq!(parse(r#"{"id":"com.plinth.fx-rain","name":"Rain","kind":"overlay","entry":"rain.mp4"}"#), Err(WallpaperError::BadEntry));
        assert_eq!(parse(r#"{"id":"com.plinth.fx-rain","name":"Rain","kind":"overlay","entry":"../x.html"}"#), Err(WallpaperError::BadEntry));
    }

    #[test]
    fn parses_a_video_wallpaper_with_fit() {
        let w = parse(r#"{"id":"user.loop","name":"Loop","kind":"video","entry":"clips/a.MP4","fit":"contain"}"#).unwrap();
        assert_eq!(w.kind, WallpaperKind::Video);
        assert_eq!(w.fit, Fit::Contain);
    }

    #[test]
    fn tolerates_a_bom_but_not_junk() {
        assert!(parse(&format!("\u{feff}{GOOD}")).is_ok());
        assert_eq!(parse("not json"), Err(WallpaperError::Json));
        assert_eq!(parse(&" ".repeat(MAX_MANIFEST_BYTES + 1)), Err(WallpaperError::TooLarge));
    }

    #[test]
    fn rejects_bad_ids() {
        for id in ["", "ab", "Has.Upper", "has space", "a..b", ".lead", "trail.", "../evil", "x/y", "ünï"] {
            let j = format!(r#"{{"id":"{id}","name":"n","kind":"web","entry":"index.html"}}"#);
            assert!(parse(&j).is_err(), "id {id:?} must be rejected");
        }
    }

    #[test]
    fn rejects_escaping_or_wrong_entries() {
        for entry in ["a\"b.html", "a b.html", "a<b>.html", "../x.html", "/abs.html", "C:/x.html", "a\\b.html", "a//b.html", "./a.html", "", "a.exe", "a.js", "a.mp4"] {
            let j = serde_json::json!({ "id": "user.t", "name": "n", "kind": "web", "entry": entry }).to_string();
            assert_eq!(parse(&j), Err(WallpaperError::BadEntry), "web entry {entry:?}");
        }
        for entry in ["a.html", "a.exe", "a.gif", "../a.mp4"] {
            let j = format!(r#"{{"id":"user.t","name":"n","kind":"video","entry":"{entry}"}}"#);
            assert_eq!(parse(&j), Err(WallpaperError::BadEntry), "video entry {entry:?}");
        }
    }

    #[test]
    fn rejects_bad_kind_fit_and_name() {
        assert!(parse(r#"{"id":"user.t","name":"n","kind":"exe","entry":"a.html"}"#).is_err());
        assert!(parse(r#"{"id":"user.t","name":"n","kind":"web","entry":"a.html","fit":"stretch"}"#).is_err());
        assert!(parse(r#"{"id":"user.t","name":"","kind":"web","entry":"a.html"}"#).is_err());
        assert!(parse(r#"{"id":"user.t","name":"a\u0007b","kind":"web","entry":"a.html"}"#).is_err());
    }

    #[test]
    fn mime_allowlist_is_closed() {
        assert_eq!(mime_for("a/b.MP4"), Some("video/mp4"));
        assert_eq!(mime_for("x.webm"), Some("video/webm"));
        assert_eq!(mime_for("index.html"), Some("text/html; charset=utf-8"));
        for bad in ["a.exe", "a.dll", "a.json", "a.ps1", "a.bat", "a", "a.mp4.exe", "wallpaper.json"] {
            assert_eq!(mime_for(bad), None, "{bad}");
        }
    }

    #[test]
    fn slug_is_safe_and_never_empty() {
        assert_eq!(slug("My Cool Clip (final) v2"), "my-cool-clip-final-v2");
        assert_eq!(slug("日本語"), "clip");
        assert_eq!(slug("---"), "clip");
        assert!(slug(&"a".repeat(200)).len() <= 40);
        assert!(valid_id(&format!("user.{}", slug("../../x y"))));
    }

    #[test]
    fn video_manifest_round_trips() {
        let w = parse(&video_manifest("user.clip", "Clip", "clip.mp4")).unwrap();
        assert_eq!((w.kind, w.entry.as_str(), w.author.as_str()), (WallpaperKind::Video, "clip.mp4", "You"));
    }

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=500-", 1000), Some((500, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=900-5000", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=1000-", 1000), None);
        assert_eq!(parse_range("bytes=5-2", 1000), None);
        assert_eq!(parse_range("bytes=0-1,5-9", 1000), None);
        assert_eq!(parse_range("bytes=-0", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
        assert_eq!(parse_range("bytes=0-1", 0), None);
        assert_eq!(parse_range("bytes=a-b", 1000), None);
    }
}
