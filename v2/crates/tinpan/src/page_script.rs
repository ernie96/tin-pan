//! Running a script on a hidden web page without wxWidgets' "Error running JavaScript" box.
//!
//! wxWidgets shows that box whenever a script fails, and on Windows a script fails in two ordinary cases: the
//! page threw, or WebView2 hasn't finished starting (it starts in the background, and on a slower computer that
//! takes seconds). Neither is worth a box nobody asked for, so scripts wait for WebView2 here and run inside a
//! try/catch that hands an error back as text starting "ERR:".

use wxdragon::widgets::WebView;

/// Whether the web view can run scripts yet. On Windows the native backend is null until WebView2 is ready.
pub fn ready(view: &WebView) -> bool {
    !view.get_native_backend().is_null()
}

/// Runs `js` (an expression, or statements whose last value is the result) and gives back its result as plain
/// text, an "ERR:" text if it threw, or None if the page can't run scripts yet.
pub fn run(view: &WebView, js: &str) -> Option<String> {
    if !ready(view) {
        return None;
    }
    let out = view.run_script(&guarded(js))?;
    Some(out.trim().trim_matches('"').to_string())
}

/// `js` wrapped so it can't throw: an exception comes back as "ERR:" and its text.
fn guarded(js: &str) -> String {
    let quoted = serde_json::to_string(js).unwrap_or_else(|_| "''".to_string());
    format!("(() => {{ try {{ const r = (0, eval)({quoted}); return r === undefined ? '' : String(r); }} catch (e) {{ return 'ERR:' + String(e); }} }})()")
}

#[cfg(test)]
mod tests {
    use super::guarded;

    #[test]
    fn quotes_and_newlines_stay_inside_the_string() {
        let js = guarded("const a = \"x\";\n'it''s' + a");
        assert!(js.starts_with("(() => { try { const r = (0, eval)(\"const a = \\\"x\\\";\\n'it''s' + a\");"));
        assert!(js.contains("catch (e) { return 'ERR:' + String(e); }"));
    }
}
