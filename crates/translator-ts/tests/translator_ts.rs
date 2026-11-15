use differ::model::Model;
use differ::{diff_models, Options};
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn sample(n: &str) -> PathBuf {
    here().join("tests/samples").join(n)
}

#[test]
fn fallbacks_line_1_is_contract_hash() {
    let root = here().join("../..");
    let out = Command::new("git").args(["log", "-1", "--format=%H", "--", "contracts/"]).current_dir(&root).output().unwrap();
    let hash = String::from_utf8(out.stdout).unwrap();
    let fb = std::fs::read_to_string(here().join("FALLBACKS.md")).unwrap();
    assert_eq!(fb.lines().next().unwrap(), hash.trim());
}

#[test]
fn typescript_pinned_to_5_9() {
    let p: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(here().join("js/package.json")).unwrap()).unwrap();
    assert!(p["dependencies"]["typescript"].as_str().unwrap().starts_with("5.9."));
    translator_ts::ensure_js().unwrap();
    let lock = std::fs::read_to_string(here().join("js/node_modules/typescript/package.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&lock).unwrap();
    assert!(v["version"].as_str().unwrap().starts_with("5.9."));
}

fn model(file: &str, version: &str) -> Model {
    let v = translator_ts::lower(&sample(file), "discount", version, Some("discount#1")).unwrap();
    Model::parse(&v).unwrap()
}

#[test]
fn lowers_pure_function() {
    let v = translator_ts::lower(&sample("price.old.ts"), "discount", "old", None).unwrap();
    assert_eq!(v["unit"], "discount#1");
    assert_eq!(v["params"][0]["sort"], "Int");
    assert_eq!(v["params"][1]["sort"], "Bool");
    assert!(v["lines"]["notModelled"].as_array().unwrap().is_empty());
    assert!(v["opaque"].as_array().unwrap().iter().any(|o| o["symbol"] == "op:+(Int,Int)"));
}

#[test]
fn differ_finds_planted_difference() {
    let opts = Options { file: Some("price.ts".into()), first_row: 1, no_dependency: true, ..Default::default() };
    let same = diff_models(&model("price.old.ts", "old"), &model("price.old.ts", "new"), &opts).unwrap();
    assert_eq!(same.differing().count(), 0);
    let r = diff_models(&model("price.old.ts", "old"), &model("price.new.ts", "new"), &opts).unwrap();
    let kinds: Vec<_> = r.differing().map(|x| x.kind.unwrap()).collect();
    assert_eq!(kinds, ["crash-fixed"], "{kinds:?}");
}

#[test]
fn unsupported_statement_is_not_modelled() {
    let dir = std::env::temp_dir().join(format!("tts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("loop.ts");
    std::fs::write(&f, "export function f(n: number): number {\n  for (let i = 0; i < n; i++) {}\n  return n;\n}\n").unwrap();
    let v = translator_ts::lower(Path::new(&f), "f", "new", None).unwrap();
    assert_eq!(v["lines"]["notModelled"][0]["location"]["line"], 2);
    Model::parse(&v).unwrap();
}

#[test]
fn react_render_causes() {
    let v = translator_ts::render_causes(&sample("Counter.tsx")).unwrap();
    let c = &v["components"];
    let labels = |i: usize| -> Vec<String> { c[i]["causes"].as_array().unwrap().iter().map(|x| x["label"].as_str().unwrap().to_string()).collect() };
    assert_eq!(c[0]["component"], "Counter");
    assert_eq!(c[0]["heading"], "may re-render when");
    assert_eq!(labels(0), ["prop label changes", "prop step changes", "state count changes", "context Theme changes", "parent re-renders"]);
    assert_eq!(c[1]["component"], "Badge");
    assert_eq!(labels(1), ["prop text changes"]);
}
