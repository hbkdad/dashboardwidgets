//! Wallpaper library logic (PRD W2, architecture 12): item model, scan diffing (new / gone / moved / changed), search and
//! filtering, and slideshow order. Pure: the file system walk, decoding and thumbnails live in the app.
//!
//! Rules this module enforces and tests: a missing file is never silently dropped (it becomes `Missing`, and `Offline` when its
//! whole folder is unavailable); a renamed or moved file keeps its favorite flag, thumbnail key and history; identical files are
//! one item.

use serde::{Deserialize, Serialize};

pub const IMAGE_EXTENSIONS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "bmp"];
/// Refuse to decode anything larger than this on a side (decompression-bomb guard).
pub const MAX_SIDE: u32 = 16384;
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
pub const THUMB_SIDE: u32 = 320;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    /// The file is gone but its folder is reachable.
    Missing,
    /// The whole library folder is unreachable (drive unplugged, network share down). Items are kept, dimmed.
    Offline,
    /// The header could not be read or the decode failed.
    Corrupt,
    TooLarge,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Ok => "ready",
            Status::Missing => "file missing",
            Status::Offline => "folder offline",
            Status::Corrupt => "cannot be read",
            Status::TooLarge => "too large",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// Content key: identical files share it. Also the thumbnail file name.
    pub key: String,
    pub path: String,
    /// The library folder this file was found under.
    pub root: String,
    pub size: u64,
    pub mtime: u64,
    pub width: u32,
    pub height: u32,
    pub added: u64,
    pub last_used: u64,
    pub favorite: bool,
    pub status: Status,
    /// Set when this file is a byte-identical copy of another library item (its key); duplicates are hidden from lists.
    #[serde(default)]
    pub dup_of: Option<String>,
}

impl Item {
    pub fn name(&self) -> &str {
        self.path.rsplit(['\\', '/']).next().unwrap_or(&self.path)
    }

    pub fn aspect(&self) -> f32 {
        if self.height == 0 {
            0.0
        } else {
            self.width as f32 / self.height as f32
        }
    }

    pub fn ext(&self) -> String {
        extension(&self.path)
    }
}

