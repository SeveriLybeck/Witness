use lean::check::{lean_version, run_lean, Limits};
use lean::emit::{parse_bridge, parse_model, parse_rows, Bridge, Model, Span, SolverRows};
use lean::proofs::{ProofSource, Recorded};
use lean::{certify, fold, wording, Options, CHECKED, MODEL_CHECKED, NOT_CHECKED};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn read(p: PathBuf) -> Value {
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p:?}: {e}"))).unwrap()
}
fn inputs() -> PathBuf {
    root().join("contracts/fixtures/2283/inputs")
}
fn models() -> (Model, Model) {
    (parse_model(&read(inputs().join("model.old.json"))).unwrap(), parse_model(&read(inputs().join("model.new.json"))).unwrap())
}
fn data(f: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(f)
}
fn rows985() -> SolverRows {
    parse_rows(&read(data("2283-985/solver-rows.json"))).unwrap()
}
fn bridge985() -> Bridge {
    parse_bridge(&read(data("2283-985/bridge.json"))).unwrap()
}
fn out(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("lean-tests").join(name);
    let _ = std::fs::remove_dir_all(&d);
    d
}
fn validate(j: &Value) {
    let s = contracts::Schemas::contract().expect("schemas");
    s.validate("lean", j, "lean.json").unwrap_or_else(|e| panic!("lean.json invalid: {e:?}"));
}

#[test]
fn toolchain_and_fallbacks_hash() {
    let v = lean_version().unwrap();
    assert!(v.contains("4.29.1"), "{v}");
    let git = std::process::Command::new("git")
        .args(["log", "-1", "--format=%H", "--", "contracts/"])
        .current_dir(root())
        .output()
        .unwrap();
    let hash = String::from_utf8(git.stdout).unwrap().trim().to_string();
    let f = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("FALLBACKS.md")).unwrap();
    assert_eq!(f.lines().next().unwrap().trim(), hash);
    // core Lean only: emitted text imports nothing
    assert!(!lean::emit::PRELUDE.contains("import") && !fold::FOLD_LEMMAS.contains("import"));
}

#[test]
fn rows_2283_accepted() {
    let (o, n) = models();
    let o_dir = out("accepted");
    let r = certify(&o, &n, &rows985(), &bridge985(), &o_dir, &Options::default()).unwrap();
    for x in &r.rows {
        assert_eq!(x.state, CHECKED, "row {}: {}", x.n, x.reason);
        assert!(x.seconds.unwrap() < 30.0);
    }
    assert_eq!(r.coverage, CHECKED, "{}", r.coverage_reason);
    let j = r.to_json();
    validate(&j);
    assert_eq!(j["rows"].as_array().unwrap().len(), 3);
    let text = std::fs::read_to_string(o_dir.join(&r.file)).unwrap();
    assert!(!text.contains("sorry") && !text.contains("grind") && !text.contains("axiom "));
    assert_eq!(wording(CHECKED), "checked by Lean");
}

#[test]
fn wrong_row_rejected_at_that_row() {
    let (o, n) = models();
    let mut rows = rows985();
    rows.rows[1].new_tag = rows.rows[1].old_tag.clone();
    let r = certify(&o, &n, &rows, &bridge985(), &out("wrong"), &Options::default()).unwrap();
    for x in &r.rows {
        let want = if x.n == 1 { NOT_CHECKED } else { CHECKED };
        assert_eq!(x.state, want, "row {}: {}", x.n, x.reason);
    }
    assert!(r.row(1).reason.starts_with("error"), "{}", r.row(1).reason);
    assert_eq!(wording(r.row(1).state), "found by the solver, not checked by Lean");
}

#[test]
fn missing_case_rejected_by_coverage() {
    let (o, n) = models();
    let opts = Options { drop_from_coverage: [5].into_iter().collect(), ..Default::default() };
    let r = certify(&o, &n, &rows985(), &bridge985(), &out("missing"), &opts).unwrap();
    assert_eq!(r.coverage, NOT_CHECKED);
    assert!(r.rows.iter().all(|x| x.state == CHECKED));
}

