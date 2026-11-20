// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Splash screen HTML, served as a `data:` URL.
//!
//! Previously the splash HTML was written to a fixed shared temp path
//! (`echophrase_splash.html`) and navigated to via a `file://` URL. That had
//! two failure modes on Windows: AV scanning could lock/quarantine the file
//! before navigation completed, and a second process (updater relaunch,
//! accidental double-launch) could delete the same fixed-name file out from
//! under the first process's in-flight navigation (the single-instance
//! plugin doesn't guard this pre-handshake startup window). Both showed up
//! as WebView2's "File not found ERR_FILE_NOT_FOUND" page.
//!
//! Serving the HTML as a base64 `data:` URL removes the filesystem
//! dependency entirely - there is nothing to lock, quarantine, or race.
//!
//! # KNOWN UPSTREAM BUG (tauri 2.11.5): no `#` in the splash HTML
//!
//! Tauri mangles `data:` URLs whenever the app declares a CSP (ours is in
//! `tauri.conf.json5` under `app.security.csp`). In
//! `tauri/src/manager/webview.rs` (~line 484-497) it decodes the base64,
//! injects the CSP into the parsed document, and then re-serializes with:
//!
//! ```ignore
//! url.set_path(&format!("{},{}", mime::TEXT_HTML, document.html()));
//! ```
//!
//! The HTML goes back in RAW - the base64 is discarded and the markup is not
//! percent-encoded. A `#` in the markup therefore starts the URL *fragment*,
//! and the webview receives a document truncated at that byte.
//!
//! This bit us for real: `background:#09090b` sat at byte 86, so WebKitGTK got
//! 86 bytes ending mid-`<style>` and painted a blank white window. Measured on
//! a dev launch before the fix:
//!
//! ```text
//! len=2283 has_fragment=true first_hash=Some(86) frag="09090b;display:flex;..."
//! ```
//!
//! and after:
//!
//! ```text
//! len=1493 has_fragment=false first_hash=None frag=None
//! ```
//!
//! It is not Linux-specific in principle - the URL is malformed before any
//! platform sees it - though Linux is where we caught it.
//!
//! ## Working around it
//!
//! Keep `#` out of `SPLASH_HTML`: use `rgb()` instead of hex colors, and CSS
//! `linear-gradient` instead of an SVG `fill="url(#id)"`. `splash_html_has_no_hash`
//! enforces this, so a stray hex color fails the test rather than silently
//! blanking the splash.
//!
//! ## When Tauri fixes it
//!
//! Filed upstream 2026-09-03: <https://github.com/tauri-apps/tauri/issues/15966>.
//! If a later Tauri percent-encodes that path (or keeps the base64), this
//! constraint can be relaxed and the SVG logo restored. Re-check
//! `manager/webview.rs` on upgrade; until then assume the constraint still
//! holds, since nothing in Tauri warns you when it breaks - the splash just
//! goes white.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};

/// Raw splash screen HTML (a few KB - logo + CSS spinner, no external assets).
///
/// MUST NOT contain a `#` character anywhere - a `#` here silently blanks the
/// splash. See the KNOWN UPSTREAM BUG section in this module's docs for why,
/// and `splash_html_has_no_hash` for the guard. Hence `rgb()` colors and CSS
/// `linear-gradient` bars instead of an SVG `fill="url(#g)"`.
const SPLASH_HTML: &str = r##"<!DOCTYPE html><html><head><style>*{margin:0;padding:0}body{background:rgb(9,9,11);display:flex;flex-direction:column;align-items:center;justify-content:center;height:100vh;overflow:hidden;gap:24px}.logo{display:flex;align-items:center;gap:4px;height:48px;opacity:0.9}.b{width:4px;border-radius:2px;background:linear-gradient(to top,rgb(6,182,212),rgb(139,92,246),rgb(236,72,153))}.s{width:32px;height:32px;border:3px solid rgba(139,92,246,0.15);border-top-color:rgba(139,92,246,0.7);border-radius:50%;animation:s .8s linear infinite}@keyframes s{to{transform:rotate(360deg)}}</style></head><body><div class="logo"><div class="b" style="height:10px"></div><div class="b" style="height:18px"></div><div class="b" style="height:28px"></div><div class="b" style="height:22px"></div><div class="b" style="height:14px"></div></div><div class="s"></div></body></html>"##;

/// Build the splash screen as a `data:text/html;base64,...` URL.
///
/// Pure function (no filesystem/IO) so it can be unit tested directly.
/// Returns an error only if the resulting string somehow isn't a valid URL,
/// which should not happen for a well-formed base64 data URL.
pub fn splash_data_url() -> Result<tauri::Url, url::ParseError> {
    let encoded = BASE64.encode(SPLASH_HTML.as_bytes());
    tauri::Url::parse(&format!("data:text/html;base64,{encoded}"))
}

/// Build and show the "splash" webview window.
///
/// Shared by real startup (`lib.rs`) and the Debug panel's Splash button, so
/// what you see when testing is exactly what users get - a separate builder for
/// the debug path would drift from the real one and hide the bug you're chasing.
pub fn create_splash_window<R: tauri::Runtime, M: tauri::Manager<R>>(
    manager: &M,
) -> Result<tauri::WebviewWindow<R>, String> {
    let splash_url = splash_data_url().map_err(|e| format!("failed to build splash URL: {e}"))?;
    tauri::WebviewWindowBuilder::new(manager, "splash", tauri::WebviewUrl::External(splash_url))
        .title("Echophrase")
        .inner_size(475.0, 400.0)
        .center()
        .decorations(true)
        .resizable(false)
        .visible(true)
        .build()
        .map_err(|e| format!("failed to create splash window: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards the bug that made the splash render as a blank white window.
    ///
    /// Tauri re-serializes CSP-injected `data:` URLs with the HTML written raw
    /// into the URL path, so any `#` in the markup starts the fragment and
    /// truncates the document from that byte onward. Reintroducing a hex color
    /// or an SVG `url(#id)` reference would silently blank the splash again.
    #[test]
    fn splash_html_has_no_hash() {
        assert!(
            !SPLASH_HTML.contains('#'),
            "splash HTML must not contain '#': it becomes a URL fragment and truncates the document"
        );
        let url = splash_data_url().expect("splash data URL should parse");
        assert!(url.fragment().is_none(), "splash URL must have no fragment");
    }

    #[test]
    fn splash_data_url_parses() {
        let url = splash_data_url().expect("splash data URL should parse");
        assert_eq!(url.scheme(), "data");
    }

    #[test]
    fn splash_data_url_stays_well_under_webview_limits() {
        let url = splash_data_url().expect("splash data URL should parse");
        // WebView2 handles data: URLs fine at this size; the raw HTML is a
        // few KB, so the base64-encoded URL should stay well under ~1.5MB.
        assert!(
            url.as_str().len() < 1_500_000,
            "splash data URL unexpectedly large: {} bytes",
            url.as_str().len()
        );
    }

    #[test]
    fn splash_data_url_contains_encoded_html() {
        let url = splash_data_url().expect("splash data URL should parse");
        assert!(url.as_str().starts_with("data:text/html;base64,"));
    }
}