pub fn extension(path: &str) -> String {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

pub fn is_image(path: &str) -> bool {
    IMAGE_EXTENSIONS.contains(&extension(path).as_str())
}

/// A key is lowercase hex, 16 to 64 characters (used as a file name, so it must never contain path characters).
pub fn valid_key(k: &str) -> bool {
    (16..=64).contains(&k.len()) && k.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// One file seen by a scan.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub path: String,
    pub root: String,
    pub size: u64,
    pub mtime: u64,
}

#[derive(Debug, Default, PartialEq)]
pub struct ScanDiff {
    /// Files not in the library yet.
    pub new: Vec<Found>,
    /// Library items whose file is gone: indices into `old`.
    pub gone: Vec<usize>,
    /// Library items found again at another path with the same size and mtime (rename or move): (index in `old`, new location).
    pub moved: Vec<(usize, Found)>,
    /// Same path but size or mtime changed: (index in `old`, new facts). Needs a re-read.
    pub changed: Vec<(usize, Found)>,
    /// Unchanged items that were Missing/Offline before and are back: indices into `old`.
    pub back: Vec<usize>,
}

/// Compare the library with what a scan found. `reachable_roots` lists the library folders the scan could actually read; an
/// item under a folder that is not in that list is never reported as gone, it is `offline` (the caller marks it).
pub fn diff_scan(old: &[Item], found: &[Found], reachable_roots: &[String]) -> ScanDiff {
    let mut d = ScanDiff::default();
    let mut used = vec![false; found.len()];
    let mut matched_old = vec![false; old.len()];
    let norm = |p: &str| p.replace('/', "\\").to_ascii_lowercase();
    let by_path: std::collections::HashMap<String, usize> = found.iter().enumerate().map(|(i, f)| (norm(&f.path), i)).collect();
    for (oi, it) in old.iter().enumerate() {
        if let Some(&fi) = by_path.get(&norm(&it.path)) {
            used[fi] = true;
            matched_old[oi] = true;
            let f = &found[fi];
            if f.size != it.size || f.mtime != it.mtime {
                d.changed.push((oi, f.clone()));
            } else if it.status == Status::Missing || it.status == Status::Offline {
                d.back.push(oi);
            }
        }
    }
    // Renames and moves: an unmatched old item whose (size, mtime) equals exactly one unmatched found file.
    for (oi, it) in old.iter().enumerate() {
        if matched_old[oi] {
            continue;
        }
        let cands: Vec<usize> = found.iter().enumerate().filter(|(fi, f)| !used[*fi] && f.size == it.size && f.mtime == it.mtime).map(|(fi, _)| fi).collect();
        if cands.len() == 1 {
            used[cands[0]] = true;
            matched_old[oi] = true;
            d.moved.push((oi, found[cands[0]].clone()));
        }
    }
    for (oi, it) in old.iter().enumerate() {
        if !matched_old[oi] && reachable_roots.iter().any(|r| r == &it.root) && it.status != Status::Missing {
            d.gone.push(oi);
        }
    }
    d.new = found.iter().enumerate().filter(|(i, _)| !used[*i]).map(|(_, f)| f.clone()).collect();
    d
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Favorites,
    Recent,
    Problems,
}

impl Filter {
    pub fn parse(s: &str) -> Filter {
        match s {
            "favorites" => Filter::Favorites,
            "recent" => Filter::Recent,
            "problems" => Filter::Problems,
            _ => Filter::All,
        }
    }
}

/// Indices of the items to show, in display order. Search is case-insensitive over the file name, folder and resolution text
/// (`1920x1080`, `4k`); `Recent` lists items used before, newest first; others sort by name.
pub fn query(items: &[Item], q: &str, filter: Filter) -> Vec<usize> {
    let q = q.trim().to_ascii_lowercase();
    let mut idx: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, it)| it.dup_of.is_none())
        .filter(|(_, it)| match filter {
            Filter::All => true,
            Filter::Favorites => it.favorite,
            Filter::Recent => it.last_used > 0,
            Filter::Problems => it.status != Status::Ok,
        })
        .filter(|(_, it)| q.is_empty() || matches(it, &q))
        .map(|(i, _)| i)
        .collect();
    if filter == Filter::Recent {
        idx.sort_by(|&a, &b| items[b].last_used.cmp(&items[a].last_used));
    } else {
        idx.sort_by(|&a, &b| items[a].name().to_ascii_lowercase().cmp(&items[b].name().to_ascii_lowercase()).then(items[a].path.cmp(&items[b].path)));
    }
    idx
}

fn matches(it: &Item, q: &str) -> bool {
    let hay = format!("{} {} {}x{}", it.name(), it.path, it.width, it.height).to_ascii_lowercase();
    if q.split_whitespace().all(|w| hay.contains(w)) {
        return true;
    }
    let k = if it.width >= 7000 { "8k" } else if it.width >= 3500 { "4k" } else if it.width >= 2500 { "1440p 2k" } else if it.width >= 1900 { "1080p" } else { "" };
    !k.is_empty() && q.split_whitespace().all(|w| k.contains(w))
}

