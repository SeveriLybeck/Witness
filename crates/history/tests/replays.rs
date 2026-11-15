use serde_json::Value;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn sessions(c: &str) -> PathBuf {
    root().join(format!("replays/{c}/gateway/sessions"))
}
fn bundle(c: &str) -> PathBuf {
    root().join(format!("replays/{c}/bundle"))
}
fn json(p: PathBuf) -> Value {
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap()
}

#[test]
fn contract_hash_on_line_one() {
    let out = std::process::Command::new("git")
        .current_dir(root())
        .args(["log", "-1", "--format=%H", "--", "contracts/"])
        .output()
        .unwrap();
    let hash = String::from_utf8(out.stdout).unwrap();
    let f = std::fs::read_to_string(root().join("crates/history/FALLBACKS.md")).unwrap();
    assert_eq!(f.lines().next().unwrap(), hash.trim());
}

#[test]
fn two_recorded_reviews() {
    // 2283: b2 is not-declared; its differences d1-d3 are in TypeCollector.cs.
    let ca = "src/MessagePack.SourceGenerator/CodeAnalysis".to_string();
    let u2283 = history::undeclared(&bundle("2283"));
    let ids: Vec<(String, String)> = ["d1", "d2", "d3"].iter().map(|d| (ca.clone(), d.to_string())).collect();
    assert_eq!(u2283, ids);
    // 2297: one difference, in not-declared b2.
    let u2297 = history::undeclared(&bundle("2297"));
    assert_eq!(u2297.len(), 1);
    let c = history::count(
        &[sessions("2283"), sessions("2297")],
        &[bundle("2283"), bundle("2297")],
        None,
    );
    let total: u64 = c.values().map(|v| v.undeclared).sum();
    assert_eq!(total as usize, u2283.len() + u2297.len());
    let k = history::bundle_key(&bundle("2283")).unwrap();
    assert!(c["src/MessagePack.SourceGenerator/CodeAnalysis"].reviews.contains(&k));
    // The bundle under review is not its own history.
    assert!(history::count(&[sessions("2283")], &[bundle("2283")], Some(&k)).is_empty());
}

#[test]
fn only_signed_reviews_count() {
    let mut r = json(root().join("contracts/fixtures/sessions/32258fdbee4bcd653c79f51b4376b5103c6f4d27-9df83c38409c8bfad0517aebffe48211eda4a5b4/reviewer-a/review.json"));
    r["event"] = "APPROVE".into();
    r["result"] = serde_json::json!({"status": 200});
    assert!(history::signed(&r));
    r["result"]["status"] = 422.into();
    assert!(!history::signed(&r));
    r["result"]["status"] = 200.into();
    r["event"] = "REQUEST_CHANGES".into();
    assert!(!history::signed(&r));
}

#[test]
fn annotated_files_validate() {
    let schemas = contracts::Schemas::contract().unwrap();
    let name = if schemas.has("files") { "files" } else { "files.schema.json" };
    let c = history::count(
        &[sessions("2283"), sessions("2234"), sessions("2129"), sessions("2297")],
        &[bundle("2283"), bundle("2234"), bundle("2129"), bundle("2297")],
        None,
    );
    assert!(!c.is_empty());
    let mut inputs: Vec<PathBuf> =
        ["2283", "2297", "2234", "2129"].iter().map(|b| bundle(b).join("files.json")).collect();
    inputs.push(root().join("contracts/fixtures/2283/files.json"));
    for p in inputs {
        let out = history::annotate(&json(p.clone()), &c);
        schemas.validate(name, &out, "").unwrap_or_else(|e| panic!("{}: {e:?}", p.display()));
        assert!(out["files"].as_array().unwrap().iter().all(|f| f["history"]["undeclared"].is_u64()));
    }
}