#[test]
fn sorry_is_not_counted() {
    let (o, n) = models();
    let opts = Options { proof_override: [("row_2".to_string(), "by sorry".to_string())].into(), ..Default::default() };
    let r = certify(&o, &n, &rows985(), &bridge985(), &out("sorry"), &opts).unwrap();
    assert_eq!(r.row(2).state, NOT_CHECKED);
    assert_eq!(r.row(2).reason, "sorry");
    assert!(r.rows.iter().filter(|x| x.n != 2).all(|x| x.state == CHECKED), "file still compiles");
    assert_eq!(wording(r.row(2).state), "found by the solver, not checked by Lean");
    let j = r.to_json();
    let st: Vec<&str> = j["rows"].as_array().unwrap().iter().map(|x| x["state"].as_str().unwrap()).collect();
    assert_eq!(st.iter().filter(|s| **s == NOT_CHECKED).count(), 1);
}

#[test]
fn extra_axiom_is_not_counted() {
    let (o, n) = models();
    let opts = Options {
        extra_decls: vec!["axiom cheat : False".into()],
        proof_override: [("row_2".to_string(), "cheat.elim".to_string())].into(),
        ..Default::default()
    };
    let r = certify(&o, &n, &rows985(), &bridge985(), &out("axiom"), &opts).unwrap();
    assert_eq!(r.row(2).state, NOT_CHECKED);
    assert_eq!(r.row(2).reason, "axiom cheat");
    assert_eq!(r.row(1).state, CHECKED);
}

#[test]
fn contract_fixture_states() {
    // Corrected fixture (contracts round 2): the `:985` rows, complete. Its atoms name no
    // model term, so without a bridge no row lemma exists; the Boolean coverage holds.
    let (o, n) = models();
    let rows = parse_rows(&read(inputs().join("solver-rows.json"))).unwrap();
    let r = certify(&o, &n, &rows, &Bridge::default(), &out("contract"), &Options::default()).unwrap();
    assert_eq!(r.rows.len(), 6);
    assert!(r.rows.iter().all(|x| x.state == NOT_CHECKED && x.reason.contains("no model term")), "{:?}", r.rows);
    assert_eq!(r.coverage, CHECKED, "{}", r.coverage_reason);
    validate(&r.to_json());
    // with the `:985` bridge every row and coverage are checked
    let r = certify(&o, &n, &rows, &bridge985(), &out("contract-bridged"), &Options::default()).unwrap();
    assert!(r.rows.iter().all(|x| x.state == CHECKED), "{:?}", r.rows);
    assert_eq!(r.coverage, CHECKED, "{}", r.coverage_reason);
}

#[test]
fn rows_without_model_term_stay_not_checked() {
    // own data: drop one literal from the bridge; only the rows using it lose their lemma
    let (o, n) = models();
    let mut b = bridge985();
    b.literals.remove("locatedInCompilation");
    let r = certify(&o, &n, &rows985(), &b, &out("no-term"), &Options::default()).unwrap();
    for x in &r.rows {
        let uses = rows985().rows.iter().find(|y| y.n == x.n).unwrap().literals.iter().any(|l| l.contains("locatedInCompilation"));
        if uses {
            assert_eq!(x.state, NOT_CHECKED);
            assert!(x.reason.contains("no model term"), "{}", x.reason);
        } else {
            assert_eq!(x.state, CHECKED, "row {}: {}", x.n, x.reason);
        }
    }
}

