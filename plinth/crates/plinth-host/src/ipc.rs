//! Who may talk to the app's IPC, and where Plinth's own web views may navigate.
//!
//! Widgets run in iframes of the layer page, and every frame of a web view can call `window.ipc.postMessage`
//! (OBSERVED in the 2026-11-06 once-over: a third-party widget frame has both `window.ipc` and `window.chrome.webview`).
//! The layer page therefore proves who it is with a random per-run secret that widget frames cannot read
//! (they live in other origins), and the host only believes `wid` from a message that carries it.

use serde_json::Value;
use std::hash::{BuildHasher, Hasher};

/// Messages longer than this are dropped before parsing (a widget message is a small JSON call).
pub const MAX_LAYER_MESSAGE: usize = 128 * 1024;

/// A fresh 256-bit secret (64 hex digits). `RandomState` is seeded by the operating system.
pub fn new_nonce() -> String {
    (0..4u32)
        .map(|i| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u32(i);
            format!("{:016x}", h.finish())
        })
        .collect()
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Accepts `{"n": <nonce>, "wid": <id>, "msg": <string>}` only when `n` equals `nonce`; returns `(wid, msg)`.
/// Anything else (no nonce, wrong nonce, wrong types, oversize, not JSON) is `None`.
pub fn parse_layer_message(body: &str, nonce: &str) -> Option<(String, String)> {
    if nonce.is_empty() || body.len() > MAX_LAYER_MESSAGE {
        return None;
    }
    let v: Value = serde_json::from_str(body).ok()?;
    if !same(v.get("n")?.as_str()?, nonce) {
        return None;
    }
    Some((v.get("wid")?.as_str()?.to_string(), v.get("msg")?.as_str()?.to_string()))
}

/// Main-frame navigation policy for Plinth's own web views (hub, layer, edit overlay): they only ever show
/// HTML the app hands them. Anything else (web pages, files, `javascript:`, blobs) is refused.
pub fn allow_navigation(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u == "about:blank" || u == "about:srcdoc" || u.starts_with("data:text/html")
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: &str = "0123456789abcdef";

    fn msg(n: &str, wid: &str) -> String {
        serde_json::json!({ "n": n, "wid": wid, "msg": "{\"id\":1}" }).to_string()
    }

    #[test]
    fn the_layer_page_with_the_secret_is_accepted() {
        assert_eq!(parse_layer_message(&msg(N, "com.plinth.notes"), N), Some(("com.plinth.notes".into(), "{\"id\":1}".into())));
    }

    #[test]
    fn a_widget_frame_without_the_secret_cannot_name_another_widget() {
        let forged = serde_json::json!({ "wid": "com.plinth.notes", "msg": "{}" }).to_string();
        assert_eq!(parse_layer_message(&forged, N), None);
    }

    #[test]
    fn a_wrong_or_partial_secret_is_refused() {
        assert_eq!(parse_layer_message(&msg("0123456789abcdee", "a"), N), None);
        assert_eq!(parse_layer_message(&msg("0123456789abcde", "a"), N), None);
        assert_eq!(parse_layer_message(&msg("", "a"), N), None);
        assert_eq!(parse_layer_message(&msg(&format!("{N}0"), "a"), N), None);
    }

    #[test]
    fn an_empty_session_secret_accepts_nothing() {
        assert_eq!(parse_layer_message(&msg("", "a"), ""), None);
    }

    #[test]
    fn wrong_types_bad_json_and_oversize_are_refused() {
        assert_eq!(parse_layer_message("not json", N), None);
        assert_eq!(parse_layer_message(&serde_json::json!({ "n": N, "wid": 5, "msg": "x" }).to_string(), N), None);
        assert_eq!(parse_layer_message(&serde_json::json!({ "n": N, "wid": "a", "msg": {"x": 1} }).to_string(), N), None);
        assert_eq!(parse_layer_message(&serde_json::json!({ "n": 7, "wid": "a", "msg": "x" }).to_string(), N), None);
        let big = serde_json::json!({ "n": N, "wid": "a", "msg": "x".repeat(MAX_LAYER_MESSAGE) }).to_string();
        assert_eq!(parse_layer_message(&big, N), None);
    }

    #[test]
    fn secrets_are_long_hex_and_differ_between_calls() {
        let (a, b) = (new_nonce(), new_nonce());
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn only_app_generated_pages_may_be_navigated_to() {
        for ok in ["about:blank", "about:srcdoc", "data:text/html;charset=utf-8,%3Chtml%3E", "DATA:text/html,<p>"] {
            assert!(allow_navigation(ok), "{ok}");
        }
        for bad in ["http://evil.example/", "https://pw0.localhost/", "file:///C:/Windows/win.ini", "javascript:alert(1)", "blob:http://x/1", "data:application/pdf;base64,AA", "ftp://x", ""] {
            assert!(!allow_navigation(bad), "{bad}");
        }
    }
}
