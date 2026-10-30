//! Widget packages: validated manifest plus an in-memory file map. Nothing is served from disk at runtime,
//! so a widget cannot reach files outside its package even through a path bug.

use plinth_core::{Kind, Manifest, ManifestError};
use std::collections::HashMap;
use std::path::Path;

pub const MAX_FILES: usize = 200;
pub const MAX_TOTAL_BYTES: usize = 5 * 1024 * 1024;
pub const ENTRY: &str = "index.html";

#[derive(Debug)]
pub enum PackageError {
    Manifest(ManifestError),
    Io(String),
    NotAWidget,
    MissingEntry,
    TooManyFiles,
    TooLarge,
    BadPath(String),
    Symlink(String),
}

impl From<ManifestError> for PackageError {
    fn from(e: ManifestError) -> Self {
        PackageError::Manifest(e)
    }
}

/// Normalizes a request or package path. Returns None for anything that could escape the package.
pub fn normalize_path(p: &str) -> Option<String> {
    let p = p.split(['?', '#']).next().unwrap_or("");
    if p.contains('\\') || p.contains('\0') || p.contains(':') {
        return None;
    }
    let trimmed = p.trim_start_matches('/');
    if trimmed.is_empty() {
        return Some(ENTRY.to_string());
    }
    let mut parts = Vec::new();
    for seg in trimmed.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." || seg.starts_with('.') {
            return None;
        }
        parts.push(seg);
    }
    Some(parts.join("/"))
}

pub struct Package {
    pub manifest: Manifest,
    files: HashMap<String, Vec<u8>>,
}

impl Package {
    pub fn from_files(manifest_json: &str, files: Vec<(String, Vec<u8>)>) -> Result<Package, PackageError> {
        let manifest = Manifest::parse(manifest_json)?;
        if manifest.kind != Kind::Widget {
            return Err(PackageError::NotAWidget);
        }
        if files.len() > MAX_FILES {
            return Err(PackageError::TooManyFiles);
        }
        let mut map = HashMap::new();
        let mut total = 0usize;
        for (path, bytes) in files {
            let norm = normalize_path(&path).ok_or_else(|| PackageError::BadPath(path.clone()))?;
            total += bytes.len();
            if total > MAX_TOTAL_BYTES {
                return Err(PackageError::TooLarge);
            }
            map.insert(norm, bytes);
        }
        if !map.contains_key(ENTRY) {
            return Err(PackageError::MissingEntry);
        }
        Ok(Package { manifest, files: map })
    }

    /// Reads `manifest.json` and every other file under `dir` (recursively, no symlinks).
    pub fn from_dir(dir: &Path) -> Result<Package, PackageError> {
        let manifest_json = std::fs::read_to_string(dir.join("manifest.json")).map_err(|e| PackageError::Io(e.to_string()))?;
        let mut files = Vec::new();
        fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), PackageError> {
            for entry in std::fs::read_dir(dir).map_err(|e| PackageError::Io(e.to_string()))? {
                let entry = entry.map_err(|e| PackageError::Io(e.to_string()))?;
                let ft = entry.file_type().map_err(|e| PackageError::Io(e.to_string()))?;
                let path = entry.path();
                if ft.is_symlink() {
                    return Err(PackageError::Symlink(path.display().to_string()));
                }
                if ft.is_dir() {
                    walk(base, &path, out)?;
                } else {
                    let rel = path.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                    if rel == "manifest.json" {
                        continue;
                    }
                    out.push((rel, std::fs::read(&path).map_err(|e| PackageError::Io(e.to_string()))?));
                }
            }
            Ok(())
        }
        walk(dir, dir, &mut files)?;
        Package::from_files(&manifest_json, files)
    }

    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(&normalize_path(path)?).map(|v| v.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: &str = r#"{"format":1,"id":"com.example.clock","name":"Clock","version":"1.0.0","type":"widget"}"#;

    #[test]
    fn normalizes_and_rejects_escapes() {
        assert_eq!(normalize_path("/"), Some("index.html".into()));
        assert_eq!(normalize_path("/a/b.js?x=1"), Some("a/b.js".into()));
        for bad in ["/../x", "/a/../b", "/a//b", "/.hidden", "/a\\b", "/C:/x", "/a\0b", "/./a"] {
            assert_eq!(normalize_path(bad), None, "{bad}");
        }
    }

    #[test]
    fn builds_and_enforces_limits() {
        let ok = Package::from_files(M, vec![("index.html".into(), b"hi".to_vec()), ("js/app.js".into(), b"1".to_vec())]).unwrap();
        assert_eq!(ok.get("/js/app.js"), Some(&b"1"[..]));
        assert_eq!(ok.get("/nope"), None);
        assert!(matches!(Package::from_files(M, vec![("a.js".into(), vec![])]), Err(PackageError::MissingEntry)));
        assert!(matches!(Package::from_files(M, vec![("../x".into(), vec![]), ("index.html".into(), vec![])]), Err(PackageError::BadPath(_))));
        let big = vec![0u8; MAX_TOTAL_BYTES + 1];
        assert!(matches!(Package::from_files(M, vec![("index.html".into(), big)]), Err(PackageError::TooLarge)));
        let many: Vec<_> = (0..=MAX_FILES).map(|i| (format!("f{i}.js"), vec![])).collect();
        assert!(matches!(Package::from_files(M, many), Err(PackageError::TooManyFiles)));
        let scene = M.replace("widget", "scene");
        assert!(matches!(Package::from_files(&scene, vec![("index.html".into(), vec![])]), Err(PackageError::NotAWidget)));
    }

    #[test]
    fn manifest_json_is_not_servable() {
        let p = Package::from_files(M, vec![("index.html".into(), vec![])]).unwrap();
        assert_eq!(p.get("/manifest.json"), None);
    }
}
