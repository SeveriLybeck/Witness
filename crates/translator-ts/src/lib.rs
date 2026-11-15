//! TypeScript front end (D24). The TypeScript compiler API (pinned 5.9.3 in `js/package.json`) runs under
//! `node js/lower.js`; this crate installs it on first use and returns its JSON.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The crate's `js/` folder.
pub fn js_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("js")
}

/// `npm ci` in `js/` when `node_modules/typescript` is missing.
pub fn ensure_js() -> Result<(), String> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = js_dir();
    if dir.join("node_modules/typescript/package.json").exists() {
        return Ok(());
    }
    let st = Command::new("npm")
        .args(["ci", "--no-audit", "--no-fund", "--silent"])
        .current_dir(&dir)
        .status()
        .map_err(|e| format!("npm: {e}"))?;
    if st.success() { Ok(()) } else { Err("npm ci failed in js/".into()) }
}

fn run(args: &[&str]) -> Result<Value, String> {
    ensure_js()?;
    let out = Command::new("node")
        .arg(js_dir().join("lower.js"))
        .args(args)
        .output()
        .map_err(|e| format!("node: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

/// Lower function `function` of `file` to a model (`contracts/messages/model.schema.json`).
/// `unit` overrides the unit id (default `<function>#<line>`); old and new should share one.
pub fn lower(file: &Path, function: &str, version: &str, unit: Option<&str>) -> Result<Value, String> {
    let f = file.to_str().ok_or("path")?;
    let mut a = vec!["lower", f, function, version];
    if let Some(u) = unit {
        a.push(u);
    }
    run(&a)
}

/// React components of `file` and what each may re-render when: `{components:[{component, file, memo,
/// heading, causes:[{cause, name, line, label}]}]}`; cause is `prop`, `state`, `context`, `store` or `parent`.
pub fn render_causes(file: &Path) -> Result<Value, String> {
    run(&["render", file.to_str().ok_or("path")?])
}
