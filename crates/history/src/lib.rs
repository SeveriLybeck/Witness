//! Folder history: undeclared differences per folder from earlier signed reviews (D44).
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Count per folder: undeclared differences and the signed reviews they came from.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FolderCount {
    pub undeclared: u64,
    pub reviews: BTreeSet<String>,
}

pub type Counts = BTreeMap<String, FolderCount>;

fn read(p: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(p).ok()?).ok()
}

/// Bundle key `<old>-<new>` from a bundle directory's `bundle.json`.
pub fn bundle_key(dir: &Path) -> Option<String> {
    let b = read(&dir.join("bundle.json"))?;
    Some(format!("{}-{}", b["oldCommit"].as_str()?, b["newCommit"].as_str()?))
}

/// Folder of a repository path: everything before the last `/`, or "" at the root.
pub fn folder(file: &str) -> String {
    file.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default()
}

/// A review is signed when it was an APPROVE accepted by the host (2xx result).
pub fn signed(review: &Value) -> bool {
    review["event"] == "APPROVE"
        && review["result"]["status"].as_u64().map_or(false, |s| (200..300).contains(&s))
}

/// Undeclared differences of one bundle (block not declared or not linked, D27) as (folder, id).
pub fn undeclared(bundle: &Path) -> Vec<(String, String)> {
    let links = read(&bundle.join("links.json")).unwrap_or(Value::Null);
    let declared: BTreeSet<&str> = links["links"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|l| l["state"] == "declared")
        .filter_map(|l| l["block"].as_str())
        .collect();
    let diffs = read(&bundle.join("differences.json")).unwrap_or(Value::Null);
    diffs["differences"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| !d["block"].as_str().map_or(false, |b| declared.contains(b)))
        .filter_map(|d| Some((folder(d["location"]["file"].as_str()?), d["id"].as_str()?.to_string())))
        .collect()
}

/// Review files under session roots: `<root>/<key>/<reviewer>/review.json`.
pub fn reviews(roots: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let mut out = vec![];
    for root in roots {
        let Ok(keys) = fs::read_dir(root) else { continue };
        for k in keys.flatten() {
            let Ok(revs) = fs::read_dir(k.path()) else { continue };
            for r in revs.flatten() {
                let p = r.path().join("review.json");
                if p.is_file() {
                    out.push((k.file_name().to_string_lossy().into_owned(), p));
                }
            }
        }
    }
    out.sort();
    out
}

/// Counts from signed reviews whose bundle is known, skipping the bundle under review.
/// A bundle signed by several reviewers counts once.
pub fn count(session_roots: &[PathBuf], bundles: &[PathBuf], exclude: Option<&str>) -> Counts {
    let by_key: BTreeMap<String, &PathBuf> =
        bundles.iter().filter_map(|b| Some((bundle_key(b)?, b))).collect();
    let mut counts = Counts::new();
    let mut seen = BTreeSet::new();
    for (key, path) in reviews(session_roots) {
        if Some(key.as_str()) == exclude || seen.contains(&key) {
            continue;
        }
        let Some(review) = read(&path) else { continue };
        if !signed(&review) {
            continue;
        }
        let Some(dir) = by_key.get(&key) else { continue };
        seen.insert(key.clone());
        for (f, _) in undeclared(dir) {
            let c = counts.entry(f).or_default();
            c.undeclared += 1;
            c.reviews.insert(key.clone());
        }
    }
    counts
}

/// `files.json` with `history: {folder, undeclared, reviews}` on each file (an extra field;
/// the record still validates against `files.schema.json`).
pub fn annotate(files: &Value, counts: &Counts) -> Value {
    let mut out = files.clone();
    if let Some(list) = out["files"].as_array_mut() {
        for f in list {
            let dir = folder(f["file"].as_str().unwrap_or(""));
            let c = counts.get(&dir).cloned().unwrap_or_default();
            f["history"] = json!({"folder": dir, "undeclared": c.undeclared, "reviews": c.reviews.len()});
        }
    }
    out
}
