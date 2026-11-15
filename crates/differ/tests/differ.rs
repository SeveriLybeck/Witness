mod support;

use differ::interp::{self, Oracle, V};
use differ::model::{Model, Opaque};
use differ::{diff_models, group, kind_of, solver_rows_json, throwing_paths, Options, Outcome};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use support::*;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(name: &str) -> Model {
    let p = repo().join("contracts/fixtures/2283/inputs").join(name);
    Model::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn m(v: &Value) -> Model {
    Model::parse(v).unwrap()
}

fn opts() -> Options {
    Options::default()
}

// ---------------------------------------------------------------- 6.1 toolchain

#[test]
fn z3_is_4_16_0() {
    assert!(differ::smt::version().unwrap().contains("4.16.0"));
}

#[test]
fn fallbacks_line_1_is_contract_hash() {
    let out = std::process::Command::new("git")
        .args(["log", "-1", "--format=%H", "--", "contracts/"])
        .current_dir(repo())
        .output()
        .unwrap();
    let hash = String::from_utf8(out.stdout).unwrap().trim().to_string();
    let text = std::fs::read_to_string(repo().join("crates/differ/FALLBACKS.md")).unwrap();
    assert_eq!(text.lines().next().unwrap().trim(), hash);
}

// ---------------------------------------------------------------- 6.2 rows and differences

#[test]
fn fixture_2283_rows_validate_and_group() {
    let r = diff_models(&fixture("model.old.json"), &fixture("model.new.json"), &opts()).unwrap();
    let schemas = contracts::Schemas::contract().unwrap();
    schemas.validate("solver-rows", &solver_rows_json(&r), "solver-rows").unwrap();
    let gs = group(r.differing());
    let kinds: Vec<_> = gs.iter().map(|g| g.kind).collect();
    assert_eq!(kinds, ["crash-fixed", "crash-fixed", "silent-failure"], "{gs:?}");
    let crash: Vec<_> = r.differing().filter(|x| x.kind == Some("crash-fixed")).collect();
    assert!(crash.iter().any(|x| x.old.text == "IndexOutOfRangeException" && x.assumptions == ["index-empty"] && x.new.tag == "nothing"));
    assert!(crash.iter().all(|x| x.new.tag == "nothing"));
    // identical rows are kept, with no row id
    assert!(r.rows.iter().any(|x| x.id.is_none() && x.old.tag == "reports" && x.new.tag == "reports"));
}

#[test]
fn both_sites_2283_three_differences() {
    let r = diff_models(&m(&tc2283_old()), &m(&tc2283_new()), &opts()).unwrap();
    let gs = group(r.differing());
    let got: Vec<_> = gs.iter().map(|g| (g.kind, g.old_tag.as_str(), g.new_tag.as_str(), g.location.as_ref().unwrap().line)).collect();
    // IndexOutOfRange and InvalidCast at :985, two diagnostics dropped by the wrapper
    assert_eq!(got.len(), 5, "{got:?}");
    assert!(got.contains(&("crash-fixed", "throws", "nothing", 985)));
    assert!(got.contains(&("crash-fixed", "throws", "nothing", 1077)));
    assert!(got.contains(&("silent-failure", "reports", "nothing", 239)));
    // The field-site crash also happens on the generator path (no reportSet literal needed).
    assert!(r.differing().any(|x| x.first_changed.as_ref().unwrap().line == 1077 && x.kind == Some("crash-fixed")
        && x.literals.iter().any(|l| l == "reportIsNull")));
    assert!(r.new_inputs.contains(&"comp".to_string()));
}

#[test]
fn removing_index_empty_removes_its_crash() {
    let o = Options { disabled: vec!["index-empty".into()], ..opts() };
    let r = diff_models(&m(&tc2283_old()), &m(&tc2283_new()), &o).unwrap();
    assert!(r.differing().all(|x| !x.assumptions.contains(&"index-empty".to_string()) && x.old.text != "IndexOutOfRangeException"));
    let o = Options { disabled: vec!["index-empty".into(), "failed-cast".into()], ..opts() };
    let r = diff_models(&m(&tc2283_old()), &m(&tc2283_new()), &o).unwrap();
    assert!(r.differing().all(|x| x.kind != Some("crash-fixed")));
}

#[test]
fn same_version_has_no_rows() {
    for v in [tc2283_old(), tc2283_new(), tc2234(false), tc2234(true)] {
        let r = diff_models(&m(&v), &m(&v), &opts()).unwrap();
        assert_eq!(r.differing().count(), 0);
        assert!(!r.rows.is_empty());
    }
    let f = fixture("model.old.json");
    assert_eq!(diff_models(&f, &f, &opts()).unwrap().differing().count(), 0);
}

// ---------------------------------------------------------------- 6.3 kinds

fn o(tag: &str, text: &str) -> Outcome {
    Outcome::new(tag, text)
}

#[test]
fn kind_table() {
    let nothing = o("nothing", "nothing reported");
    assert_eq!(kind_of(&o("throws", "IndexOutOfRangeException"), &nothing, None, false, false), "crash-fixed");
    assert_eq!(kind_of(&nothing, &o("throws", "NullReferenceException"), None, false, false), "null-access");
    assert_eq!(kind_of(&nothing, &o("throws", "ArgumentException"), None, true, false), "null-access");
    assert_eq!(kind_of(&nothing, &o("throws", "ArgumentException"), None, false, false), "crash");
    assert_eq!(kind_of(&o("reports", "MsgPack004"), &nothing, None, false, false), "silent-failure");
    assert_eq!(kind_of(&o("value", "independent of b"), &o("value", "depends on b"), Some("no-new-dependency"), false, false), "new-dependency");
    assert_eq!(kind_of(&o("value", "1"), &o("value", "2"), None, false, true), "public-api");
    assert_eq!(kind_of(&o("value", "1"), &o("value", "2"), None, false, false), "output-changed");
}

fn tiny(version: &str, params: &[(&str, &str)], body: Value, contracts: Vec<Value>) -> Value {
    let mut v = model("U", version, params, vec![opaque("f", "M:F(System.Object)", &["Obj"], "Obj")], contracts, body);
    v["lines"]["modelled"] = json!([1]);
    v
}

fn only_kind(old: Value, new: Value) -> Vec<&'static str> {
    kinds_with(old, new, true)
}

