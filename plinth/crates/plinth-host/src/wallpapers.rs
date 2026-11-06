//! Wallpaper library: scans the bundled and user wallpaper folders, serves their files to the desktop layer under the
//! `pwv` origin, and imports video files the user drops onto the hub. No network, no widget bridge, no permissions: a
//! wallpaper can only draw. Metadata rules live in `plinth_core::wallpaper`.

use crate::host::Response;
use plinth_core::wallpaper::{self, Wallpaper, WallpaperKind};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub const WALLPAPER_SCHEME: &str = "pwv";
/// Largest body returned for one request. Media elements ask for ranges; a shorter 206 is always valid.
pub const CHUNK: u64 = 8 * 1024 * 1024;
pub const MANIFEST_FILE: &str = "wallpaper.json";
/// A web wallpaper is plain files; cap how big one folder may be so a bad package cannot fill memory or disk through us.
const MAX_WEB_FILE: u64 = 8 * 1024 * 1024;

/// CSP for web wallpapers: draw and animate, nothing else. `sandbox` is also set on the iframe by the layer page, which gives
/// the page an opaque origin; this header closes network and navigation even if the sandbox attribute were ever dropped.
pub const WEB_CSP: &str = "default-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; media-src 'self' data: blob:; font-src 'self' data:; connect-src 'none'; frame-src 'none'; object-src 'none'; form-action 'none'; base-uri 'none'";

#[derive(Debug, Clone)]
pub struct Entry {
    pub wallpaper: Wallpaper,
    pub dir: PathBuf,
    /// True for wallpapers shipped with the app (cannot be removed).
    pub bundled: bool,
}

pub struct WallpaperStore {
    bundled: PathBuf,
    user: PathBuf,
    items: Vec<Entry>,
}

fn text(status: u16, body: &str) -> Response {
    Response { status, headers: vec![("Content-Type".into(), "text/plain; charset=utf-8".into()), ("Cache-Control".into(), "no-store".into())], body: body.as_bytes().to_vec() }
}

impl WallpaperStore {
    pub fn new(bundled: PathBuf, user: PathBuf) -> Self {
        let mut s = WallpaperStore { bundled, user, items: Vec::new() };
        s.scan();
        s
    }

    pub fn user_dir(&self) -> &Path {
        &self.user
    }

    /// Rescan both folders. A folder with a bad manifest is skipped (and reported on stderr), never fatal.
    pub fn scan(&mut self) {
        let mut items: Vec<Entry> = Vec::new();
        for (root, bundled) in [(self.bundled.clone(), true), (self.user.clone(), false)] {
            let Ok(rd) = std::fs::read_dir(&root) else { continue };
            let mut dirs: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
            dirs.sort();
            for dir in dirs {
                let Ok(raw) = std::fs::read_to_string(dir.join(MANIFEST_FILE)) else { continue };
                match wallpaper::parse(&raw) {
                    Ok(w) => {
                        if items.iter().any(|e| e.wallpaper.id == w.id) {
                            eprintln!("wallpaper {}: duplicate id, skipped", w.id);
                        } else if !dir.join(&w.entry).is_file() {
                            eprintln!("wallpaper {}: entry file {} is missing, skipped", w.id, w.entry);
                        } else {
                            items.push(Entry { wallpaper: w, dir, bundled });
                        }
                    }
                    Err(e) => eprintln!("wallpaper folder {}: {e:?}, skipped", dir.display()),
                }
            }
        }
        self.items = items;
    }

    pub fn list(&self) -> &[Entry] {
        &self.items
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.items.iter().find(|e| e.wallpaper.id == id)
    }

    /// A wallpaper the user can pick as THE wallpaper: overlays are a separate choice and are never one.
    pub fn get_live(&self, id: &str) -> Option<&Entry> {
        self.get(id).filter(|e| e.wallpaper.kind != WallpaperKind::Overlay)
    }

    /// A motion overlay package.
    pub fn get_overlay(&self, id: &str) -> Option<&Entry> {
        self.get(id).filter(|e| e.wallpaper.kind == WallpaperKind::Overlay)
    }

    /// URL path the layer page uses for a wallpaper's entry file.
    pub fn entry_url(w: &Wallpaper) -> String {
        format!("http://{WALLPAPER_SCHEME}.localhost/{}/{}", w.id, w.entry)
    }

