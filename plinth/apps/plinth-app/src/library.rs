//! Wallpaper library engine (PRD W2, architecture 12): folders, scan, index file, dedupe, thumbnails, composing.
//! The decisions that can be unit-tested (diffing, search, order) live in `plinth_core::library`; this file does the file system
//! and image work. Nothing here deletes or modifies a user's image file: Plinth only reads them, and writes its own cache copies.

use image::imageops::{self, FilterType};
use image::{ImageBuffer, Rgb, RgbImage};
use plinth_core::fit::{self, FitMode};
use plinth_core::library::{self as core, Filter, Found, Item, Status, MAX_FILE_BYTES, MAX_SIDE, THUMB_SIDE};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const MAX_ROOTS: usize = 32;
const MAX_ENTRIES: usize = 400_000;
const MAX_DEPTH: usize = 24;
const PARTIAL: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    pub path: String,
    pub excludes: Vec<String>,
    pub bundled: bool,
}

pub struct Library {
    pub items: Vec<Item>,
    pub roots: Vec<Root>,
    file: PathBuf,
    pub thumbs: PathBuf,
    pub composed: PathBuf,
    /// Bumps on every change the hub should refresh for.
    pub revision: u64,
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn norm(p: &str) -> String {
    p.replace('/', "\\").trim_end_matches('\\').to_ascii_lowercase()
}

impl Library {
    pub fn load(file: PathBuf, thumbs: PathBuf, composed: PathBuf, bundled: Option<PathBuf>) -> Library {
        let _ = std::fs::create_dir_all(&thumbs);
        let _ = std::fs::create_dir_all(&composed);
        let mut lib = Library { items: Vec::new(), roots: Vec::new(), file, thumbs, composed, revision: 1 };
        if let Ok(text) = std::fs::read_to_string(&lib.file) {
            if let Ok(v) = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) {
                lib.items = v.get("items").and_then(Value::as_array).map(|a| a.iter().filter_map(|i| serde_json::from_value::<Item>(i.clone()).ok()).filter(|i| core::valid_key(&i.key)).collect()).unwrap_or_default();
                lib.roots = v
                    .get("roots")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|r| Some(Root { path: r.get("path")?.as_str()?.to_string(), excludes: r.get("excludes").and_then(Value::as_array).map(|e| e.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(), bundled: false })).collect())
                    .unwrap_or_default();
            }
        }
        if let Some(b) = bundled.filter(|b| b.is_dir()) {
            let bp = b.canonicalize().unwrap_or(b).to_string_lossy().trim_start_matches(r"\\?\").to_string();
            lib.roots.retain(|r| norm(&r.path) != norm(&bp));
            lib.roots.insert(0, Root { path: bp, excludes: Vec::new(), bundled: true });
        }
        lib
    }