fn kinds_with(old: Value, new: Value, no_dependency: bool) -> Vec<&'static str> {
    let r = diff_models(&m(&old), &m(&new), &Options { no_dependency, file: Some("U.cs".into()), ..opts() }).unwrap();
    r.differing().map(|x| x.kind.unwrap()).collect()
}

#[test]
fn kinds_from_the_executor() {
    let p = &[("x", "Obj"), ("a", "Bool"), ("b", "Bool")];
    let nr = vec![json!({"contract": "null-receiver", "symbol": "M:F"})];
    let null_call = json!({"op": "call", "name": "f", "contract": "null-receiver", "args": [var("x")]});
    let ks = only_kind(tiny("old", p, nothing(), vec![]), tiny("new", p, op("seq", vec![null_call, nothing()]), nr));
    assert!(ks.contains(&"null-access"), "{ks:?}");
    let ks = only_kind(tiny("old", p, nothing(), vec![]), tiny("new", p, ite(var("a"), json!({"op": "throw", "name": "ArgumentException"}), nothing()), vec![]));
    assert_eq!(ks, ["crash"]);
    let ks = only_kind(tiny("old", p, lit(json!(1)), vec![]), tiny("new", p, lit(json!(2)), vec![]));
    assert_eq!(ks, ["output-changed"]);
    let ks = only_kind(tiny("old", p, report("D", vec![]), vec![]), tiny("new", p, nothing(), vec![]));
    assert_eq!(ks, ["silent-failure"]);
    let ks = only_kind(tiny("old", p, json!({"op": "throw", "name": "X"}), vec![]), tiny("new", p, nothing(), vec![]));
    assert_eq!(ks, ["crash-fixed"]);
    // new reads b, old does not
    let ks = kinds_with(tiny("old", p, ite(var("a"), report("D", vec![]), nothing()), vec![]),
        tiny("new", p, ite(op("and", vec![var("a"), var("b")]), report("D", vec![]), nothing()), vec![]), false);
    assert!(ks.contains(&"silent-failure") && ks.contains(&"new-dependency"), "{ks:?}");
}

// ---------------------------------------------------------------- 6.4 properties and loops

#[test]
fn tc2234_new_body_never_throws() {
    assert!(throwing_paths(&m(&tc2234(true)), &[]).unwrap().is_empty());
    let old = throwing_paths(&m(&tc2234(false)), &[]).unwrap();
    assert!(old.iter().any(|(_, e, a)| e == "InvalidOperationException" && a == &["nullable-value-null"]));
    assert!(old.iter().any(|(_, e, _)| e == "ArgumentException"));
}

#[test]
fn tc2234_rows_per_iteration_and_no_suppress_dependency() {
    let r = diff_models(&m(&tc2234(false)), &m(&tc2234(true)), &opts()).unwrap();
    assert!(r.differing().count() > 0);
    assert!(r.rows.iter().all(|x| x.per_iteration));
    assert!(r.differing().all(|x| x.property != Some("no-exception")));
    assert!(!r.differing().any(|x| x.property == Some("no-new-dependency") && x.new.text.contains("suppress")));
    // the silent loss: CollectCore no longer called for a string-keyed member without a string key
    let kinds: Vec<_> = group(r.differing()).iter().map(|g| g.kind).collect();
    assert!(kinds.contains(&"crash-fixed"), "{kinds:?}");
}