    /// Serve `/<wallpaper-id>/<relative path>`. Only files inside that wallpaper's folder, only allow-listed types.
    pub fn serve(&self, path_and_query: &str, range: Option<&str>) -> Response {
        let path = path_and_query.split(['?', '#']).next().unwrap_or("");
        let Some(rest) = path.strip_prefix('/') else { return text(404, "not found") };
        let Some((id, rel)) = rest.split_once('/') else { return text(404, "not found") };
        if !wallpaper::safe_rel_path(rel) {
            return text(400, "bad path");
        }
        let Some(entry) = self.get(id) else { return text(404, "not found") };
        let Some(mime) = wallpaper::mime_for(rel) else { return text(403, "type not allowed") };
        // A video wallpaper serves only its video; a web wallpaper may serve any allow-listed type (including its own media).
        let is_video = wallpaper::is_video_file(rel);
        if entry.wallpaper.kind == WallpaperKind::Video && !is_video {
            return text(403, "type not allowed");
        }
        // Resolve symlinks and junctions, then require the real file to still be inside the wallpaper's real folder.
        let (Ok(root), Ok(file)) = (entry.dir.canonicalize(), entry.dir.join(rel).canonicalize()) else { return text(404, "not found") };
        if !file.starts_with(&root) || !file.is_file() {
            return text(404, "not found");
        }
        let Ok(mut f) = std::fs::File::open(&file) else { return text(404, "not found") };
        let Ok(len) = f.metadata().map(|m| m.len()) else { return text(404, "not found") };
        let mut headers = vec![("Content-Type".to_string(), mime.to_string()), ("X-Content-Type-Options".into(), "nosniff".into()), ("Cache-Control".into(), "no-store".into()), ("Accept-Ranges".into(), "bytes".into())];
        if matches!(entry.wallpaper.kind, WallpaperKind::Web | WallpaperKind::Overlay) {
            if len > MAX_WEB_FILE {
                return text(413, "file too large");
            }
            headers.push(("Content-Security-Policy".into(), WEB_CSP.into()));
            let mut body = Vec::with_capacity(len as usize);
            if f.read_to_end(&mut body).is_err() {
                return text(500, "read error");
            }
            return Response { status: 200, headers, body };
        }
        // Video: honour Range, and never put more than CHUNK bytes in one response.
        if len == 0 {
            return Response { status: 200, headers, body: Vec::new() };
        }
        let asked = range.and_then(|h| wallpaper::parse_range(h, len));
        if range.is_some() && asked.is_none() {
            headers.push(("Content-Range".into(), format!("bytes */{len}")));
            return Response { status: 416, headers, body: Vec::new() };
        }
        let (start, end) = asked.unwrap_or((0, len - 1));
        let end = end.min(start + CHUNK - 1);
        let partial = asked.is_some() || end < len - 1;
        let mut body = vec![0u8; (end - start + 1) as usize];
        if f.seek(SeekFrom::Start(start)).is_err() || f.read_exact(&mut body).is_err() {
            return text(500, "read error");
        }
        if partial {
            headers.push(("Content-Range".into(), format!("bytes {start}-{end}/{len}")));
        }
        Response { status: if partial { 206 } else { 200 }, headers, body }
    }

    /// Copy a video the user chose into the user wallpaper folder and write its manifest. Only the extension allow-list and
    /// a size cap are checked here (the browser engine decides whether it can play it); the copy never executes anything.
    pub fn import_video(&mut self, src: &Path) -> Result<Wallpaper, String> {
        let name = src.file_name().and_then(|n| n.to_str()).ok_or("That file name is not usable.")?;
        if !wallpaper::is_video_file(&name.replace('\\', "/")) {
            return Err("Only .mp4, .webm and .m4v videos can be used as wallpapers.".into());
        }
        let meta = std::fs::metadata(src).map_err(|_| "Plinth could not read that file.")?;
        if !meta.is_file() {
            return Err("That is not a file.".into());
        }
        if meta.len() == 0 || meta.len() > wallpaper::MAX_IMPORT_BYTES {
            return Err("That video is empty or larger than 4 GB.".into());
        }
        let ext = wallpaper::extension(name).unwrap_or_default();
        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("clip");
        let base = wallpaper::slug(stem);
        let mut n = 1;
        let (id, folder) = loop {
            let candidate = if n == 1 { base.clone() } else { format!("{base}-{n}") };
            let id = format!("user.{candidate}");
            if self.get(&id).is_none() && !self.user.join(&candidate).exists() {
                break (id, self.user.join(&candidate));
            }
            n += 1;
            if n > 99 {
                return Err("Too many wallpapers with that name.".into());
            }
        };
        std::fs::create_dir_all(&folder).map_err(|_| "Plinth could not create the wallpaper folder.")?;
        let entry = format!("clip.{ext}");
        if std::fs::copy(src, folder.join(&entry)).is_err() {
            let _ = std::fs::remove_dir_all(&folder);
            return Err("Plinth could not copy that video (is the disk full?).".into());
        }
        let display: String = stem.chars().filter(|c| !c.is_control()).take(60).collect();
        let display = if display.trim().is_empty() { "My video".to_string() } else { display.trim().to_string() };
        let manifest = wallpaper::video_manifest(&id, &display, &entry);
        if std::fs::write(folder.join(MANIFEST_FILE), manifest).is_err() {
            let _ = std::fs::remove_dir_all(&folder);
            return Err("Plinth could not save the wallpaper.".into());
        }
        self.scan();
        self.get(&id).map(|e| e.wallpaper.clone()).ok_or_else(|| "The imported wallpaper was not recognised.".to_string())
    }