    pub fn save(&self) {
        let v = json!({ "version": 1, "roots": self.roots.iter().filter(|r| !r.bundled).map(|r| json!({ "path": r.path, "excludes": r.excludes })).collect::<Vec<_>>(), "items": self.items });
        let tmp = self.file.with_extension("json.tmp");
        if let Some(d) = self.file.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if std::fs::write(&tmp, serde_json::to_vec(&v).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, &self.file);
        }
    }

    pub fn snapshot(&self) -> (Vec<Root>, Vec<Item>) {
        (self.roots.clone(), self.items.clone())
    }

    /// Merge a finished scan: take the scan's items but keep favorite/last-used edits made while it ran.
    pub fn apply_scan(&mut self, mut scanned: Vec<Item>) {
        let current: HashMap<&str, &Item> = self.items.iter().map(|i| (i.key.as_str(), i)).collect();
        for it in scanned.iter_mut() {
            if let Some(c) = current.get(it.key.as_str()) {
                it.favorite = c.favorite;
                it.last_used = c.last_used;
            }
        }
        // roots removed while the scan ran
        scanned.retain(|i| self.roots.iter().any(|r| r.path == i.root));
        if scanned != self.items {
            self.items = scanned;
            self.revision += 1;
        }
    }

    pub fn add_root(&mut self, path: &str) -> Result<(), String> {
        let p = Path::new(path.trim());
        if !p.is_dir() {
            return Err("That folder is not available.".into());
        }
        let canon = p.canonicalize().map_err(|_| "That folder is not available.".to_string())?.to_string_lossy().trim_start_matches(r"\\?\").to_string();
        if self.roots.iter().any(|r| norm(&r.path) == norm(&canon)) {
            return Err("That folder is already in your library.".into());
        }
        if self.roots.len() >= MAX_ROOTS {
            return Err("The library folder limit has been reached.".into());
        }
        self.roots.push(Root { path: canon, excludes: Vec::new(), bundled: false });
        self.revision += 1;
        Ok(())
    }

    pub fn remove_root(&mut self, path: &str) -> Result<(), String> {
        let i = self.roots.iter().position(|r| r.path == path).ok_or("Unknown folder.")?;
        if self.roots[i].bundled {
            return Err("The built-in wallpapers cannot be removed.".into());
        }
        let root = self.roots.remove(i);
        self.items.retain(|it| it.root != root.path);
        self.revision += 1;
        Ok(())
    }

    pub fn set_exclude(&mut self, root: &str, sub: &str, on: bool) -> Result<(), String> {
        let r = self.roots.iter_mut().find(|r| r.path == root).ok_or("Unknown folder.")?;
        let s = norm(sub);
        if !s.starts_with(&norm(&r.path)) || s == norm(&r.path) {
            return Err("Choose a folder inside the library folder.".into());
        }
        r.excludes.retain(|e| norm(e) != s);
        if on {
            r.excludes.push(sub.to_string());
        }
        self.revision += 1;
        Ok(())
    }

    pub fn find(&self, key: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.key == key && i.dup_of.is_none())
    }

    pub fn set_favorite(&mut self, key: &str, on: bool) -> bool {
        match self.items.iter_mut().find(|i| i.key == key && i.dup_of.is_none()) {
            Some(i) => {
                i.favorite = on;
                self.revision += 1;
                true
            }
            None => false,
        }
    }

    pub fn touch(&mut self, key: &str) {
        let t = now();
        if let Some(i) = self.items.iter_mut().find(|i| i.key == key && i.dup_of.is_none()) {
            i.last_used = t;
            self.revision += 1;
        }
    }

    /// Drop an entry from the index. The image file itself is never touched.
    pub fn remove_item(&mut self, key: &str) -> bool {
        let n = self.items.len();
        self.items.retain(|i| !(i.key == key || i.dup_of.as_deref() == Some(key)));
        let changed = self.items.len() != n;
        if changed {
            self.revision += 1;
        }
        changed
    }

    /// Point a missing item at a new file ("Locate"). The favorite flag and history stay.
    pub fn relocate(&mut self, key: &str, new_path: &str) -> Result<(), String> {
        if !core::is_image(new_path) || !Path::new(new_path).is_file() {
            return Err("Choose an image file (.jpg, .png, .webp or .bmp).".into());
        }
        let root = self.roots.iter().filter(|r| norm(new_path).starts_with(&norm(&r.path))).max_by_key(|r| r.path.len()).map(|r| r.path.clone());
        let old = self.items.iter().find(|i| i.key == key).cloned().ok_or("Unknown wallpaper.")?;
        let f = Found { path: new_path.to_string(), root: root.unwrap_or_else(|| old.root.clone()), size: std::fs::metadata(new_path).map_or(0, |m| m.len()), mtime: mtime(new_path) };
        let fresh = read_item(&f, now());
        if fresh.status != Status::Ok {
            return Err(format!("That file {}.", fresh.status.label()));
        }
        let it = self.items.iter_mut().find(|i| i.key == key).unwrap();
        let (fav, used, added) = (it.favorite, it.last_used, it.added);
        *it = Item { favorite: fav, last_used: used, added, ..fresh };
        self.revision += 1;
        Ok(())
    }

    pub fn thumb_file(&self, key: &str) -> PathBuf {
        self.thumbs.join(format!("{key}.jpg"))
    }

    /// A page of the library for the hub (virtualized grid).
    pub fn page(&self, q: &str, filter: Filter, offset: usize, limit: usize) -> Value {
        let idx = core::query(&self.items, q, filter);
        let total = idx.len();
        let items: Vec<Value> = idx
            .iter()
            .skip(offset)
            .take(limit.min(200))
            .map(|&i| {
                let it = &self.items[i];
                json!({ "key": it.key, "name": it.name(), "path": it.path, "w": it.width, "h": it.height, "fav": it.favorite, "status": it.status.label(), "ok": it.status == Status::Ok, "bundled": self.roots.iter().any(|r| r.bundled && r.path == it.root) })
            })
            .collect();
        json!({ "total": total, "offset": offset, "items": items })
    }

    pub fn summary(&self) -> Value {
        let visible = self.items.iter().filter(|i| i.dup_of.is_none()).count();
        let problems = self.items.iter().filter(|i| i.dup_of.is_none() && i.status != Status::Ok).count();
        json!({ "count": visible, "duplicates": self.items.len() - visible, "problems": problems, "revision": self.revision,
            "roots": self.roots.iter().map(|r| json!({ "path": r.path, "bundled": r.bundled, "excludes": r.excludes, "online": Path::new(&r.path).is_dir(), "count": self.items.iter().filter(|i| i.root == r.path && i.dup_of.is_none()).count() })).collect::<Vec<_>>() })
    }
}