// ---------------------------------------------------------------- 6.5 interpreter and mutations

/// Concrete Roslyn behaviour for the issue's reproduction: a member from a referenced
/// assembly, so `DeclaringSyntaxReferences` is empty.
struct Repro;

impl Oracle for Repro {
    fn call(&mut self, o: &Opaque, args: &[V]) -> V {
        match (o.name.as_str(), args) {
            ("dsr", _) => V::Arr(vec![]),
            ("firstOrDefault", [V::Arr(xs)]) => xs.first().cloned().unwrap_or(V::Null),
            ("isInSource" | "containsTree" | "located", _) => V::B(false),
            (_, [V::Null, ..]) => V::Null,
            (n, _) => V::Obj(n.to_string()),
        }
    }
}

/// The real outcome of one side of `worker-reply.run.json`.
fn real(side: &Value) -> Outcome {
    let diags = side["diagnostics"].as_array().unwrap();
    for d in diags {
        let msg = d["message"].as_str().unwrap();
        if d["id"] == "AD0001" || d["id"] == "CS8785" {
            let exc = msg.split(|c: char| !c.is_alphanumeric()).find(|w| w.ends_with("Exception")).unwrap();
            return o("throws", exc);
        }
    }
    if diags.is_empty() { o("nothing", "nothing reported") } else { o("reports", "") }
}

fn repro_inputs() -> HashMap<String, V> {
    HashMap::from([
        ("isProperty".into(), V::B(false)), ("reportIsNull".into(), V::B(false)), ("nullKey".into(), V::B(false)),
        ("derivedNull".into(), V::B(true)), ("item".into(), V::Obj("Lib.S<int>.Unkeyed".into())), ("comp".into(), V::Obj("comp".into())),
    ])
}

fn conforms(old: &Value, new: &Value) -> bool {
    let run: Value = serde_json::from_str(&std::fs::read_to_string(repo().join("contracts/fixtures/2283/inputs/worker-reply.run.json")).unwrap()).unwrap();
    let ro = interp::run(&m(old), &repro_inputs(), &mut Repro, &[]).unwrap().outcome();
    let rn = interp::run(&m(new), &repro_inputs(), &mut Repro, &[]).unwrap().outcome();
    ro == real(&run["result"]["old"]) && rn == real(&run["result"]["new"])
}

#[test]
fn interpreter_matches_real_run() {
    assert!(conforms(&tc2283_old(), &tc2283_new()));
    // and the differ has a row with exactly that outcome pair
    let r = diff_models(&m(&tc2283_old()), &m(&tc2283_new()), &opts()).unwrap();
    assert!(r.differing().any(|x| x.old == o("throws", "IndexOutOfRangeException") && x.new == o("nothing", "nothing reported")));
}

#[test]
fn interpreter_on_fixture_models() {
    let mut inputs = repro_inputs();
    inputs.insert("nullKey".into(), V::B(true));
    let a = interp::run(&fixture("model.old.json"), &inputs, &mut Repro, &[]).unwrap().outcome();
    let b = interp::run(&fixture("model.new.json"), &inputs, &mut Repro, &[]).unwrap().outcome();
    assert_eq!((a, b), (o("throws", "IndexOutOfRangeException"), o("nothing", "nothing reported")));
}

#[test]
fn flipped_branch_fails_conformance() {
    let mut old = tc2283_old();
    // if 0 = isProperty, if 1.. inside; flip the field-site `if derivedNull`
    let mut k = 0;
    loop {
        let mut mutant = old.clone();
        assert!(flip_if(&mut mutant, k), "no branch on the reproduction path found");
        if !conforms(&mutant, &tc2283_new()) {
            break;
        }
        k += 1;
    }
    flip_if(&mut old, k);
    assert!(!conforms(&old, &tc2283_new()));
}

#[test]
fn every_flipped_branch_is_detected() {
    for base in [tc2283_old(), tc2283_new(), tc2234(true)] {
        let mut k = 0;
        loop {
            let mut mutant = base.clone();
            if !flip_if(&mut mutant, k) {
                break;
            }
            let r = diff_models(&m(&base), &m(&mutant), &Options { no_dependency: true, ..opts() }).unwrap();
            assert!(r.differing().count() > 0, "flip {k} of {} not detected", base["version"]);
            k += 1;
        }
        assert!(k > 3);
    }
}

