use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
use translator_rust::{lower, Options};

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn model(version: &str) -> Value {
    let src = std::fs::read_to_string(here().join(format!("tests/data/{version}.rs"))).unwrap();
    let o = Options { function: "pick".into(), file: "src/lib.rs".into(), version: version.into() };
    lower(&src, &o).unwrap()
}

fn on_path(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

// 25.1: contract hash; Charon and Aeneas availability recorded
#[test]
fn fallbacks_line_1_is_contract_hash() {
    let out = Command::new("git").args(["log", "-1", "--format=%H", "--", "contracts/"]).current_dir(here().join("../..")).output().unwrap();
    let hash = String::from_utf8(out.stdout).unwrap().trim().to_string();
    let text = std::fs::read_to_string(here().join("FALLBACKS.md")).unwrap();
    assert_eq!(text.lines().next().unwrap().trim(), hash);
}

#[test]
fn charon_and_aeneas_recorded() {
    let text = std::fs::read_to_string(here().join("FALLBACKS.md")).unwrap();
    use translator_rust::tools::{aeneas, charon, version};
    match charon() {
        Some(c) => {
            let v = version(&c, "version").unwrap();
            assert!(text.contains(&v), "FALLBACKS.md must record charon {v}");
        }
        None => eprintln!("SKIPPED LOUDLY: charon not installed; run crates/translator-rust/tools/setup.sh"),
    }
    match aeneas() {
        Some(a) => {
            let v = version(&a, "-version").unwrap();
            assert!(text.contains(&v), "FALLBACKS.md must record {v}");
        }
        None => eprintln!("SKIPPED LOUDLY: aeneas not installed; run crates/translator-rust/tools/setup.sh"),
    }
}

#[test]
fn models_validate_against_contract() {
    let s = contracts::Schemas::contract().unwrap();
    for v in ["old", "new"] {
        s.validate("model", &model(v), "model").unwrap_or_else(|e| panic!("{v}: {e:?}"));
    }
}

#[test]
fn unsupported_syntax_is_not_modelled() {
    let src = "fn g(a: i64) -> i64 { a * 2 }";
    let o = Options { function: "g".into(), file: "x.rs".into(), version: "old".into() };
    let m = lower(src, &o).unwrap();
    assert_eq!(m["lines"]["notModelled"].as_array().unwrap().len(), 1);
    assert_eq!(m["opaque"][0]["name"], "unmodelled");
}

fn diff() -> (differ::model::Model, differ::model::Model, differ::UnitResult) {
    let o = differ::model::Model::parse(&model("old")).unwrap();
    let n = differ::model::Model::parse(&model("new")).unwrap();
    let r = differ::diff_models(&o, &n, &differ::Options::default()).unwrap();
    (o, n, r)
}

// 25.2: the differ finds the planted difference (panic at :3 removed)
#[test]
fn differ_finds_planted_difference() {
    if !on_path("z3") {
        panic!("z3 required");
    }
    let (_, _, r) = diff();
    let d: Vec<_> = r.differing().collect();
    assert!(!d.is_empty());
    assert!(d.iter().any(|x| x.old.tag == "throws" && x.new.tag != "throws" && x.literals.iter().any(|l| l.contains("flag"))), "{d:?}");
    // the index into `v` is unchanged: no row differs only through it
    assert!(d.iter().all(|x| x.old.tag == "throws" && x.old.text.contains("panic")), "{d:?}");
}

// 25.3: rows go through lean-certify; one reads "checked by Lean"
#[test]
fn lean_checks_rows() {
    if !on_path("lean") {
        panic!("lean required");
    }
    let (o, n, r) = diff();
    let rows = lean::emit::parse_rows(&differ::solver_rows_json(&r)).unwrap();
    let bridge = lean::emit::parse_bridge(&differ::bridge::bridge_json(&o, &n, &r)).unwrap();
    let (lo, ln) = (lean::emit::parse_model(&model("old")).unwrap(), lean::emit::parse_model(&model("new")).unwrap());
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("lean-pick");
    let u = lean::certify(&lo, &ln, &rows, &bridge, &out, &lean::Options::default()).unwrap();
    let words: Vec<_> = u.rows.iter().map(|x| lean::wording(x.state)).collect();
    assert!(words.iter().any(|w| *w == "checked by Lean"), "{:?}", u.rows);
    assert_eq!(u.coverage, "checked", "{}", u.coverage_reason);
}

// ---------------------------------------------------------------- Charon and Aeneas path (default)

fn charon_model(version: &str) -> Option<Value> {
    if translator_rust::tools::charon().is_none() {
        eprintln!("SKIPPED LOUDLY: charon not installed; run crates/translator-rust/tools/setup.sh");
        return None;
    }
    let o = Options { function: "pick".into(), file: "src/lib.rs".into(), version: version.into() };
    let work = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("charon");
    let (m, p) = translator_rust::lower_file(&here().join(format!("tests/data/{version}.rs")), &o, &work).unwrap();
    assert_eq!(p, translator_rust::Producer::Charon);
    Some(m)
}

// 25.2 on Charon's LLBC: model validates, differ finds the planted difference
#[test]
fn charon_llbc_lowered_and_differ_finds_planted_difference() {
    let (Some(old), Some(new)) = (charon_model("old"), charon_model("new")) else { return };
    let s = contracts::Schemas::contract().unwrap();
    s.validate("model", &old, "model").unwrap();
    s.validate("model", &new, "model").unwrap();
    assert_eq!(old["contracts"][0]["contract"], "index-empty", "{old}");
    let (o, n) = (differ::model::Model::parse(&old).unwrap(), differ::model::Model::parse(&new).unwrap());
    let r = differ::diff_models(&o, &n, &differ::Options::default()).unwrap();
    let d: Vec<_> = r.differing().collect();
    assert!(!d.is_empty(), "{old}\n{new}");
    assert!(d.iter().all(|x| x.old.tag == "throws" && x.old.text.contains("panic") && x.new.tag != "throws"), "{d:?}");
    assert!(d.iter().all(|x| x.literals.iter().any(|l| l.contains("flag"))), "{d:?}");
    // the differ's rows close in Lean (lean-certify, differ mode)
    let rows = lean::emit::parse_rows(&differ::solver_rows_json(&r)).unwrap();
    let bridge = lean::emit::parse_bridge(&differ::bridge::bridge_json(&o, &n, &r)).unwrap();
    let (lo, ln) = (lean::emit::parse_model(&old).unwrap(), lean::emit::parse_model(&new).unwrap());
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("lean-charon");
    let u = lean::certify(&lo, &ln, &rows, &bridge, &out, &lean::Options::default()).unwrap();
    assert!(u.rows.iter().any(|x| lean::wording(x.state) == "checked by Lean"), "{:?}", u.rows);
}

// 25.3 first half: Aeneas translates the pair to Lean; old asserts ¬flag, new does not
#[test]
fn aeneas_translates_pair_to_lean() {
    if translator_rust::tools::aeneas().is_none() || translator_rust::tools::charon().is_none() {
        eprintln!("SKIPPED LOUDLY: aeneas or charon not installed; run crates/translator-rust/tools/setup.sh");
        return;
    }
    let w = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("aeneas");
    let mut text = vec![];
    for v in ["old", "new"] {
        let llbc = w.join(format!("pick_{v}.llbc"));
        std::fs::create_dir_all(&w).unwrap();
        translator_rust::tools::charon_llbc(&here().join(format!("tests/data/{v}.rs")), &format!("pick_{v}"), &llbc, true).unwrap();
        text.push(translator_rust::tools::aeneas_to_lean(&llbc, &w.join(format!("lean_{v}"))).unwrap());
    }
    assert!(text[0].contains("def pick") && text[0].contains("massert (¬ flag)"), "{}", text[0]);
    assert!(text[1].contains("def pick") && !text[1].contains("massert"), "{}", text[1]);

    // 25.3 second half: the differing row (flag) stated over Aeneas's definitions, checked by Lean
    if translator_rust::tools::aeneas_lean().is_none() {
        eprintln!("SKIPPED LOUDLY: Aeneas Lean library not built (needs mathlib, ~10 GB); run crates/translator-rust/tools/setup.sh");
        return;
    }
    let body = |t: &str| t[t.find("namespace").unwrap()..].to_string();
    let header = &text[0][..text[0].find("namespace").unwrap()];
    let lean = format!(
        "{header}{}\n{}\n\
         theorem row_flag (ok1 : Bool) (v : Slice Std.I64) :\n  \
         (∃ e, pick_old.pick true ok1 v = .fail e) ∧ (∃ x, pick_new.pick true ok1 v = .ok x) := by\n  \
         simp [pick_old.pick, pick_new.pick, massert]\n",
        body(&text[0]),
        body(&text[1])
    );
    let f = w.join("Rows.lean");
    std::fs::write(&f, lean).unwrap();
    let (ok, out) = translator_rust::tools::check_lean(&f).unwrap();
    assert!(ok && !out.contains("sorry"), "{out}");
    eprintln!("row flag: checked by Lean (Aeneas)");
}
