//! Wraps a raw Win32 HWND so wry can create a WebView2 child inside it.
//! Page modes (`--mode blank|iframe|inline`, `--widgets N`, default 5) let us compare the RAM/CPU cost of
//! the sandboxed-iframe widget design against a single inline page and a blank page.
use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle};
use std::num::NonZeroIsize;

pub struct Host(pub isize);

impl HasWindowHandle for Host {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let h = Win32WindowHandle::new(NonZeroIsize::new(self.0).ok_or(HandleError::Unavailable)?);
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(h)) })
    }
}

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

/// Each widget: a label plus a clock that updates once per second (realistic light idle activity).
const WIDGET_BODY: &str = r#"<div style="font:12px sans-serif;color:#000;background:#fff;padding:4px;width:84px;height:36px;overflow:hidden"><b>W__N__</b><div id=t></div></div><script>const t=document.getElementById('t');setInterval(()=>{t.textContent=new Date().toLocaleTimeString()},1000)</script>"#;

fn html() -> String {
    let mode = arg("--mode").unwrap_or_else(|| "blank".into());
    let n: usize = arg("--widgets").and_then(|s| s.parse().ok()).unwrap_or(5);
    let mut body = String::new();
    match mode.as_str() {
        "iframe" => {
            for i in 0..n {
                // sandbox without allow-same-origin => opaque origin per widget, like the planned design.
                let doc = WIDGET_BODY.replace("__N__", &i.to_string()).replace('"', "&quot;");
                body.push_str(&format!(
                    "<iframe sandbox=\"allow-scripts\" style=\"border:0;width:92px;height:44px\" srcdoc=\"{doc}\"></iframe>"
                ));
            }
        }
        "inline" => {
            for i in 0..n {
                body.push_str(&format!(
                    "<div style=\"display:inline-block\">{}</div>",
                    WIDGET_BODY.replace("__N__", &i.to_string())
                ));
            }
        }
        _ => {}
    }
    // Chips sit in the top strip; the rest stays magenta so the Win+D pixel test still works.
    format!("<html><body style=\"margin:0;background:#ff00ff\"><div style=\"height:50px;white-space:nowrap\">{body}</div></body></html>")
}

pub fn make_webview(hwnd: isize, w: u32, h: u32) -> wry::WebView {
    let host = Host(hwnd);
    wry::WebViewBuilder::new()
        .with_html(html())
        .with_bounds(wry::Rect {
            position: wry::dpi::PhysicalPosition::new(0, 0).into(),
            size: wry::dpi::PhysicalSize::new(w, h).into(),
        })
        .build_as_child(&host)
        .expect("WebView2 creation failed")
}