struct Fixed(Vec<&'static str>);
impl ProofSource for Fixed {
    fn propose(&self, _s: &str, _e: &str, attempt: u32) -> Option<String> {
        self.0.get(attempt as usize - 1).map(|p| p.to_string())
    }
}

#[test]
fn model_written_proofs() {
    let (o, n) = models();
    let broken = || -> std::collections::BTreeMap<String, String> { [("row_2".to_string(), "by rfl".to_string())].into() };
    // no recording: no call, stays not-checked
    let empty = out("norec");
    std::fs::create_dir_all(&empty).unwrap();
    let rec = Recorded { dir: empty.clone() };
    let opts = Options { proof_override: broken(), proofs: Some(&rec), ..Default::default() };
    let r = certify(&o, &n, &rows985(), &bridge985(), &out("model-none"), &opts).unwrap();
    assert_eq!(r.row(2).state, NOT_CHECKED);
    // model proof with sorry, then an axiom-free bad one: stays not-checked
    let bad = Fixed(vec!["by sorry", "by trivial"]);
    let opts = Options { proof_override: broken(), proofs: Some(&bad), ..Default::default() };
    let r = certify(&o, &n, &rows985(), &bridge985(), &out("model-sorry"), &opts).unwrap();
    assert_eq!(r.row(2).state, NOT_CHECKED);
    // third attempt succeeds
    let good = Fixed(vec!["by sorry", "by trivial", "by\n  simp_all [U.old_f, U.new_f, W.Out.tag]"]);
    let opts = Options { proof_override: broken(), proofs: Some(&good), ..Default::default() };
    let r = certify(&o, &n, &rows985(), &bridge985(), &out("model-good"), &opts).unwrap();
    assert_eq!(r.row(2).state, MODEL_CHECKED, "{}", r.row(2).reason);
    assert_eq!(wording(MODEL_CHECKED), "proof found by a model, checked by Lean");
    validate(&r.to_json());
    // recordings are read by key
    let key = contracts::recording_key(lean::proofs::PURPOSE, &lean::proofs::prompt("S", "E", 1), &lean::proofs::schema());
    std::fs::write(empty.join(format!("{key}.json")), r#"{"purpose":"lean-proof","prompt":"","schema":{},"response":{"proof":"by rfl"}}"#).unwrap();
    assert_eq!(rec.propose("S", "E", 1).as_deref(), Some("by rfl"));
    assert_eq!(rec.propose("S", "E", 2), None);
}

#[test]
fn fold_lifts_2234_body_lemma() {
    let defs = std::fs::read_to_string(data("m2234.lean")).unwrap();
    let mut text = String::from("set_option linter.unusedVariables false\n");
    text.push_str(fold::FOLD_LEMMAS);
    text.push_str(&defs);
    text.push_str("open M2234\n");
    let start1 = text.lines().count() + 1;
    text.push_str(
        "theorem new_nothrow_prop {IK SK I : Type} [DecidableEq IK] [DecidableEq SK] (E : Env IK SK I) (s : St IK SK I) (i : I) :\n    W.throws (bodyNew E false s i) = false := by\n  fixed_script\n#print axioms new_nothrow_prop\n",
    );
    let end1 = text.lines().count();
    let start2 = end1 + 1;
    text.push_str(&fold::lift_nothrow(
        "new_loop_nothrow_prop",
        "{IK SK I : Type} [DecidableEq IK] [DecidableEq SK] (E : Env IK SK I)",
        "bodyNew E false",
        "new_nothrow_prop E",
    ));
    let end2 = text.lines().count();
    assert!(!text.contains("[i1"), "no fixed-length lemma");
    let d = out("fold");
    std::fs::create_dir_all(&d).unwrap();
    let f = d.join("Fold2234.lean");
    std::fs::write(&f, &text).unwrap();
    let spans = vec![
        Span { name: "body".into(), start: start1, end: end1 },
        Span { name: "lift".into(), start: start2, end: end2 },
    ];
    let run = run_lean(&f, &spans, Limits::default());
    for (k, v) in &run.verdicts {
        assert!(v.accepted, "{k}: {} {:?}", v.reason, run.messages);
    }
}

fn differ_chain(rows: SolverRows, bridge: Bridge, name: &str) {
    let (o, n) = models();
    // differ vocabulary: literals translate directly, the bridge is not needed
    let r = certify(&o, &n, &rows, &bridge, &out(name), &Options::default()).unwrap();
    assert_eq!(r.rows.len(), 6);
    assert!(r.rows.iter().all(|x| x.state == CHECKED), "{:?}", r.rows);
    assert_eq!(r.rows.iter().filter(|x| x.row.is_some()).count(), 3);
    assert_eq!(r.coverage, CHECKED, "{}", r.coverage_reason);
    validate(&r.to_json());
    for drop in [0u64, 3] {
        let opts = Options { drop_from_coverage: [drop].into_iter().collect(), ..Default::default() };
        let r = certify(&o, &n, &rows, &bridge, &out(&format!("{name}-drop{drop}")), &opts).unwrap();
        assert_eq!(r.coverage, NOT_CHECKED, "drop {drop}");
    }
}

#[test]
fn differ_2283_saved_output() {
    let rows = parse_rows(&read(data("2283-differ/solver-rows.json"))).unwrap();
    let bridge = parse_bridge(&read(data("2283-differ/bridge.json"))).unwrap();
    differ_chain(rows, bridge, "differ-saved");
}

#[test]
fn differ_2283_live() {
    let z3 = std::process::Command::new("z3").arg("--version").output();
    if z3.map(|o| !o.status.success()).unwrap_or(true) {
        eprintln!("skipped: no z3 on PATH");
        return;
    }
    let d = out("differ-live");
    std::fs::create_dir_all(&d).unwrap();
    let i = inputs();
    let st = std::process::Command::new("cargo")
        .args(["run", "-q", "-j", "2", "-p", "differ", "--", "--old"])
        .arg(i.join("model.old.json"))
        .arg("--new")
        .arg(i.join("model.new.json"))
        .arg("--out")
        .arg(d.join("solver-rows.json"))
        .arg("--rows")
        .arg(d.join("rows.json"))
        .arg("--bridge")
        .arg(d.join("bridge.json"))
        .arg("--scope-in")
        .arg(root().join("contracts/fixtures/2283/scope.json"))
        .arg("--scope-out")
        .arg(d.join("scope.json"))
        .current_dir(root())
        .status()
        .unwrap();
    assert!(st.success());
    let rows = parse_rows(&read(d.join("solver-rows.json"))).unwrap();
    let bridge = parse_bridge(&read(d.join("bridge.json"))).unwrap();
    differ_chain(rows, bridge, "differ-live");
}

/// Real 2283 chain (translator on the real worker replies, then differ), saved by
/// `tests/data/2283-real/regenerate.sh`.
fn real(site: &str) -> (Model, Model, SolverRows, Bridge) {
    let f = |k: &str| read(data(&format!("2283-real/{k}.{site}.json")));
    (
        parse_model(&f("model.old")).unwrap(),
        parse_model(&f("model.new")).unwrap(),
        parse_rows(&f("solver-rows")).unwrap(),
        parse_bridge(&f("bridge")).unwrap(),
    )
}

#[test]
fn real_2283_rows_and_coverage() {
    // refreshed replies: :1077 models carry the `null-receiver` contract, which the mirror reads like any contract
    assert!(read(data("2283-real/model.new.1077.json")).to_string().contains("\"null-receiver\""));
    for (site, total, differing) in [("985", 6, 4), ("1077", 37, 22)] {
        let (o, n, rows, b) = real(site);
        let r = certify(&o, &n, &rows, &b, &out(&format!("real-{site}")), &Options::default()).unwrap();
        assert_eq!(r.rows.len(), total, "{site}");
        for x in &r.rows {
            assert_eq!(x.state, CHECKED, "{site} row {}: {}", x.n, x.reason);
        }
        assert_eq!(r.rows.iter().filter(|x| x.row.is_some()).count(), differing, "{site}");
        assert_eq!(r.coverage, CHECKED, "{site}: {}", r.coverage_reason);
        assert!(r.seconds.unwrap() < 30.0 * (r.rows.len() + 1) as f64, "per-theorem budget");
        validate(&r.to_json());
    }
}

#[test]
fn real_2283_new_only_units() {
    // new helpers lowered alone (`differ --new-only`, old = nothing): rows are the throwing paths;
    // neither helper throws, so no row has an ID and every lemma and coverage is checked
    for m in ["GetPropertyIdentifierLocation", "ReportIfLocatedInCompilation"] {
        let (o, n, rows, b) = real(m);
        let r = certify(&o, &n, &rows, &b, &out(&format!("new-only-{m}")), &Options::default()).unwrap();
        assert_eq!(r.rows.len(), 3, "{m}");
        assert!(r.rows.iter().all(|x| x.state == CHECKED && x.row.is_none()), "{m}: {:?}", r.rows);
        assert_eq!(r.coverage, CHECKED, "{m}: {}", r.coverage_reason);
        validate(&r.to_json());
    }
}

#[test]
fn real_2283_wrong_row_and_dropped_case() {
    let (o, n, mut rows, b) = real("985");
    let i = rows.rows.iter().position(|r| r.row.is_some()).unwrap();
    rows.rows[i].new_tag = rows.rows[i].old_tag.clone();
    let wrong = rows.rows[i].n;
    let opts = Options { drop_from_coverage: [rows.rows[0].n].into_iter().collect(), ..Default::default() };
    let r = certify(&o, &n, &rows, &b, &out("real-wrong"), &opts).unwrap();
    for x in &r.rows {
        assert_eq!(x.state, if x.n == wrong { NOT_CHECKED } else { CHECKED }, "row {}", x.n);
    }
    assert_eq!(r.coverage, NOT_CHECKED);
}

/// Fails when the differ gains a library fact the mirror mode does not reproduce.
#[test]
fn mirror_covers_every_differ_fact() {
    for f in differ::exec::FACTS {
        assert!(lean::mirror::COVERED_FACTS.contains(&f.key), "differ fact `{}` ({}) has no mirror counterpart", f.key, f.text);
        // pin content too: the mirror fact's text must be the differ's text, word for word
        let t = lean::mirror::FACT_TEXT.iter().find(|(x, _)| *x == f.key).map(|x| x.1);
        assert_eq!(t, Some(f.text), "differ fact `{}` changed its statement", f.key);
        let s = lean::mirror::FACT_SMT.iter().find(|(x, _)| *x == f.key).map(|x| x.1);
        assert_eq!(s, Some(f.smt), "differ fact `{}` changed its SMT statement: review mirror::opaque_facts", f.key);
    }
    for (k, what) in differ::exec::READINGS {
        assert!(lean::mirror::COVERED_FACTS.contains(k), "differ reading `{k}` ({what}) has no mirror counterpart");
    }
}

/// `lean.json` lists exactly the facts the unit's lemmas take as hypotheses.
#[test]
fn facts_listed_are_the_hypotheses() {
    let (o, n, rows, b) = real("985");
    let r = certify(&o, &n, &rows, &b, &out("facts-985"), &Options::default()).unwrap();
    let j = r.to_json();
    validate(&j);
    let listed: Vec<String> = j["coverage"][0]["facts"].as_array().unwrap().iter().map(|f| f["lean"].as_str().unwrap().to_string()).collect();
    let keys: Vec<&str> = j["coverage"][0]["facts"].as_array().unwrap().iter().map(|f| f["key"].as_str().unwrap()).collect();
    assert_eq!(keys, ["length-non-negative", "dsr-elements-non-null", "first-or-default", "getsyntax-non-null", "isinsource-null-false"]);
    let text = std::fs::read_to_string(Path::new(env!("CARGO_TARGET_TMPDIR")).join("lean-tests/facts-985").join(&r.file)).unwrap();
    for th in text.split("\ntheorem ").skip(1) {
        let stmt = &th[..th.find(":=").unwrap()];
        let mut hyps = vec![];
        let mut rest = stmt;
        while let Some(i) = rest.find("(k") {
            let s = &rest[i + 1..];
            let colon = s.find(" : ").unwrap();
            let mut depth = 1;
            let mut end = 0;
            for (p, c) in s.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = p;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            hyps.push(s[colon + 3..end].to_string());
            rest = &s[end..];
        }
        assert_eq!(hyps, listed, "theorem {}", &th[..th.find(' ').unwrap()]);
    }
    for f in j["coverage"][0]["facts"].as_array().unwrap() {
        assert!(!f["text"].as_str().unwrap().is_empty());
        assert_eq!(f["rows"].as_array().unwrap().len(), 4);
    }
}

fn real2234(site: &str) -> (Model, Model, SolverRows) {
    let f = |k: &str| read(data(&format!("2234-real/{k}.{site}.json")));
    (parse_model(&f("model.old")).unwrap(), parse_model(&f("model.new")).unwrap(), parse_rows(&f("solver-rows")).unwrap())
}

#[test]
fn real_2234_per_iteration() {
    // one loop unit for the whole property `foreach` body (unit rule D19, `--per-iteration --exact`);
    // with the `continue` inside the unit no new path reaches `intKey.Value` on a null key, so the lift holds
    for (site, total, differing, lifted) in [("1005", 23, 18, CHECKED)] {
        let (o, n, rows) = real2234(site);
        let r = certify(&o, &n, &rows, &Bridge::default(), &out(&format!("real2234-{site}")), &Options::default()).unwrap();
        assert_eq!(r.rows.len(), total);
        for x in &r.rows {
            assert_eq!(x.state, CHECKED, "{site} row {}: {}", x.n, x.reason);
        }
        assert_eq!(r.rows.iter().filter(|x| x.row.is_some()).count(), differing);
        assert_eq!(r.coverage, CHECKED, "{site}: {}", r.coverage_reason);
        assert_eq!(r.per_iteration, Some(lifted), "{site}: {}", r.lift_reason);
        assert_eq!(r.lifted, Some(lifted), "{site}: {}", r.lift_reason);
        let j = r.to_json();
        validate(&j);
        assert_eq!(j["loops"][0]["lifted"], lifted);
        assert_eq!(j["loops"][0]["carried"]["lean"], NOT_CHECKED, "carried state is not claimed by Lean");
    }
}

#[test]
fn real_2234_wrong_row_and_dropped_case() {
    let (o, n, mut rows) = real2234("1005");
    let i = rows.rows.iter().position(|r| r.row.is_some()).unwrap();
    rows.rows[i].new_tag = rows.rows[i].old_tag.clone();
    let wrong = rows.rows[i].n;
    let r = certify(&o, &n, &rows, &Bridge::default(), &out("real2234-wrong"), &Options::default()).unwrap();
    for x in &r.rows {
        assert_eq!(x.state, if x.n == wrong { NOT_CHECKED } else { CHECKED }, "row {}", x.n);
    }
    let (o, n, rows) = real2234("1005");
    let opts = Options { drop_from_coverage: [rows.rows[3].n].into_iter().collect(), ..Default::default() };
    let r = certify(&o, &n, &rows, &Bridge::default(), &out("real2234-drop"), &opts).unwrap();
    assert_eq!(r.coverage, NOT_CHECKED);
    assert_eq!(r.lifted, Some(NOT_CHECKED));
}

/// The two-argument GetSyntax (with a cancellation token) gets its non-null fact.
#[test]
fn getsyntax_fact_two_arguments() {
    let (o, n, rows, _) = real("985");
    let em = lean::mirror::emit(&o, &n, &rows, &Default::default());
    let t = &em.theorems[0].statement;
    assert!(t.contains("∀ y0 y1, y0 ≠ w_null → (getSyntax_"), "{t}");
}