    /// Remove a wallpaper the user imported (Plinth's own copy only). Bundled wallpapers cannot be removed.
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        let entry = self.get(id).ok_or("Unknown wallpaper.")?;
        if entry.bundled {
            return Err("Built-in wallpapers cannot be removed.".into());
        }
        let (Ok(root), Ok(dir)) = (self.user.canonicalize(), entry.dir.canonicalize()) else { return Err("Wallpaper folder not found.".into()) };
        if dir.parent() != Some(root.as_path()) {
            return Err("Refusing to remove a folder outside the wallpaper library.".into());
        }
        std::fs::remove_dir_all(&dir).map_err(|_| "Plinth could not remove it (is it in use?).".to_string())?;
        self.scan();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static N: AtomicU32 = AtomicU32::new(0);

    fn sandbox() -> (PathBuf, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("plinth-wp-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&base);
        let (b, u) = (base.join("bundled"), base.join("user"));
        std::fs::create_dir_all(&b).unwrap();
        std::fs::create_dir_all(&u).unwrap();
        (base, b, u)
    }

    fn web(dir: &Path, id: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("wallpaper.json"), format!(r#"{{"id":"{id}","name":"Test","kind":"web","entry":"index.html"}}"#)).unwrap();
        std::fs::write(dir.join("index.html"), "<html>hi</html>").unwrap();
        std::fs::write(dir.join("secret.json"), "{}").unwrap();
    }

    #[test]
    fn scans_both_folders_and_skips_bad_ones() {
        let (base, b, u) = sandbox();
        web(&b.join("one"), "com.plinth.one");
        web(&u.join("dup"), "com.plinth.one"); // duplicate id
        std::fs::create_dir_all(u.join("broken")).unwrap();
        std::fs::write(u.join("broken").join("wallpaper.json"), "{nope").unwrap();
        std::fs::create_dir_all(u.join("missing")).unwrap();
        std::fs::write(u.join("missing").join("wallpaper.json"), r#"{"id":"user.missing","name":"M","kind":"web","entry":"index.html"}"#).unwrap();
        let s = WallpaperStore::new(b, u);
        assert_eq!(s.list().len(), 1);
        assert!(s.list()[0].bundled);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn web_files_get_csp_and_only_allowed_types() {
        let (base, b, u) = sandbox();
        web(&b.join("one"), "com.plinth.one");
        let s = WallpaperStore::new(b, u);
        let r = s.serve("/com.plinth.one/index.html?x=1", None);
        assert_eq!(r.status, 200);
        assert_eq!(r.header("content-security-policy"), Some(WEB_CSP));
        assert!(WEB_CSP.contains("connect-src 'none'"));
        assert_eq!(r.header("x-content-type-options"), Some("nosniff"));
        assert_eq!(s.serve("/com.plinth.one/secret.json", None).status, 403);
        assert_eq!(s.serve("/com.plinth.one/wallpaper.json", None).status, 403);
        assert_eq!(s.serve("/com.plinth.one/nope.html", None).status, 404);
        assert_eq!(s.serve("/unknown.id/index.html", None).status, 404);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn an_overlay_is_served_with_the_same_locked_down_csp_and_a_query_string_changes_nothing() {
        let (base, b, u) = sandbox();
        let d = b.join("rain");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("wallpaper.json"), r#"{"id":"com.plinth.fx-rain","name":"Rain","kind":"overlay","entry":"index.html"}"#).unwrap();
        std::fs::write(d.join("index.html"), "<canvas></canvas>").unwrap();
        std::fs::write(d.join("data.json"), "{}").unwrap();
        let s = WallpaperStore::new(b, u);
        assert_eq!(s.get("com.plinth.fx-rain").unwrap().wallpaper.kind, WallpaperKind::Overlay);
        for p in ["/com.plinth.fx-rain/index.html", "/com.plinth.fx-rain/index.html?i=3&s=150", "/com.plinth.fx-rain/index.html?i=%3Cscript%3E"] {
            let r = s.serve(p, None);
            assert_eq!(r.status, 200, "{p}");
            assert_eq!(r.header("content-security-policy"), Some(WEB_CSP));
            assert!(WEB_CSP.contains("connect-src 'none'") && WEB_CSP.contains("frame-src 'none'") && WEB_CSP.contains("default-src 'none'"));
        }
        assert_eq!(s.serve("/com.plinth.fx-rain/data.json", None).status, 403);
        assert_eq!(s.serve("/com.plinth.fx-rain/wallpaper.json", None).status, 403);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn traversal_is_refused() {
        let (base, b, u) = sandbox();
        web(&b.join("one"), "com.plinth.one");
        std::fs::write(b.join("outside.html"), "secret").unwrap();
        let s = WallpaperStore::new(b, u);
        for p in ["/com.plinth.one/../outside.html", "/com.plinth.one/..%2Foutside.html", "/com.plinth.one//index.html", "/com.plinth.one/a\\..\\b.html", "/com.plinth.one/C:/x.html", "/../outside.html", "/", ""] {
            let st = s.serve(p, None).status;
            assert!(st == 400 || st == 404, "{p:?} -> {st}");
        }
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn video_ranges_and_chunking() {
        let (base, b, u) = sandbox();
        let d = u.join("clip");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("wallpaper.json"), r#"{"id":"user.clip","name":"C","kind":"video","entry":"clip.mp4"}"#).unwrap();
        let data: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(d.join("clip.mp4"), &data).unwrap();
        let s = WallpaperStore::new(b, u);
        let whole = s.serve("/user.clip/clip.mp4", None);
        assert_eq!((whole.status, whole.body.len()), (200, data.len()));
        assert_eq!(whole.header("content-type"), Some("video/mp4"));
        let part = s.serve("/user.clip/clip.mp4", Some("bytes=1000-1999"));
        assert_eq!(part.status, 206);
        assert_eq!(part.header("content-range"), Some("bytes 1000-1999/100000"));
        assert_eq!(part.body, data[1000..2000]);
        let tail = s.serve("/user.clip/clip.mp4", Some("bytes=-10"));
        assert_eq!(tail.body, data[data.len() - 10..]);
        assert_eq!(s.serve("/user.clip/clip.mp4", Some("bytes=999999-")).status, 416);
        assert_eq!(s.serve("/user.clip/index.html", None).status, 403); // a video wallpaper serves no pages
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn symlink_style_escape_is_blocked_by_canonicalisation() {
        let (base, b, u) = sandbox();
        web(&b.join("one"), "com.plinth.one");
        // A real directory junction/symlink needs privileges on Windows; where it can be made, it must not leak.
        let outside = base.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("leak.html"), "secret").unwrap();
        let link = b.join("one").join("link");
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_dir(&outside, &link).is_ok();
        #[cfg(not(windows))]
        let made = std::os::unix::fs::symlink(&outside, &link).is_ok();
        let s = WallpaperStore::new(b, u);
        if made {
            assert_eq!(s.serve("/com.plinth.one/link/leak.html", None).status, 404);
        }
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn import_copies_names_and_removes() {
        let (base, b, u) = sandbox();
        let src = base.join("My Loop (v2).MP4");
        std::fs::write(&src, vec![1u8; 4096]).unwrap();
        let mut s = WallpaperStore::new(b, u);
        let w = s.import_video(&src).unwrap();
        assert_eq!((w.id.as_str(), w.name.as_str(), w.kind), ("user.my-loop-v2", "My Loop (v2)", WallpaperKind::Video));
        let again = s.import_video(&src).unwrap();
        assert_eq!(again.id, "user.my-loop-v2-2");
        assert_eq!(s.list().len(), 2);
        assert!(src.exists(), "the original must be left alone");
        assert_eq!(s.serve(&format!("/{}/{}", w.id, w.entry), Some("bytes=0-9")).status, 206);
        s.remove(&w.id).unwrap();
        assert_eq!(s.list().len(), 1);
        assert!(s.remove("nope").is_err());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn import_rejects_wrong_types_and_empty_files() {
        let (base, b, u) = sandbox();
        let exe = base.join("evil.exe");
        std::fs::write(&exe, b"MZ").unwrap();
        let empty = base.join("empty.mp4");
        std::fs::write(&empty, b"").unwrap();
        let mut s = WallpaperStore::new(b, u);
        assert!(s.import_video(&exe).is_err());
        assert!(s.import_video(&empty).is_err());
        assert!(s.import_video(&base.join("missing.mp4")).is_err());
        assert!(s.import_video(&base).is_err());
        assert!(s.list().is_empty());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn bundled_wallpapers_cannot_be_removed() {
        let (base, b, u) = sandbox();
        web(&b.join("one"), "com.plinth.one");
        let mut s = WallpaperStore::new(b.clone(), u);
        assert!(s.remove("com.plinth.one").is_err());
        assert!(b.join("one").exists());
        let _ = std::fs::remove_dir_all(base);
    }
}