#[test]
fn wrapper_and_to_or_is_detected() {
    let r = diff_models(&m(&tc2283_new()), &m(&tc2283_new_with("or")), &opts()).unwrap();
    assert!(r.differing().count() > 0);
}

#[test]
fn index_zero_to_one_is_detected() {
    let mut mutant = tc2283_old();
    rewrite(&mut mutant, &|v| v["op"] == "index", &|v| v["args"][1] = lit(json!(1)));
    let r = diff_models(&m(&tc2283_old()), &m(&mutant), &opts()).unwrap();
    assert!(r.differing().count() > 0);
}

// ---------------------------------------------------------------- fit check: bridge, rows.json, scope

fn on_path(bin: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
}

#[test]
fn rows_record_and_scope_are_schema_valid() {
    let r = diff_models(&fixture("model.old.json"), &fixture("model.new.json"), &Options { first_row: 7, ..opts() }).unwrap();
    let rows = differ::rows_record(&[&r], 4).unwrap();
    let schemas = contracts::Schemas::contract().unwrap();
    schemas.validate("rows", &rows, "rows").unwrap();
    assert_eq!(rows["rows"][0]["id"], "r7");
    assert_eq!(rows["rows"][0]["difference"], "d4");
    assert!(rows["rows"].as_array().unwrap().iter().all(|x| x["kind"].is_string()));
    let mut scope: Value =
        serde_json::from_str(&std::fs::read_to_string(repo().join("contracts/fixtures/2283/scope.json")).unwrap()).unwrap();
    for a in scope["assumptions"].as_array_mut().unwrap() {
        a["rows"] = json!([]);
    }
    differ::fill_scope(&mut scope, &[&r]);
    schemas.validate("scope", &scope, "scope").unwrap();
    let idx = scope["assumptions"].as_array().unwrap().iter().find(|a| a["contract"] == "index-empty").unwrap();
    assert!(!idx["rows"].as_array().unwrap().is_empty());
}

#[test]
fn bridge_has_no_smt_names_and_covers_differing_rows() {
    let (o, n) = (fixture("model.old.json"), fixture("model.new.json"));
    let r = diff_models(&o, &n, &opts()).unwrap();
    let b = differ::bridge::bridge_json(&o, &n, &r);
    assert_eq!(b["facts"], json!(["∀ c, located c none = false",
        "((empty_RefArray (dsr item) = true) ∧ (firstOrDefault (dsr item) = none)) ∨ ((empty_RefArray (dsr item) = false) ∧ (firstOrDefault (dsr item) = some (at_RefArray (dsr item) (0 : Int))))"]));
    let lits = b["literals"].as_object().unwrap();
    for (k, v) in lits {
        for p in v.as_array().unwrap() {
            assert!(!p.as_str().unwrap().contains("w_"), "{k} -> {p}");
        }
    }
    assert!(r.differing().all(|x| x.literals.iter().all(|l| lits.contains_key(l))));
}