// ------------------------------------------------------------------------------------------------ scanning

fn mtime(path: &str) -> u64 {
    std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
}

fn walk(root: &Root, out: &mut Vec<Found>, budget: &mut usize) {
    let excludes: Vec<String> = root.excludes.iter().map(|e| norm(e)).collect();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut stack: Vec<(PathBuf, usize)> = vec![(PathBuf::from(&root.path), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_DEPTH || *budget == 0 {
            continue;
        }
        // Junctions and links can loop; remember the real directory and never visit one twice.
        if let Ok(c) = dir.canonicalize() {
            if !seen.insert(c) {
                continue;
            }
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.filter_map(|e| e.ok()) {
            *budget = budget.saturating_sub(1);
            let Ok(ft) = e.file_type() else { continue };
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if ft.is_dir() {
                let low = name.to_ascii_lowercase();
                if low == "$recycle.bin" || low == "system volume information" || excludes.iter().any(|x| &norm(&p.to_string_lossy()) == x || norm(&p.to_string_lossy()).starts_with(&format!("{x}\\"))) {
                    continue;
                }
                stack.push((p, depth + 1));
            } else if ft.is_file() {
                let ps = p.to_string_lossy().to_string();
                if core::is_image(&ps) {
                    if let Ok(m) = e.metadata() {
                        out.push(Found { path: ps, root: root.path.clone(), size: m.len(), mtime: m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs()) });
                    }
                }
            }
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Content key: SHA-256 of the size plus the first and last 64 KB, 32 hex characters. Identical files always share it.
fn quick_key(path: &str, size: u64) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    h.update(size.to_le_bytes());
    let mut buf = vec![0u8; PARTIAL as usize];
    let n = f.read(&mut buf)?;
    h.update(&buf[..n]);
    if size > PARTIAL {
        f.seek(SeekFrom::Start(size.saturating_sub(PARTIAL).max(PARTIAL)))?;
        let n = f.read(&mut buf)?;
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize())[..32].to_string())
}

fn path_key(path: &str) -> String {
    hex(&Sha256::digest(norm(path).as_bytes()))[..32].to_string()
}

fn files_equal(a: &str, b: &str) -> bool {
    let (Ok(mut fa), Ok(mut fb)) = (std::fs::File::open(a), std::fs::File::open(b)) else { return false };
    let (mut ba, mut bb) = (vec![0u8; 1 << 16], vec![0u8; 1 << 16]);
    loop {
        let (Ok(na), Ok(nb)) = (fa.read(&mut ba), fb.read(&mut bb)) else { return false };
        if na != nb || ba[..na] != bb[..nb] {
            return false;
        }
        if na == 0 {
            return true;
        }
    }
}

/// Read one file's facts. Anything unreadable becomes an item with a problem status (never an error that stops the scan).
pub fn read_item(f: &Found, now: u64) -> Item {
    let mk = |key: String, status: Status, w: u32, h: u32| Item { key, path: f.path.clone(), root: f.root.clone(), size: f.size, mtime: f.mtime, width: w, height: h, added: now, last_used: 0, favorite: false, status, dup_of: None };
    if f.size > MAX_FILE_BYTES {
        return mk(path_key(&f.path), Status::TooLarge, 0, 0);
    }
    // Guess the format from the file's content, not its extension: a JPEG saved as .png is still a picture.
    let dims = image::ImageReader::open(&f.path).and_then(|r| r.with_guessed_format()).map_err(|e| e.to_string()).and_then(|r| r.into_dimensions().map_err(|e| e.to_string()));
    match dims {
        Ok((w, h)) if w > MAX_SIDE || h > MAX_SIDE || w == 0 || h == 0 => mk(path_key(&f.path), Status::TooLarge, w, h),
        Ok((w, h)) => match quick_key(&f.path, f.size) {
            Ok(k) => mk(k, Status::Ok, w, h),
            Err(_) => mk(path_key(&f.path), Status::Corrupt, w, h),
        },
        Err(_) => mk(path_key(&f.path), Status::Corrupt, 0, 0),
    }
}

/// Scan every folder and return the new item list. Works on a snapshot, so it can run without holding any lock.
pub fn scan(roots: &[Root], old: &[Item]) -> Vec<Item> {
    let t = now();
    let mut found: Vec<Found> = Vec::new();
    let (mut reachable, mut offline): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    let mut budget = MAX_ENTRIES;
    for r in roots {
        if Path::new(&r.path).is_dir() {
            reachable.push(r.path.clone());
            walk(r, &mut found, &mut budget);
        } else {
            offline.push(r.path.clone());
        }
    }
    let mut out: Vec<Item> = old.iter().filter(|i| roots.iter().any(|r| r.path == i.root)).cloned().collect();
    let diff = core::diff_scan(&out, &found, &reachable);
    for &i in &diff.gone {
        out[i].status = Status::Missing;
    }
    for it in out.iter_mut() {
        if offline.contains(&it.root) && it.status == Status::Ok {
            it.status = Status::Offline;
        }
    }
    for &i in &diff.back {
        out[i].status = Status::Ok;
    }
    for (i, f) in &diff.moved {
        out[*i].path = f.path.clone();
        out[*i].root = f.root.clone();
        out[*i].status = Status::Ok;
    }
    let mut touched: HashSet<String> = HashSet::new();
    for (i, f) in &diff.changed {
        let fresh = read_item(f, t);
        let old_it = &out[*i];
        out[*i] = Item { favorite: old_it.favorite, last_used: old_it.last_used, added: old_it.added, ..fresh };
        touched.insert(out[*i].path.clone());
    }
    for f in &diff.new {
        let it = read_item(f, t);
        touched.insert(it.path.clone());
        out.push(it);
    }
    dedupe(&mut out, &touched);
    out
}

/// Mark byte-identical copies. A pair is verified by comparing the files only when one of them is new or changed in this scan.
fn dedupe(items: &mut [Item], touched: &HashSet<String>) {
    // Keep existing duplicate links that are still valid (their original exists and is readable).
    let ok_keys: HashSet<String> = items.iter().filter(|i| i.dup_of.is_none() && i.status == Status::Ok).map(|i| i.key.clone()).collect();
    for it in items.iter_mut() {
        if it.dup_of.as_ref().map_or(false, |k| !ok_keys.contains(k)) {
            it.dup_of = None;
        }
    }
    let mut canon: HashMap<String, usize> = HashMap::new();
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| items[a].added.cmp(&items[b].added).then(items[a].path.cmp(&items[b].path)));
    for i in order {
        if items[i].status != Status::Ok || items[i].dup_of.is_some() {
            continue;
        }
        match canon.get(&items[i].key).copied() {
            None => {
                canon.insert(items[i].key.clone(), i);
            }
            Some(c) => {
                let needs_check = touched.contains(&items[i].path) || touched.contains(&items[c].path);
                if !needs_check || (items[i].size == items[c].size && files_equal(&items[i].path, &items[c].path)) {
                    items[i].dup_of = Some(items[i].key.clone());
                } else {
                    // Same partial key but different bytes: give this file its own key from the whole file.
                    let full = std::fs::read(&items[i].path).map(|b| hex(&Sha256::digest(&b))[..32].to_string()).unwrap_or_else(|_| path_key(&items[i].path));
                    items[i].key = full;
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------ images

fn decode(path: &str) -> Result<RgbImage, String> {
    let mut r = image::ImageReader::open(path).map_err(|e| e.to_string())?.with_guessed_format().map_err(|e| e.to_string())?;
    let mut l = image::Limits::default();
    l.max_image_width = Some(MAX_SIDE);
    l.max_image_height = Some(MAX_SIDE);
    l.max_alloc = Some(1 << 30);
    r.limits(l);
    Ok(r.decode().map_err(|e| e.to_string())?.to_rgb8())
}

fn save_jpeg(img: &RgbImage, dest: &Path, quality: u8) -> Result<(), String> {
    let tmp = dest.with_extension("tmp");
    let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    img.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(&mut f, quality)).map_err(|e| e.to_string())?;
    f.flush().map_err(|e| e.to_string())?;
    drop(f);
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

/// Write the 320 px thumbnail of `src` to `dest` (atomic). Large images are down-scaled while decoding limits keep memory bounded.
pub fn make_thumb(src: &str, dest: &Path) -> Result<(), String> {
    let img = decode(src)?;
    let (w, h) = (img.width(), img.height());
    let s = (THUMB_SIDE as f32 / w.max(h) as f32).min(1.0);
    let (tw, th) = (((w as f32 * s).round() as u32).max(1), ((h as f32 * s).round() as u32).max(1));
    let small = imageops::resize(&img, tw, th, FilterType::Triangle);
    save_jpeg(&small, dest, 80)
}

/// File name of a composed picture; deterministic so a restart finds it again. Only `[a-z0-9._-]`.
pub fn composed_name(key: &str, canvas: (u32, u32), mode: FitMode, bg: &str, span_off: (i32, i32), span_total: (u32, u32)) -> String {
    let n = |v: i32| if v < 0 { format!("m{}", -v) } else { v.to_string() };
    let bg: String = bg.trim_start_matches('#').to_ascii_lowercase().chars().filter(|c| c.is_ascii_hexdigit()).collect();
    format!("{key}-{}x{}-{}-{bg}-{}_{}_{}x{}.jpg", canvas.0, canvas.1, mode.as_str(), n(span_off.0), n(span_off.1), span_total.0, span_total.1)
}

pub fn valid_composed_name(name: &str) -> bool {
    name.len() < 140 && name.ends_with(".jpg") && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'))
}

/// Paint `src` onto a monitor-sized canvas according to the fit mode and save it as a JPEG.
pub fn compose(src: &str, canvas: (u32, u32), mode: FitMode, bg: (u8, u8, u8), span_off: (i32, i32), span_total: (u32, u32), dest: &Path) -> Result<(), String> {
    if canvas.0 == 0 || canvas.1 == 0 || canvas.0 > 16384 || canvas.1 > 16384 {
        return Err("Unusable monitor size.".into());
    }
    let img = decode(src)?;
    let plan = fit::plan(mode, (img.width(), img.height()), canvas, span_off, span_total);
    let mut out: RgbImage = ImageBuffer::from_pixel(canvas.0, canvas.1, Rgb([bg.0, bg.1, bg.2]));
    if plan.tile {
        let (tw, th) = (img.width() as i64, img.height() as i64);
        let mut y = 0i64;
        while y < canvas.1 as i64 {
            let mut x = 0i64;
            while x < canvas.0 as i64 {
                imageops::overlay(&mut out, &img, x, y);
                x += tw;
            }
            y += th;
        }
    } else {
        let sx = plan.src.x.round().clamp(0.0, img.width() as f64 - 1.0) as u32;
        let sy = plan.src.y.round().clamp(0.0, img.height() as f64 - 1.0) as u32;
        let sw = (plan.src.w.round() as u32).clamp(1, img.width() - sx);
        let sh = (plan.src.h.round() as u32).clamp(1, img.height() - sy);
        let (dw, dh) = ((plan.dst.w.round() as u32).max(1), (plan.dst.h.round() as u32).max(1));
        let crop = imageops::crop_imm(&img, sx, sy, sw, sh).to_image();
        let filter = if sw > dw * 2 { FilterType::Triangle } else { FilterType::CatmullRom };
        let scaled = imageops::resize(&crop, dw, dh, filter);
        imageops::overlay(&mut out, &scaled, plan.dst.x.round() as i64, plan.dst.y.round() as i64);
    }
    save_jpeg(&out, dest, 92)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static N: AtomicU32 = AtomicU32::new(0);

    fn sandbox() -> PathBuf {
        let d = std::env::temp_dir().join(format!("plinth-lib-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn png(path: &Path, w: u32, h: u32, shade: u8) {
        let img: RgbImage = ImageBuffer::from_fn(w, h, |x, y| Rgb([shade, (x * 255 / w.max(1)) as u8, (y * 255 / h.max(1)) as u8]));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        img.save(path).unwrap();
    }

    fn lib_in(d: &Path, roots: &[&Path]) -> Library {
        let mut l = Library::load(d.join("lib.json"), d.join("thumbs"), d.join("composed"), None);
        for r in roots {
            l.add_root(&r.to_string_lossy()).unwrap();
        }
        l
    }

    fn rescan(l: &mut Library) {
        let (roots, items) = l.snapshot();
        let out = scan(&roots, &items);
        l.apply_scan(out);
    }

    #[test]
    fn scans_recursively_and_skips_non_images_and_excluded_folders() {
        let d = sandbox();
        let w = d.join("walls");
        png(&w.join("a.png"), 64, 36, 10);
        png(&w.join("sub").join("b.png"), 64, 36, 20);
        png(&w.join("skipme").join("c.png"), 64, 36, 30);
        std::fs::write(w.join("notes.txt"), "x").unwrap();
        let mut l = lib_in(&d, &[&w]);
        let root = l.roots[0].path.clone();
        l.set_exclude(&root, &w.join("skipme").to_string_lossy(), true).unwrap();
        rescan(&mut l);
        let mut names: Vec<_> = l.items.iter().map(|i| i.name().to_string()).collect();
        names.sort();
        assert_eq!(names, vec!["a.png", "b.png"]);
        assert!(l.items.iter().all(|i| i.status == Status::Ok && i.width == 64 && core::valid_key(&i.key)));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn new_deleted_renamed_files_are_tracked_without_losing_favorites() {
        let d = sandbox();
        let w = d.join("walls");
        png(&w.join("a.png"), 64, 36, 10);
        png(&w.join("b.png"), 80, 40, 20);
        let mut l = lib_in(&d, &[&w]);
        rescan(&mut l);
        let akey = l.items.iter().find(|i| i.name() == "a.png").unwrap().key.clone();
        assert!(l.set_favorite(&akey, true));
        // new file
        png(&w.join("c.png"), 48, 48, 30);
        // rename a.png -> moved/renamed.png (same bytes, same mtime)
        std::fs::create_dir_all(w.join("moved")).unwrap();
        std::fs::rename(w.join("a.png"), w.join("moved").join("renamed.png")).unwrap();
        // delete b.png
        std::fs::remove_file(w.join("b.png")).unwrap();
        rescan(&mut l);
        let by = |n: &str| l.items.iter().find(|i| i.name() == n).cloned();
        assert!(by("c.png").is_some(), "new file appears");
        let r = by("renamed.png").expect("renamed file is the same item");
        assert!(r.favorite && r.key == akey && r.status == Status::Ok, "favorite and key survive a rename");
        assert!(by("a.png").is_none());
        assert_eq!(by("b.png").unwrap().status, Status::Missing, "a deleted file is shown as missing, not silently dropped");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn identical_files_are_one_item_and_different_files_with_equal_size_are_not() {
        let d = sandbox();
        let w = d.join("walls");
        png(&w.join("orig.png"), 64, 36, 10);
        std::fs::copy(w.join("orig.png"), w.join("copy.png")).unwrap();
        png(&w.join("other.png"), 64, 36, 99);
        let mut l = lib_in(&d, &[&w]);
        rescan(&mut l);
        let shown: Vec<_> = core::query(&l.items, "", Filter::All).into_iter().map(|i| l.items[i].name().to_string()).collect();
        assert_eq!(shown.len(), 2, "{shown:?}");
        assert_eq!(l.summary()["duplicates"], 1);
        assert_eq!(l.items.iter().filter(|i| i.dup_of.is_some()).count(), 1);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn corrupt_files_and_unavailable_folders_degrade_gracefully() {
        let d = sandbox();
        let w = d.join("walls");
        png(&w.join("ok.png"), 64, 36, 10);
        std::fs::write(w.join("broken.jpg"), b"this is not a jpeg").unwrap();
        let mut l = lib_in(&d, &[&w]);
        rescan(&mut l);
        let broken = l.items.iter().find(|i| i.name() == "broken.jpg").unwrap();
        assert_eq!(broken.status, Status::Corrupt);
        assert_eq!(l.items.iter().find(|i| i.name() == "ok.png").unwrap().status, Status::Ok);
        // the folder goes away (unplugged drive): items are kept and marked offline
        let moved = d.join("walls-away");
        std::fs::rename(&w, &moved).unwrap();
        rescan(&mut l);
        assert!(l.items.iter().all(|i| i.status == Status::Offline || i.status == Status::Corrupt));
        assert_eq!(l.items.len(), 2, "nothing was deleted from the library");
        // and it comes back
        std::fs::rename(&moved, &w).unwrap();
        rescan(&mut l);
        assert_eq!(l.items.iter().find(|i| i.name() == "ok.png").unwrap().status, Status::Ok);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn thumbnails_are_small_and_compose_matches_the_monitor_exactly() {
        let d = sandbox();
        let w = d.join("walls");
        png(&w.join("big.png"), 1600, 1200, 10);
        let mut l = lib_in(&d, &[&w]);
        rescan(&mut l);
        let it = l.items[0].clone();
        let t = l.thumb_file(&it.key);
        make_thumb(&it.path, &t).unwrap();
        let (tw, th) = image::image_dimensions(&t).unwrap();
        assert!(tw.max(th) <= THUMB_SIDE && tw == 320, "{tw}x{th}");
        for (mode, name) in [(FitMode::Fill, "fill"), (FitMode::Fit, "fit"), (FitMode::Stretch, "stretch"), (FitMode::Center, "center"), (FitMode::Tile, "tile"), (FitMode::Span, "span")] {
            let dest = d.join(format!("composed-{name}.jpg"));
            compose(&it.path, (1920, 1080), mode, (0, 0, 0), (0, 0), (3840, 1080), &dest).unwrap();
            assert_eq!(image::image_dimensions(&dest).unwrap(), (1920, 1080), "{name}");
        }
        // Fit leaves letterbox bars in the requested color
        let dest = d.join("bars.jpg");
        compose(&it.path, (1920, 1080), FitMode::Fit, (255, 0, 0), (0, 0), (1920, 1080), &dest).unwrap();
        let px = image::open(&dest).unwrap().to_rgb8();
        let p = px.get_pixel(10, 540);
        assert!(p[0] > 200 && p[1] < 60 && p[2] < 60, "left bar should be red, got {p:?}");
        let _ = std::fs::remove_dir_all(d);
    }

    /// Benchmark, run by hand: PLINTH_BENCH_DIR=<folder of many pictures> cargo test --release -p plinth-app bench_scan -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bench_scan() {
        let Some(dir) = std::env::var_os("PLINTH_BENCH_DIR") else { return };
        let d = sandbox();
        let mut l = Library::load(d.join("lib.json"), d.join("thumbs"), d.join("composed"), None);
        l.add_root(&dir.to_string_lossy()).unwrap();
        let t = std::time::Instant::now();
        let (roots, items) = l.snapshot();
        l.apply_scan(scan(&roots, &items));
        let first = t.elapsed();
        let n = l.items.len();
        let t = std::time::Instant::now();
        let (roots, items) = l.snapshot();
        l.apply_scan(scan(&roots, &items));
        let again = t.elapsed();
        let t = std::time::Instant::now();
        l.save();
        let save = t.elapsed();
        let t = std::time::Instant::now();
        let page = l.page("", Filter::All, 5000, 60);
        let q1 = t.elapsed();
        let t = std::time::Instant::now();
        let _ = l.page("pic_0042", Filter::All, 0, 60);
        let q2 = t.elapsed();
        let t = std::time::Instant::now();
        let loaded = Library::load(d.join("lib.json"), d.join("thumbs"), d.join("composed"), None);
        let load = t.elapsed();
        println!("BENCH items={n} first_scan={first:?} unchanged_rescan={again:?} save={save:?} ({} KB) load={load:?} ({}) page_query={q1:?} search={q2:?} page_items={}", std::fs::metadata(d.join("lib.json")).map_or(0, |m| m.len() / 1024), loaded.items.len(), page["items"].as_array().map_or(0, |a| a.len()));
        let _ = std::fs::remove_dir_all(d);
    }

    /// Benchmark, run by hand: PLINTH_BENCH_IMAGES=<folder of large pictures> cargo test --release -p plinth-app bench_thumbs -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bench_thumbs() {
        let Some(dir) = std::env::var_os("PLINTH_BENCH_IMAGES") else { return };
        let d = sandbox();
        let files: Vec<String> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path().to_string_lossy().to_string()).filter(|p| core::is_image(p)).collect();
        let t = std::time::Instant::now();
        for (i, f) in files.iter().enumerate() {
            make_thumb(f, &d.join(format!("t{i}.jpg"))).unwrap();
        }
        let per = t.elapsed() / files.len().max(1) as u32;
        let dims = image::image_dimensions(&files[0]).unwrap();
        let t = std::time::Instant::now();
        compose(&files[0], (1920, 1080), FitMode::Fill, (0, 0, 0), (0, 0), (1920, 1080), &d.join("c.jpg")).unwrap();
        println!("BENCH thumbs: {} pictures of {}x{}: {:?} each (one thread); compose to 1080p: {:?}", files.len(), dims.0, dims.1, per, t.elapsed());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn composed_names_are_file_safe_and_deterministic() {
        let n = composed_name("0123456789abcdef0123456789abcdef", (1920, 1080), FitMode::Span, "#1A2B3C", (-1280, 0), (3200, 1080));
        assert!(valid_composed_name(&n), "{n}");
        assert_eq!(n, composed_name("0123456789abcdef0123456789abcdef", (1920, 1080), FitMode::Span, "#1a2b3c", (-1280, 0), (3200, 1080)));
        for bad in ["../x.jpg", "a/b.jpg", "A.jpg", "x.exe", "a b.jpg", ""] {
            assert!(!valid_composed_name(bad), "{bad}");
        }
    }

    #[test]
    fn roots_have_guard_rails_and_the_index_round_trips() {
        let d = sandbox();
        let w = d.join("walls");
        png(&w.join("a.png"), 64, 36, 10);
        let mut l = lib_in(&d, &[&w]);
        assert!(l.add_root(&w.to_string_lossy()).is_err(), "duplicate folder");
        assert!(l.add_root(&d.join("nope").to_string_lossy()).is_err(), "missing folder");
        rescan(&mut l);
        l.save();
        let l2 = Library::load(d.join("lib.json"), d.join("thumbs"), d.join("composed"), None);
        assert_eq!(l2.items.len(), 1);
        assert_eq!(l2.roots.len(), 1);
        let root = l.roots[0].path.clone();
        l.remove_root(&root).unwrap();
        assert!(l.items.is_empty());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_picture_with_the_wrong_extension_is_still_a_picture() {
        let d = sandbox();
        let w = d.join("walls");
        png(&w.join("real.png"), 64, 36, 10);
        let jpg: RgbImage = ImageBuffer::from_pixel(32, 32, Rgb([5, 5, 200]));
        jpg.save(w.join("actually-jpeg.jpg")).unwrap();
        std::fs::copy(w.join("actually-jpeg.jpg"), w.join("misnamed.png")).unwrap();
        let mut l = lib_in(&d, &[&w]);
        rescan(&mut l);
        let m = l.items.iter().find(|i| i.name() == "misnamed.png").unwrap();
        assert!(m.status == Status::Ok || m.dup_of.is_some(), "{:?}", m.status);
        assert_eq!((m.width, m.height), (32, 32));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn locate_and_remove_work_on_missing_items() {
        let d = sandbox();
        let w = d.join("walls");
        png(&w.join("a.png"), 64, 36, 10);
        let mut l = lib_in(&d, &[&w]);
        rescan(&mut l);
        let key = l.items[0].key.clone();
        l.set_favorite(&key, true);
        std::fs::remove_file(w.join("a.png")).unwrap();
        rescan(&mut l);
        assert_eq!(l.items[0].status, Status::Missing);
        png(&d.join("elsewhere").join("again.png"), 64, 36, 10);
        assert!(l.relocate(&key, &d.join("elsewhere").join("again.txt").to_string_lossy()).is_err());
        l.relocate(&key, &d.join("elsewhere").join("again.png").to_string_lossy()).unwrap();
        assert!(l.items[0].favorite && l.items[0].status == Status::Ok);
        assert!(l.remove_item(&key));
        assert!(l.items.is_empty());
        assert!(d.join("elsewhere").join("again.png").exists(), "the image file is never deleted");
        let _ = std::fs::remove_dir_all(d);
    }
}
