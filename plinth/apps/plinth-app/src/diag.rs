//! Diagnostics for support: a log file the app writes while it runs, and a text report the user can read, copy and send.
//! The report is built to hold no names, files or hardware ids: the user name inside paths is replaced, monitor names and ids are
//! left out (only counts, sizes and scale), and library contents are counted, never listed.

use std::path::{Path, PathBuf};

pub const MAX_LOG_BYTES: u64 = 1024 * 1024;

pub fn log_path() -> PathBuf {
    std::env::var_os("PLINTH_LOG").map(PathBuf::from).unwrap_or_else(|| crate::state::local_dir().join("plinth.log"))
}

/// Replace the current user's name wherever it appears (case-insensitive), and any `C:\Users\<name>` prefix, so a pasted report is
/// not tied to a person.
pub fn scrub(text: &str, user: &str) -> String {
    let mut out = text.to_string();
    if !user.is_empty() && user.len() >= 2 {
        let lower = out.to_ascii_lowercase();
        let needle = user.to_ascii_lowercase();
        let mut res = String::with_capacity(out.len());
        let mut i = 0;
        while let Some(pos) = lower[i..].find(&needle) {
            res.push_str(&out[i..i + pos]);
            res.push_str("<user>");
            i += pos + needle.len();
        }
        res.push_str(&out[i..]);
        out = res;
    }
    out
}

/// The last `lines` lines of the log, scrubbed. Missing log gives an empty string.
pub fn log_tail(path: &Path, lines: usize, user: &str) -> String {
    let Ok(bytes) = std::fs::read(path) else { return String::new() };
    let start = bytes.len().saturating_sub(48 * 1024);
    let text = String::from_utf8_lossy(&bytes[start..]).to_string();
    let all: Vec<&str> = text.lines().collect();
    let tail = all[all.len().saturating_sub(lines)..].join("\n");
    scrub(&tail, user)
}

/// Keep the log from growing without bound: past the limit it becomes `.old` and a fresh file starts.
pub fn rotate(path: &Path) {
    if std::fs::metadata(path).map_or(false, |m| m.len() > MAX_LOG_BYTES) {
        let _ = std::fs::remove_file(path.with_extension("old"));
        let _ = std::fs::rename(path, path.with_extension("old"));
    }
}

pub struct Facts {
    pub version: String,
    pub os_build: String,
    pub monitors: Vec<(u32, u32, u32, bool)>,
    pub widgets: usize,
    pub copies: usize,
    pub applied: bool,
    pub safe: bool,
    pub library_items: u64,
    pub library_problems: u64,
    pub wallpapers_found: usize,
    pub moods_on: bool,
    pub rules: usize,
    pub autostart: bool,
    pub layers: usize,
}

pub fn report(f: &Facts, log_tail: &str, user: &str) -> String {
    let mons: Vec<String> = f.monitors.iter().enumerate().map(|(i, (w, h, s, p))| format!("  {}: {}x{} at {}%{}", i + 1, w, h, s, if *p { " (primary)" } else { "" })).collect();
    scrub(
        &format!(
            "Plinth diagnostics\nversion {}\nWindows build {}\napplied {} | safe mode {} | layers {}\nmonitors {}:\n{}\nwidgets {} (+{} copies)\nwallpapers {} | library pictures {} ({} with problems)\nmoods {} ({} rules) | start with Windows {}\n\n--- recent log ---\n{}\n",
            f.version, f.os_build, f.applied, f.safe, f.layers, f.monitors.len(), mons.join("\n"), f.widgets, f.copies, f.wallpapers_found, f.library_items, f.library_problems, if f.moods_on { "on" } else { "off" }, f.rules, f.autostart, log_tail
        ),
        user,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_names_are_removed_everywhere_case_insensitively() {
        let t = scrub(r"C:\Users\Miche\AppData\x and C:\USERS\MICHE\y, miche!", "miche");
        assert!(!t.to_ascii_lowercase().contains("miche"), "{t}");
        assert!(t.contains("<user>"));
        assert_eq!(scrub("nothing here", "miche"), "nothing here");
        assert_eq!(scrub("keep", ""), "keep");
        assert_eq!(scrub("a", "a"), "a", "one-letter names are not scrubbed (they would wreck the text)");
    }

    #[test]
    fn the_report_lists_counts_only() {
        let f = Facts { version: "1.2.3".into(), os_build: "26300".into(), monitors: vec![(1920, 1080, 100, true), (1280, 400, 100, false)], widgets: 5, copies: 1, applied: true, safe: false, library_items: 40, library_problems: 1, wallpapers_found: 3, moods_on: true, rules: 2, autostart: false, layers: 2 };
        let r = report(&f, r"opened C:\Users\Miche\Pictures\secret.png", "miche");
        assert!(r.contains("version 1.2.3") && r.contains("1920x1080 at 100% (primary)") && r.contains("library pictures 40 (1 with problems)"));
        assert!(!r.to_ascii_lowercase().contains("miche"), "{r}");
        assert!(!r.contains("DISPLAY#"));
    }

    #[test]
    fn the_log_tail_is_bounded_and_rotation_works() {
        let d = std::env::temp_dir().join(format!("plinth-diag-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("plinth.log");
        let big: String = (0..80000).map(|i| format!("line {i} for miche\n")).collect();
        std::fs::write(&f, &big).unwrap();
        let tail = log_tail(&f, 5, "miche");
        assert_eq!(tail.lines().count(), 5);
        assert!(tail.contains("line 79999") && !tail.contains("miche"));
        assert!(log_tail(&d.join("none.log"), 5, "x").is_empty());
        rotate(&f);
        assert!(!f.exists() && f.with_extension("old").exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