#[test]
fn lean_checks_bridged_rows() {
    if !on_path("lean") {
        eprintln!("lean not on PATH: skipped");
        return;
    }
    let root = repo();
    let st = std::process::Command::new(env!("CARGO")).args(["build", "-q", "-j", "2", "-p", "lean"]).current_dir(&root).status().unwrap();
    assert!(st.success());
    let dir = root.join("target").join(format!("differ-lean-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (o, n) = (fixture("model.old.json"), fixture("model.new.json"));
    let r = diff_models(&o, &n, &opts()).unwrap();
    std::fs::write(dir.join("rows.json"), solver_rows_json(&r).to_string()).unwrap();
    std::fs::write(dir.join("bridge.json"), differ::bridge::bridge_json(&o, &n, &r).to_string()).unwrap();
    let f = root.join("contracts/fixtures/2283/inputs");
    let st = std::process::Command::new(root.join("target/debug/lean-certify"))
        .args(["--old", f.join("model.old.json").to_str().unwrap(), "--new", f.join("model.new.json").to_str().unwrap()])
        .args(["--rows", dir.join("rows.json").to_str().unwrap(), "--bridge", dir.join("bridge.json").to_str().unwrap()])
        .args(["--out", dir.join("out").to_str().unwrap()])
        .status()
        .unwrap();
    assert!(st.success());
    let lean: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("out/lean.json")).unwrap()).unwrap();
    let checked: Vec<_> = lean["rows"].as_array().unwrap().iter().filter(|x| x["state"] == "checked").map(|x| x["row"].clone()).collect();
    assert_eq!(checked, vec![json!("r1"), json!("r2"), json!("r3")]);
    assert!(lean["coverage"].as_array().unwrap().iter().all(|c| c["state"] == "checked"), "{}", lean["coverage"]);
    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------- rows without a location

fn unlocated(version: &str, body: Value) -> Value {
    let mut v = tiny(version, &[("a", "Bool")], body, vec![]);
    v["unit"] = json!("C.M#12");
    v["lines"]["modelled"] = json!([10, 12]);
    v
}

#[test]
fn row_without_location_takes_the_unit_site() {
    let (o, n) = (unlocated("old", report("D", vec![])), unlocated("new", nothing()));
    let r = diff_models(&m(&o), &m(&n), &Options { file: Some("src/C.cs".into()), ..opts() }).unwrap();
    let rows = differ::rows_record(&[&r], 1).unwrap();
    contracts::Schemas::contract().unwrap().validate("rows", &rows, "rows").unwrap();
    assert_eq!(rows["rows"][0]["firstChanged"], json!({"file": "src/C.cs", "line": 12, "endLine": 12}));
    let err = diff_models(&m(&o), &m(&n), &opts()).unwrap_err();
    assert!(err.contains("C.M#12"), "{err}");
}

#[test]
fn identical_rows_need_no_location() {
    let o = unlocated("old", nothing());
    let r = diff_models(&m(&o), &m(&o), &opts()).unwrap();
    assert!(r.rows.iter().all(|x| x.id.is_none()));
    assert_eq!(differ::rows_record(&[&r], 1).unwrap()["rows"], json!([]));
}

#[test]
fn context_line_in_model_lines_is_not_used_with_changed() {
    // 2129 shape: the model's `lines` hold a context line (:20) where the decision sits.
    let mk = |version: &str, body: Value| {
        let mut v = tiny(version, &[("a", "Bool")], body, vec![]);
        v["unit"] = json!("G.Initialize#29");
        v["lines"]["modelled"] = json!([20, 29]);
        v
    };
    let o = mk("old", at(ite(var("a"), report("D", vec![]), nothing()), 20));
    let n = mk("new", at(ite(var("a"), nothing(), nothing()), 20));
    let r = diff_models(&m(&o), &m(&n), &Options { changed: vec![29, 30, 60], ..opts() }).unwrap();
    assert!(r.differing().count() > 0);
    for x in r.differing() {
        let l = x.first_changed.as_ref().unwrap();
        assert_eq!((l.file.as_str(), l.line), (TC, 29));
    }
}

#[test]
fn effect_outcome_names_the_method_not_the_model_id() {
    let p = &[("a", "Bool")];
    let add = || json!({"op": "call", "name": "add_6b17", "args": []});
    let mk = |v: &str, body: Value| {
        let mut x = tiny(v, p, body, vec![]);
        x["opaque"] = json!([opaque("add_6b17", "M:System.Collections.Generic.List`1.Add(`0)", &[], "Unit")]);
        x
    };
    let r = diff_models(&m(&mk("old", nothing())), &m(&mk("new", op("seq", vec![add(), nothing()]))),
        &Options { file: Some("U.cs".into()), ..opts() }).unwrap();
    let x = r.differing().next().unwrap();
    assert_eq!(x.new.text, "calls List.Add");
}

#[test]
fn real_2234_report_before_throw() {
    let rd = |p: &str| -> Value {
        serde_json::from_str(&std::fs::read_to_string(repo().join("crates/translator/tests/data").join(p)).unwrap()).unwrap()
    };
    let (old, new) = (rd("2283.old.cfg.json"), rd("2234.new.cfg.json"));
    let o = |v: &str, site: Vec<u64>| translator::Options {
        unit: "TypeCollector.GetObjectInfo".into(), version: v.into(), region: site, ..Default::default() };
    let mut all = vec![];
    for (os, ns) in [(vec![1022], vec![1023, 1024, 1025, 1026]), (vec![1035], vec![1039, 1040, 1041, 1042])] {
        let mo = m(&translator::lower(&old, &o("old", os.clone())).unwrap());
        let mn = m(&translator::lower(&new, &o("new", ns.clone())).unwrap());
        let r = diff_models(&mo, &mn, &Options { changed: ns.clone(), file: Some("TypeCollector.cs".into()), ..opts() }).unwrap();
        let hit = r.differing().find(|x| x.old.tag == "throws" && x.old.text.contains("ArgumentException") && x.old.text.contains("reports"))
            .expect("dup-add row with report");
        assert!(hit.old.text.contains("MsgPack005, throws ArgumentException"), "{:?}", hit.old);
        assert!(hit.new.tag == "reports" && hit.new.text.contains("MsgPack005"), "{:?}", hit.new);
        assert_eq!(hit.kind, Some("crash-fixed"));
        for g in group(r.differing()) {
            all.push(format!("{os:?} {} {}->{}", g.kind, g.old_text, g.new_text));
        }
    }
    let k = "MsgPack005"; // KeysMustBeUnique, by the worker's diagnosticId
    // new keeps the report and does not add the member: the outcome names the effect it leaves out
    let both = |s: &str, d: &str| [format!("{s} crash-fixed ArgumentException->without {d}.Add"),
        format!("{s} crash-fixed reports {k}, throws ArgumentException->reports {k}, without {d}.Add")];
    let want: Vec<String> = both("[1022]", "intMembers").into_iter().chain(both("[1035]", "stringMembers")).collect();
    assert_eq!(all, want);
}

#[test]
fn real_2234_per_iteration_rows_json() {
    let rd = |p: &str| -> Value {
        serde_json::from_str(&std::fs::read_to_string(repo().join("crates/translator/tests/data").join(p)).unwrap()).unwrap()
    };
    let (old, new) = (rd("2234.old.cfg.json"), rd("2234.new.cfg.json"));
    let o = |v: &str, site: Vec<u64>| translator::Options {
        unit: "TypeCollector.GetObjectInfo".into(), version: v.into(), region: site, per_iteration: true, ..Default::default() };
    let ns = vec![1023, 1024, 1025, 1026];
    let mo = m(&translator::lower(&old, &o("old", vec![1022])).unwrap());
    let mn = m(&translator::lower(&new, &o("new", ns.clone())).unwrap());
    let r = diff_models(&mo, &mn, &Options { changed: ns, file: Some("TypeCollector.cs".into()), ..opts() }).unwrap();
    let rows = differ::rows_record(&[&r], 1).unwrap();
    contracts::Schemas::contract().unwrap().validate("rows", &rows, "rows").unwrap();
    let rs = rows["rows"].as_array().unwrap();
    // the duplicate key with no reporter, and with one (declaration found or not): `member.IntKey` reads back the
    // constructor argument, so `Add` throws exactly when the `ContainsKey` before it held
    assert_eq!(rs.len(), 3);
    assert!(rs.iter().all(|x| x["perIteration"] == json!(true) && x.get("property").is_some()));
    // conditions are in the source's terms; the solver literals stay in `literals`
    for x in rs {
        let c = x["condition"].as_str().unwrap();
        assert!(c.contains("intMembers.ContainsKey(intKey!.Value)") && !c.contains("w_null") && !c.contains("(="), "{c}");
        assert!(x["literals"].as_array().unwrap().iter().any(|l| l.as_str().unwrap().contains("w_contains_")));
    }
}

/// The whole property loop of real 2234 as one closed per-iteration unit (the rule integration uses): the added
/// `continue` is an observable early end, a skipped `Add` is named, and nothing in new throws.
#[test]
fn real_2234_loop_unit() {
    let rd = |p: &str| -> Value {
        serde_json::from_str(&std::fs::read_to_string(repo().join("crates/translator/tests/data").join(p)).unwrap()).unwrap()
    };
    let (old, new) = (rd("2234.old.cfg.json"), rd("2234.new.cfg.json"));
    let o = |v: &str, site: Vec<u64>, parts: Vec<Vec<u64>>| translator::Options {
        unit: "TypeCollector.GetObjectInfo".into(), version: v.into(), region: site, per_iteration: true, exact: true, parts, ..Default::default() };
    let parts = vec![vec![1008], vec![1023, 1024, 1025, 1026], vec![1039, 1040, 1041, 1042]];
    let lo = translator::lower(&old, &o("old", vec![1005, 1022, 1035, 1036], vec![vec![1022], vec![1035]])).unwrap();
    let ln = translator::lower(&new, &o("new", vec![1005, 1008, 1023, 1026, 1039, 1042], parts.clone())).unwrap();
    // every changed line of the loop is modelled, the `continue` among them
    assert_eq!(ln["lines"]["modelled"], json!([1008, 1023, 1024, 1025, 1026, 1039, 1040, 1041, 1042]));
    assert_eq!(ln["lines"]["notModelled"], json!([]));
    let (mo, mn) = (m(&lo), m(&ln));
    let changed: Vec<u64> = parts.concat();
    let r = diff_models(&mo, &mn, &Options { changed, file: Some("TypeCollector.cs".into()), ..opts() }).unwrap();
    assert!(r.rows.iter().all(|x| x.new.tag != "throws"), "new throws on no path of the iteration");
    assert!(r.differing().all(|x| x.per_iteration));
    let mut got: Vec<String> = group(r.differing()).iter().map(|g| format!("{} {}->{}", g.kind, g.old_text, g.new_text)).collect();
    got.sort();
    got.dedup();
    // KeysMustBeUnique and DoNotMixStringAndIntKeys share the id MsgPack005 (worker diagnosticId)
    let (k, mix) = ("MsgPack005", "MsgPack005");
    let skip = "skips the rest of the iteration";
    let mut want = vec![
        "crash-fixed ArgumentException->without intMembers.Add".to_string(),
        "crash-fixed ArgumentException->without stringMembers.Add".to_string(),
        format!("crash-fixed InvalidOperationException->{skip}"),
        format!("crash-fixed reports {k}, throws ArgumentException->reports {k}, without intMembers.Add"),
        format!("crash-fixed reports {k}, throws ArgumentException->reports {k}, without stringMembers.Add"),
        format!("crash-fixed reports {mix}, throws InvalidOperationException->reports {mix}, {skip}"),
        format!("output-changed {mix}->reports {mix}, {skip}"),
        format!("output-changed nothing reported->{skip}"),
    ];
    want.sort();
    want.dedup();
    assert_eq!(got, want);
}

/// New code has no old side (D18): only the paths of the new model that throw are rows.
#[test]
fn new_only_rows_are_the_throwing_paths() {
    let p = &[("a", "Bool")];
    let body = op("if", vec![var("a"), json!({"op": "throw", "name": "System.InvalidOperationException"}), nothing()]);
    let new = m(&tiny("new", p, body, vec![]));
    let old = m(&tiny("old", p, nothing(), vec![]));
    let r = diff_models(&old, &new, &Options { new_only: true, file: Some("U.cs".into()), ..opts() }).unwrap();
    assert_eq!(r.rows.len(), 2);
    let d: Vec<_> = r.differing().collect();
    assert_eq!(d.len(), 1);
    assert_eq!((d[0].kind, d[0].property, d[0].old.text.as_str(), d[0].new.text.as_str()),
        (Some("crash"), Some("no-exception"), "not present", "InvalidOperationException"));
}

#[test]
fn real_2283_library_facts() {
    let d = repo().join("crates/lean/tests/data/2283-real");
    let rd = |f: &str| Model::from_str(&std::fs::read_to_string(d.join(f)).unwrap()).unwrap();
    let mut used = vec![];
    for u in ["985", "1077"] {
        let r = diff_models(&rd(&format!("model.old.{u}.json")), &rd(&format!("model.new.{u}.json")), &opts()).unwrap();
        let fs = differ::library_facts(&r);
        let keys: Vec<&str> = fs.iter().map(|(f, _)| f.key).collect();
                assert!(fs.iter().all(|(f, rows)| !rows.is_empty() && !f.text.is_empty()));
        used.push(keys.join(","));
        let detail = differ::detail_json(&r);
        assert_eq!(detail["libraryFacts"].as_array().unwrap().len(), fs.len());
    }
    let all = "length-non-negative,first-or-default,isinsource-null-false,dsr-elements-non-null,getsyntax-non-null";
    assert_eq!(used, [all, all]);
}

#[test]
fn real_2283_scope_library_facts() {
    let d = repo().join("crates/lean/tests/data/2283-real");
    let rd = |f: &str| Model::from_str(&std::fs::read_to_string(d.join(f)).unwrap()).unwrap();
    let r = diff_models(&rd("model.old.985.json"), &rd("model.new.985.json"), &opts()).unwrap();
    let mut scope = json!({"changedLines": {"production": 29, "test": 0, "docs": 0, "snapshot": 0, "generated": 0},
        "modelledLines": 29, "notModelled": [],
        "assumptions": [{"contract": "index-empty", "symbol": "P:System.Collections.Immutable.ImmutableArray`1.Item(System.Int32)", "rows": []}]});
    differ::fill_scope(&mut scope, &[&r]);
    differ::fill_scope(&mut scope, &[&r]); // idempotent
    contracts::Schemas::contract().unwrap().validate("scope", &scope, "scope").unwrap();
    let a = scope["assumptions"].as_array().unwrap();
    let lf: Vec<&Value> = a.iter().filter(|x| x["kind"] == "library-fact").collect();
    assert_eq!(lf.len(), differ::library_facts(&r).len());
    for x in &lf {
        let f = differ::exec::fact(x["key"].as_str().unwrap()).unwrap();
        assert_eq!(x["text"], f.text);
        assert!(!x["rows"].as_array().unwrap().is_empty() && f.smt.starts_with('('));
    }
    assert!(lf.iter().any(|x| x["key"] == "first-or-default"));
    assert!(!a[0]["rows"].as_array().unwrap().is_empty());
}

#[test]
fn real_2283_groups() {
    let d = repo().join("crates/lean/tests/data/2283-real");
    let rd = |f: &str| Model::from_str(&std::fs::read_to_string(d.join(f)).unwrap()).unwrap();
    let mut all = vec![];
    for u in ["985", "1077"] {
        let r = diff_models(&rd(&format!("model.old.{u}.json")), &rd(&format!("model.new.{u}.json")), &opts()).unwrap();
        // the DeclaringSyntaxReferences non-null-element fact leaves no "non-empty but first is null" row
        assert!(!r.differing().any(|x| x.literals.iter().any(|l| l.starts_with("(not (<= (w_len"))
            && x.literals.iter().any(|l| l.starts_with("(= (firstOrDefault") && l.ends_with("w_null)"))));
        assert!(r.rows.iter().all(|x| !x.old.text.contains("_6b17") && !x.new.text.contains("_6b17")));
        for g in group(r.differing()) {
            all.push(format!("{u} {} {}->{}", g.kind, g.old_text, g.new_text));
        }
    }
    assert_eq!(all, [
        "985 crash-fixed IndexOutOfRangeException->nothing reported",
        "985 crash-fixed InvalidCastException->nothing reported",
        "985 silent-failure MsgPack005->nothing reported",
        "1077 silent-failure MsgPack016->nothing reported",
        "1077 crash-fixed IndexOutOfRangeException->nothing reported",
        "1077 crash-fixed IndexOutOfRangeException->calls deferredDiagnosticsForNonPublicMembers.Add",
        "1077 silent-failure MsgPack004->nothing reported"]);
}

// ---------------------------------------------------------------- real 2283 worker replies

fn prod_lines(side: &str) -> Vec<Vec<u64>> {
    let p: Value = serde_json::from_str(&std::fs::read_to_string(repo().join("contracts/fixtures/2283/parts.json")).unwrap()).unwrap();
    let key = if side == "old" { "deletedLines" } else { "addedLines" };
    p["parts"].as_array().unwrap().iter().filter(|x| x["category"] == "production")
        .map(|x| x[key].as_array().unwrap().iter().map(|l| l.as_u64().unwrap()).collect::<Vec<u64>>())
        .filter(|l| !l.is_empty()).collect()
}

fn real_model(side: &str, site: u64) -> Model {
    let reply: Value = serde_json::from_str(&std::fs::read_to_string(
        repo().join(format!("crates/translator/tests/data/2283.{side}.cfg.json"))).unwrap()).unwrap();
    let tc = "MessagePack.SourceGenerator.CodeAnalysis.TypeCollector";
    let changed: std::collections::BTreeSet<u64> = prod_lines(side).into_iter().flatten().collect();
    let mut o = translator::Options {
        unit: "TypeCollector.GetObjectInfo".into(), unit_id: Some(format!("TypeCollector.GetObjectInfo#{site}")),
        version: side.into(), parts: prod_lines(side), region: vec![site], ..Default::default() };
    o.inline.insert(format!("M:{tc}.GetPropertyIdentifierLocation(Microsoft.CodeAnalysis.IPropertySymbol)"));
    o.inline.insert(format!("M:{tc}.ReportIfLocatedInCompilation(System.Action{{Microsoft.CodeAnalysis.Diagnostic}},Microsoft.CodeAnalysis.Diagnostic)"));
    o.delegates = translator::derive_delegates(&reply["result"], &changed);
    m(&translator::lower(&reply, &o).unwrap())
}

#[test]
fn real_2283_first_changed_is_a_changed_line() {
    let added: Vec<u64> = prod_lines("new").into_iter().flatten().collect();
    for site in [985, 1077] {
        let r = diff_models(&real_model("old", site), &real_model("new", site), &opts()).unwrap();
        assert!(r.differing().count() > 0, "#{site}");
        for x in r.differing() {
            let l = x.first_changed.as_ref().unwrap();
            assert!(added.contains(&l.line), "#{site} {:?} {:?} -> {:?} at {}", x.id, x.old, x.new, l.line);
        }
    }
}

#[test]
fn cli_writes_valid_rows() {
    let dir = std::env::temp_dir().join(format!("differ-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = repo().join("contracts/fixtures/2283/inputs");
    let st = std::process::Command::new(env!("CARGO_BIN_EXE_differ"))
        .args(["--old", f.join("model.old.json").to_str().unwrap(), "--new", f.join("model.new.json").to_str().unwrap()])
        .args(["--out", dir.join("solver-rows.json").to_str().unwrap(), "--detail", dir.join("detail.json").to_str().unwrap()])
        .status()
        .unwrap();
    assert!(st.success());
    let v: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("solver-rows.json")).unwrap()).unwrap();
    contracts::Schemas::contract().unwrap().validate("solver-rows", &v, "solver-rows").unwrap();
    let d: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("detail.json")).unwrap()).unwrap();
    assert_eq!(d["groups"].as_array().unwrap().len(), 3);
    std::fs::remove_dir_all(&dir).ok();
}