/// Next index in a slideshow of `len` items. Sequential wraps around; shuffle picks a different item from `seed` (never the
/// current one when there is more than one).
pub fn next_index(len: usize, current: Option<usize>, shuffle: bool, seed: u64) -> Option<usize> {
    if len == 0 {
        return None;
    }
    if len == 1 {
        return Some(0);
    }
    let cur = current.filter(|&c| c < len);
    if shuffle {
        // xorshift mix so consecutive seeds do not give consecutive picks
        let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let mut n = (x % (len as u64 - cur.map_or(0, |_| 1))) as usize;
        if let Some(c) = cur {
            if n >= c {
                n += 1;
            }
        }
        Some(n)
    } else {
        Some(cur.map_or(0, |c| (c + 1) % len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &str, root: &str, size: u64, mtime: u64) -> Item {
        Item { key: format!("{:016x}", size ^ mtime), path: path.into(), root: root.into(), size, mtime, width: 1920, height: 1080, added: 1, last_used: 0, favorite: false, status: Status::Ok, dup_of: None }
    }

    fn found(path: &str, root: &str, size: u64, mtime: u64) -> Found {
        Found { path: path.into(), root: root.into(), size, mtime }
    }

    const R: &str = r"D:\Walls";

    #[test]
    fn unchanged_files_produce_an_empty_diff() {
        let old = vec![item(r"D:\Walls\a.jpg", R, 10, 5), item(r"D:\Walls\b.png", R, 20, 6)];
        let f = vec![found(r"D:\Walls\a.jpg", R, 10, 5), found(r"d:/walls/B.PNG", R, 20, 6)];
        assert_eq!(diff_scan(&old, &f, &[R.into()]), ScanDiff::default());
    }

    #[test]
    fn new_files_are_detected() {
        let old = vec![item(r"D:\Walls\a.jpg", R, 10, 5)];
        let f = vec![found(r"D:\Walls\a.jpg", R, 10, 5), found(r"D:\Walls\new.jpg", R, 30, 9)];
        let d = diff_scan(&old, &f, &[R.into()]);
        assert_eq!(d.new.len(), 1);
        assert!(d.gone.is_empty() && d.moved.is_empty());
    }

    #[test]
    fn deleted_files_are_gone_only_when_their_folder_was_reachable() {
        let old = vec![item(r"D:\Walls\a.jpg", R, 10, 5)];
        let d = diff_scan(&old, &[], &[R.into()]);
        assert_eq!(d.gone, vec![0]);
        // folder unreachable (unplugged drive): nothing is reported gone, the caller marks the items offline
        let d2 = diff_scan(&old, &[], &[]);
        assert!(d2.gone.is_empty());
    }

    #[test]
    fn a_rename_or_move_keeps_the_item() {
        let old = vec![item(r"D:\Walls\a.jpg", R, 10, 5)];
        let f = vec![found(r"D:\Walls\sub\renamed.jpg", R, 10, 5)];
        let d = diff_scan(&old, &f, &[R.into()]);
        assert_eq!(d.moved.len(), 1);
        assert_eq!(d.moved[0].0, 0);
        assert!(d.new.is_empty() && d.gone.is_empty());
    }

    #[test]
    fn ambiguous_matches_are_not_guessed() {
        // two new files with the same size and mtime as the vanished one: do not pick one, treat the old as gone and both as new
        let old = vec![item(r"D:\Walls\a.jpg", R, 10, 5)];
        let f = vec![found(r"D:\Walls\x.jpg", R, 10, 5), found(r"D:\Walls\y.jpg", R, 10, 5)];
        let d = diff_scan(&old, &f, &[R.into()]);
        assert!(d.moved.is_empty());
        assert_eq!((d.gone.len(), d.new.len()), (1, 2));
    }

    #[test]
    fn changed_files_are_flagged_for_a_re_read() {
        let old = vec![item(r"D:\Walls\a.jpg", R, 10, 5)];
        let f = vec![found(r"D:\Walls\a.jpg", R, 11, 7)];
        let d = diff_scan(&old, &f, &[R.into()]);
        assert_eq!(d.changed.len(), 1);
    }

    #[test]
    fn missing_or_offline_items_come_back_when_the_file_returns() {
        let mut a = item(r"D:\Walls\a.jpg", R, 10, 5);
        a.status = Status::Offline;
        let mut b = item(r"D:\Walls\b.jpg", R, 11, 5);
        b.status = Status::Missing;
        let f = vec![found(r"D:\Walls\a.jpg", R, 10, 5), found(r"D:\Walls\b.jpg", R, 11, 5)];
        let d = diff_scan(&[a, b], &f, &[R.into()]);
        assert_eq!(d.back, vec![0, 1]);
    }

    #[test]
    fn already_missing_items_are_not_reported_gone_again() {
        let mut a = item(r"D:\Walls\a.jpg", R, 10, 5);
        a.status = Status::Missing;
        assert!(diff_scan(&[a], &[], &[R.into()]).gone.is_empty());
    }

    #[test]
    fn search_filters_and_sorts() {
        let mut items = vec![item(r"D:\Walls\Forest dawn.jpg", R, 1, 1), item(r"D:\Walls\city night.png", R, 2, 2), item(r"D:\Walls\Beach.jpg", R, 3, 3)];
        items[1].favorite = true;
        items[2].last_used = 50;
        items[0].last_used = 80;
        items[1].width = 3840;
        items[1].height = 2160;
        fn names(items: &[Item], v: Vec<usize>) -> Vec<String> {
            v.into_iter().map(|i| items[i].name().to_string()).collect()
        }
        assert_eq!(names(&items, query(&items, "", Filter::All)), vec!["Beach.jpg", "city night.png", "Forest dawn.jpg"]);
        assert_eq!(names(&items, query(&items, "FOREST", Filter::All)), vec!["Forest dawn.jpg"]);
        assert_eq!(names(&items, query(&items, "night 3840", Filter::All)), vec!["city night.png"]);
        assert_eq!(names(&items, query(&items, "4k", Filter::All)), vec!["city night.png"]);
        assert_eq!(names(&items, query(&items, "", Filter::Favorites)), vec!["city night.png"]);
        assert_eq!(names(&items, query(&items, "", Filter::Recent)), vec!["Forest dawn.jpg", "Beach.jpg"]);
        assert!(query(&items, "zzz", Filter::All).is_empty());
        items[0].status = Status::Missing;
        assert_eq!(names(&items, query(&items, "", Filter::Problems)), vec!["Forest dawn.jpg"]);
    }

    #[test]
    fn duplicates_are_hidden_from_lists() {
        let mut items = vec![item(r"D:\Walls.jpg", R, 1, 1), item(r"D:\Walls\copy of a.jpg", R, 1, 1)];
        items[1].dup_of = Some(items[0].key.clone());
        assert_eq!(query(&items, "", Filter::All), vec![0]);
    }

    #[test]
    fn slideshow_order() {
        assert_eq!(next_index(0, None, false, 1), None);
        assert_eq!(next_index(1, Some(0), true, 9), Some(0));
        assert_eq!(next_index(3, None, false, 0), Some(0));
        assert_eq!(next_index(3, Some(2), false, 0), Some(0));
        for seed in 0..500u64 {
            let n = next_index(5, Some(2), true, seed).unwrap();
            assert!(n < 5 && n != 2, "shuffle must pick a different item (seed {seed} gave {n})");
        }
        let picks: std::collections::HashSet<_> = (0..500u64).filter_map(|s| next_index(5, Some(2), true, s)).collect();
        assert_eq!(picks.len(), 4, "every other item should be reachable");
    }

    #[test]
    fn keys_extensions_and_status_names() {
        assert!(valid_key("0123456789abcdef"));
        for bad in ["", "short", "0123456789abcdeg", "../../etc/passwd0000", "0123456789ABCDEF", &"a".repeat(65)] {
            assert!(!valid_key(bad), "{bad}");
        }
        assert!(is_image(r"C:\x\A.JPG") && is_image("b.webp") && is_image("c.bmp") && !is_image("d.gif") && !is_image("e.exe") && !is_image("noext"));
        assert_eq!(Status::Missing.label(), "file missing");
    }
}
