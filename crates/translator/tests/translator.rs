mod fixtures;

use contracts::Schemas;
use fixtures::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;
use translator::{derive_delegates, lower, scope, Line, Options};

fn contracts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts")
}

fn read(p: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(contracts_dir().join(p)).unwrap()).unwrap()
}

fn schemas() -> Schemas {
    Schemas::contract().unwrap()
}

fn valid_model(m: &Value) {
    if let Err(f) = schemas().validate("model", m, "") {
        panic!("model invalid: {} {}\n{}", f.path, f.message, serde_json::to_string_pretty(m).unwrap());
    }
}

fn all<'a>(e: &'a Value, out: &mut Vec<&'a Value>) {
    out.push(e);
    for a in e.get("args").and_then(Value::as_array).into_iter().flatten() {
        all(a, out);
    }
}

fn nodes(m: &Value) -> Vec<&Value> {
    let mut v = Vec::new();
    all(&m["body"], &mut v);
    v
}

/// Production parts of the 2283 fixture in TypeCollector.cs, as line lists of one side.
fn parts(side: &str) -> Vec<Vec<u64>> {
    let p = read("fixtures/2283/parts.json");
    let key = if side == "old" { "deletedLines" } else { "addedLines" };
    p["parts"].as_array().unwrap().iter()
        .filter(|x| x["category"] == "production")
        .map(|x| x[key].as_array().unwrap().iter().map(|l| l.as_u64().unwrap()).collect::<Vec<u64>>())
        .filter(|l| !l.is_empty())
        .collect()
}

fn changed(side: &str) -> BTreeSet<u64> {
    parts(side).into_iter().flatten().collect()
}

fn opts(version: &str, reply: &Value) -> Options {
    let mut o = Options { unit: "TypeCollector.GetObjectInfo".into(), version: version.into(), parts: parts(version), ..Default::default() };
    o.inline.insert(format!("M:{TC}.GetPropertyIdentifierLocation(Microsoft.CodeAnalysis.IPropertySymbol)"));
    o.inline.insert(format!("M:{TC}.ReportIfLocatedInCompilation(System.Action{{Microsoft.CodeAnalysis.Diagnostic}},Microsoft.CodeAnalysis.Diagnostic)"));
    o.delegates = derive_delegates(&reply["result"], &changed(version));
    o
}

#[test]
fn fixtures_match_worker_schema() {
    let s = schemas();
    for r in [old_reply(), new_reply()] {
        s.validate("worker-reply", &r, "").map_err(|f| format!("{} {}", f.path, f.message)).unwrap();
    }
}

#[test]
fn contract_fixture_lowers() {
    let r = read("fixtures/2283/inputs/worker-reply.cfg.json");
    let o = Options { unit: "TypeCollector.GetPropertyIdentifierLocation".into(), version: "new".into(), parts: vec![vec![1434]], ..Default::default() };
    let m = lower(&r, &o).unwrap();
    valid_model(&m);
    assert_eq!(m["lines"]["notModelled"], json!([]));
    assert_eq!(m["lines"]["modelled"], json!([1434]));
    assert!(nodes(&m).iter().any(|n| n["op"] == "isnull"));
}

#[test]
fn old_lowers_both_sites_with_index_contracts() {
    let r = old_reply();
    let m = lower(&r, &opts("old", &r)).unwrap();
    valid_model(&m);
    assert_eq!(m["lines"]["notModelled"], json!([]));
    assert_eq!(m["lines"]["modelled"], json!([985, 1077]));
    let idx: Vec<&Value> = nodes(&m).into_iter().filter(|n| n["op"] == "index").collect();
    assert!(idx.len() >= 2 && idx.iter().all(|n| n["contract"] == "index-empty"));
    let uses: Vec<&Value> = m["contracts"].as_array().unwrap().iter().filter(|c| c["contract"] == "index-empty").collect();
    assert_eq!(uses.len(), 2, "one counted assumption per use (:985, :1077)");
    assert_eq!(uses[0]["symbol"], "P:System.Collections.Immutable.ImmutableArray`1.Item(System.Int32)");
    assert!(m["contracts"].as_array().unwrap().iter().any(|c| c["contract"] == "failed-cast"));
    let reports: Vec<String> = nodes(&m).iter().filter(|n| n["op"] == "report").map(|n| n["name"].as_str().unwrap().to_string()).collect();
    assert!(reports.iter().any(|r| r.ends_with("BothStringAndIntKeyAreNull")));
    assert!(reports.iter().any(|r| r.ends_with("MemberNeedsKey")));
}

#[test]
fn disabled_contract_removes_index() {
    let r = old_reply();
    let mut o = opts("old", &r);
    o.disabled.insert("index-empty".into());
    let m = lower(&r, &o).unwrap();
    assert!(!nodes(&m).iter().any(|n| n["contract"] == "index-empty"));
    assert!(!m["contracts"].as_array().unwrap().iter().any(|c| c["contract"] == "index-empty"));
}

#[test]
fn new_inlines_helper_and_filter() {
    let r = new_reply();
    let o = opts("new", &r);
    assert_eq!(o.delegates.get(&format!("F:{TC}.reportDiagnostic")).map(String::as_str), Some("lambda@239"));
    let m = lower(&r, &o).unwrap();
    valid_model(&m);
    assert_eq!(m["lines"]["notModelled"], json!([]));
    let syms: Vec<&str> = m["opaque"].as_array().unwrap().iter().map(|x| x["symbol"].as_str().unwrap()).collect();
    assert!(!syms.iter().any(|s| s.contains("GetPropertyIdentifierLocation") || s.contains("ReportIfLocatedInCompilation")), "{syms:?}");
    assert!(syms.iter().any(|s| s.contains("ContainsSyntaxTree")), "filter inlined at every Invoke");
    assert!(syms.iter().any(|s| s.contains("IsInSource")));
    assert!(!nodes(&m).iter().any(|n| n["op"] == "index"));
    assert!(m["contracts"].as_array().unwrap().iter().all(|c| c["contract"] != "index-empty"));
    // every report is guarded by the filter's ContainsSyntaxTree test
    let cst = m["opaque"].as_array().unwrap().iter().find(|x| x["symbol"].as_str().unwrap().contains("ContainsSyntaxTree")).unwrap()["name"].clone();
    fn guarded(e: &Value, cst: &Value, under: bool) -> bool {
        match e["op"].as_str() {
            Some("report") => under,
            Some("if") => {
                let a = e["args"].as_array().unwrap();
                let c = a[0]["op"] == "call" && a[0]["name"] == *cst || a[0]["op"] == "var";
                guarded(&a[1], cst, under || c) && guarded(&a[2], cst, under)
            }
            _ => e.get("args").and_then(Value::as_array).map(|a| a.iter().all(|x| guarded(x, cst, under))).unwrap_or(true),
        }
    }
    assert!(nodes(&m).iter().filter(|n| n["op"] == "report").count() >= 2);
    assert!(guarded(&m["body"], &cst, false));
    let modelled: Vec<u64> = m["lines"]["modelled"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap()).collect();
    assert!(modelled.contains(&239) && modelled.contains(&985) && modelled.contains(&1077) && modelled.contains(&1434) && modelled.contains(&1445));
}

#[test]
fn without_delegate_reports_go_straight_to_sink() {
    let r = new_reply();
    let mut o = opts("new", &r);
    o.delegates.clear();
    let m = lower(&r, &o).unwrap();
    let syms: Vec<&str> = m["opaque"].as_array().unwrap().iter().map(|x| x["symbol"].as_str().unwrap()).collect();
    assert!(!syms.iter().any(|s| s.contains("ContainsSyntaxTree")));
}

#[test]
fn unsupported_operation_is_not_modelled() {
    let g = graph("C.M", 10, 14, &[("x", "System.Int32")], vec![
        entry(),
        block(1, vec![op("ExpressionStatement", Some("System.Void"), None, "await Task.Delay(x);", 12, vec![
            op("Await", Some("System.Void"), None, "await Task.Delay(x)", 12, vec![])])], None, false, Some(2), None),
        exit(2)], vec![]);
    let r = reply(vec![g]);
    let o = Options { unit: "C.M".into(), version: "new".into(), parts: vec![vec![11], vec![12]], ..Default::default() };
    let m = lower(&r, &o).unwrap();
    valid_model(&m);
    let nm = m["lines"]["notModelled"].as_array().unwrap();
    assert_eq!(nm.len(), 1);
    assert!(nm[0]["reason"].as_str().unwrap().contains("Await"));
    assert_eq!(nm[0]["location"]["line"], 12);
    assert_eq!(m["lines"]["modelled"], json!([11]), "part on the unsupported line is not modelled; the other part is");
    let sc = scope(&json!({"production": 2, "test": 0, "docs": 0, "snapshot": 0, "generated": 0}),
        &[Line { side: "new".into(), file: F.into(), line: 11 }, Line { side: "new".into(), file: F.into(), line: 12 }], &[(F.into(), m)]);
    assert_eq!(sc["modelledLines"], 1);
    assert!(sc["notModelled"].as_array().unwrap().iter().any(|n| n["reason"].as_str().unwrap().contains("Await")));
}

#[test]
fn loop_is_not_modelled() {
    let g = graph("C.L", 20, 25, &[("b", "System.Boolean")], vec![
        entry(),
        block(1, vec![], Some(op("ParameterReference", Some("System.Boolean"), None, "b", 21, vec![])), true, Some(2), Some(1)),
        exit(2)], vec![]);
    let m = lower(&reply(vec![g]), &Options { unit: "C.L".into(), version: "old".into(), parts: vec![vec![21]], ..Default::default() }).unwrap();
    valid_model(&m);
    assert!(m["lines"]["notModelled"].as_array().unwrap().iter().any(|n| n["reason"] == "loop"));
    assert_eq!(m["lines"]["modelled"], json!([]));
}

#[test]
fn contracts_from_symbols() {
    let g = graph("C.K", 30, 34, &[("d", "System.Collections.Generic.Dictionary<System.Int32,System.String>"), ("n", "System.Int32?")], vec![
        entry(),
        block(1, vec![op("ExpressionStatement", Some("System.Void"), None, "d.Add(n.Value, \"a\");", 31, vec![
            op("Invocation", Some("System.Void"), Some("M:System.Collections.Generic.Dictionary`2.Add(`0,`1)"), "d.Add(n.Value, \"a\")", 31, vec![
                op("ParameterReference", Some("System.Collections.Generic.Dictionary<System.Int32,System.String>"), None, "d", 31, vec![]),
                op("PropertyReference", Some("System.Int32"), Some("P:System.Nullable`1.Value"), "n.Value", 31, vec![
                    op("ParameterReference", Some("System.Int32?"), None, "n", 31, vec![])]),
                op("Literal", Some("System.String"), None, "\"a\"", 31, vec![])])])], None, false, Some(2), None),
        exit(2)], vec![]);
    let m = lower(&reply(vec![g]), &Options { unit: "C.K".into(), version: "new".into(), ..Default::default() }).unwrap();
    valid_model(&m);
    let cs: Vec<&str> = m["contracts"].as_array().unwrap().iter().map(|c| c["contract"].as_str().unwrap()).collect();
    assert!(cs.contains(&"dictionary-duplicate-add") && cs.contains(&"nullable-value-null"), "{cs:?}");
}

#[test]
fn null_receiver_only_on_maybe_null() {
    let g = graph("C.N", 40, 42, &[("o", "System.Object")], vec![
        entry(),
        block(1, vec![op("FlowCapture", Some("Microsoft.CodeAnalysis.SyntaxNode"), None, "s", 41, vec![
            op("Conversion", Some("Microsoft.CodeAnalysis.SyntaxNode"), None, "o as SyntaxNode", 41, vec![
                op("ParameterReference", Some("System.Object"), None, "o", 41, vec![])])])], None, false, Some(2), None),
        block(2, vec![op("ExpressionStatement", Some("System.Void"), None, "s.GetLocation();", 41, vec![
            op("Invocation", Some("Microsoft.CodeAnalysis.Location"), Some("M:Microsoft.CodeAnalysis.SyntaxNode.GetLocation"), "s.GetLocation()", 41, vec![
                op("FlowCaptureReference", Some("Microsoft.CodeAnalysis.SyntaxNode"), None, "s", 41, vec![])])])], None, false, Some(3), None),
        exit(3)], vec![]);
    let m = lower(&reply(vec![g]), &Options { unit: "C.N".into(), version: "new".into(), ..Default::default() }).unwrap();
    valid_model(&m);
    assert!(m["contracts"].as_array().unwrap().iter().any(|c| c["contract"] == "null-receiver"));
    // the ?. pattern of the new :1077 site null-checks first, so no null-receiver there
    let r = new_reply();
    let n = lower(&r, &opts("new", &r)).unwrap();
    assert!(!n["contracts"].as_array().unwrap().iter().any(|c| c["contract"] == "null-receiver"), "{}", n["contracts"]);
}

#[test]
fn scope_2283() {
    let (ro, rn) = (old_reply(), new_reply());
    let mo = lower(&ro, &opts("old", &ro)).unwrap();
    let mn = lower(&rn, &opts("new", &rn)).unwrap();
    let mut prod = Vec::new();
    for side in ["old", "new"] {
        for l in changed(side) {
            prod.push(Line { side: side.into(), file: F.into(), line: l });
        }
    }
    assert_eq!(prod.len(), 29);
    let counts = read("fixtures/2283/scope.json")["changedLines"].clone();
    let sc = scope(&counts, &prod, &[(F.into(), mo), (F.into(), mn)]);
    schemas().validate("scope", &sc, "").map_err(|f| format!("{} {}", f.path, f.message)).unwrap();
    assert_eq!(sc["changedLines"]["production"], 29);
    let modelled = sc["modelledLines"].as_u64().unwrap();
    let nm_lines: u64 = sc["notModelled"].as_array().unwrap().iter()
        .map(|n| n["location"]["endLine"].as_u64().unwrap() - n["location"]["line"].as_u64().unwrap() + 1).sum();
    assert_eq!(modelled + nm_lines, 29, "{sc}");
    // old :239 (`this.reportDiagnostic = reportAnalyzerDiagnostic`) is in no compared unit
    assert_eq!(modelled, 28);
    assert!(sc["assumptions"].as_array().unwrap().iter().any(|a| a["contract"] == "index-empty"));
}

fn real(v: &str) -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/data/2283.{v}.cfg.json"));
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn real_opts(version: &str, reply: &Value, site: u64) -> Options {
    let mut o = opts(version, reply);
    o.region = vec![site];
    // as a caller derives it from parts.json: the `member` of every member part
    o.inline = ["GetPropertyIdentifierLocation".to_string(), "ReportIfLocatedInCompilation".to_string()].into();
    o
}

fn opaque_symbols(m: &Value) -> Vec<String> {
    m["opaque"].as_array().unwrap().iter().map(|x| x["symbol"].as_str().unwrap().to_string()).collect()
}

#[test]
fn real_worker_sites_lower() {
    let (ro, rn) = (real("old"), real("new"));
    let so = schemas();
    for r in [&ro, &rn] {
        so.validate("worker-reply", r, "").map_err(|f| format!("{} {}", f.path, f.message)).unwrap();
    }
    let on = opts("new", &rn);
    assert_eq!(on.delegates.get(&format!("F:{TC}.reportDiagnostic")).map(String::as_str), Some("TypeCollector..ctor/lambda@239:75"));
    assert!(opts("old", &ro).delegates.is_empty());
    for site in [985u64, 1077] {
        let mo = lower(&ro, &real_opts("old", &ro, site)).unwrap();
        let mn = lower(&rn, &real_opts("new", &rn, site)).unwrap();
        for m in [&mo, &mn] {
            valid_model(m);
            assert_eq!(m["unit"], format!("TypeCollector.GetObjectInfo#{site}"));
            assert_eq!(m["lines"]["notModelled"], json!([]), "{site}");
            let ps: Vec<&str> = m["params"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
            assert!(ps.contains(&"item"), "{ps:?}");
            // calls carry their arguments
            assert!(nodes(m).iter().filter(|n| n["op"] == "call").any(|n| !n["args"].as_array().unwrap().is_empty()));
            assert!(m["opaque"].as_array().unwrap().iter().any(|o| o["symbol"] == "P:Microsoft.CodeAnalysis.ISymbol.DeclaringSyntaxReferences" && o["args"].as_array().unwrap().len() == 1));
        }
        // old: [0] index contract at the site
        assert!(mo["contracts"].as_array().unwrap().iter().any(|c| c["contract"] == "index-empty"), "{site}");
        assert!(nodes(&mo).iter().any(|n| n["op"] == "index" && n["contract"] == "index-empty"));
        // new: no index, helper and wrapper inlined, filter tests present
        assert!(!nodes(&mn).iter().any(|n| n["op"] == "index"));
        let syms = opaque_symbols(&mn);
        assert!(!syms.iter().any(|s| s.contains("GetPropertyIdentifierLocation") || s.contains("ReportIfLocatedInCompilation")), "{syms:?}");
        assert!(syms.iter().any(|s| s.contains("ContainsSyntaxTree")) && syms.iter().any(|s| s.contains("Location.IsInSource")));
        assert!(syms.iter().any(|s| s.contains("FirstOrDefault")));
        assert!(mn["lines"]["modelled"].as_array().unwrap().contains(&json!(239)), "wrapper line 239 modelled via inlining");
        // shared functions are named alike in both versions
        let names = |m: &Value| m["opaque"].as_array().unwrap().iter().map(|o| (format!("{}/{}", o["symbol"].as_str().unwrap(), o["args"].as_array().unwrap().len()), o["name"].as_str().unwrap().to_string())).collect::<Vec<_>>();
        for (sy, n) in names(&mo) {
            if let Some((_, n2)) = names(&mn).into_iter().find(|(s2, _)| *s2 == sy) {
                assert_eq!(n, n2);
            }
        }
    }
    // the :985 old site also uses a hard cast
    let m985 = lower(&ro, &real_opts("old", &ro, 985)).unwrap();
    assert!(m985["contracts"].as_array().unwrap().iter().any(|c| c["contract"] == "failed-cast"));
    // :1077 widens to the Invoke at :1080 that reports the local `diagnostic`
    let m1077 = lower(&rn, &real_opts("new", &rn, 1077)).unwrap();
    assert!(nodes(&m1077).iter().any(|n| n["op"] == "report" && (1444..=1450).contains(&n["location"]["line"].as_u64().unwrap_or(0))));
}

/// On some path of `e`: a report named `rep` and, after it, a call with contract `c`.
fn report_then_contract(e: &Value, rep: &str, c: &str, seen_report: bool) -> bool {
    match e["op"].as_str() {
        Some("seq") => {
            let a = e["args"].as_array().unwrap();
            let first_is_rep = a[0]["op"] == "report" && a[0]["name"].as_str().unwrap_or("").ends_with(rep);
            report_then_contract(&a[0], rep, c, seen_report) || report_then_contract(&a[1], rep, c, seen_report || first_is_rep)
        }
        Some("if") => e["args"].as_array().unwrap().iter().any(|x| report_then_contract(x, rep, c, seen_report)),
        Some("let") => {
            let a = e["args"].as_array().unwrap();
            report_then_contract(&a[0], rep, c, seen_report) || report_then_contract(&a[1], rep, c, seen_report)
        }
        _ => {
            let here = seen_report && e.get("contract").and_then(Value::as_str) == Some(c);
            here || e.get("args").and_then(Value::as_array).map(|a| a.iter().any(|x| report_then_contract(x, rep, c, seen_report))).unwrap_or(false)
        }
    }
}

#[test]
fn real_2234_unit_keeps_msgpack005_report() {
    let old = real("old"); // 2234's base is the same commit as 2283's
    let new = {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/2234.new.cfg.json");
        serde_json::from_str::<Value>(&std::fs::read_to_string(p).unwrap()).unwrap()
    };
    schemas().validate("worker-reply", &new, "").map_err(|f| format!("{} {}", f.path, f.message)).unwrap();
    let o = |v: &str, site: Vec<u64>| Options { unit: "TypeCollector.GetObjectInfo".into(), version: v.into(), region: site, ..Default::default() };
    for (os, ns) in [(vec![1022], vec![1023, 1024, 1025, 1026]), (vec![1035], vec![1039, 1040, 1041, 1042])] {
        let mo = lower(&old, &o("old", os.clone())).unwrap();
        let mn = lower(&new, &o("new", ns.clone())).unwrap();
        valid_model(&mo);
        valid_model(&mn);
        // old: KeysMustBeUnique (MsgPack005) is reported, then Dictionary.Add throws on the duplicate key
        assert!(report_then_contract(&mo["body"], "MsgPack005", "dictionary-duplicate-add", false), "{os:?}");
        // new: the report is still in the unit; Add is guarded by ContainsKey
        assert!(nodes(&mn).iter().any(|n| n["op"] == "report" && n["name"] == "MsgPack005"));
        assert!(nodes(&mn).iter().any(|n| n["op"] == "report" && n["descriptor"].as_str().unwrap_or("").ends_with(".KeysMustBeUnique")));
        assert_eq!(mo["lines"]["notModelled"], json!([]));
        assert_eq!(mn["lines"]["notModelled"], json!([]));
    }
}

#[test]
fn real_2234_per_iteration_unit() {
    let data = |n: &str| -> Value {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/data/{n}"));
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    };
    let (old, new) = (data("2234.old.cfg.json"), data("2234.new.cfg.json"));
    let o = |v: &str, site: Vec<u64>| Options { unit: "TypeCollector.GetObjectInfo".into(), version: v.into(), region: site, per_iteration: true, ..Default::default() };
    for (os, ns) in [(vec![1022], vec![1023, 1024, 1025, 1026]), (vec![1035], vec![1039, 1040, 1041, 1042])] {
        let mo = lower(&old, &o("old", os.clone())).unwrap();
        let mn = lower(&new, &o("new", ns)).unwrap();
        for m in [&mo, &mn] {
            valid_model(m);
            // one arbitrary iteration of `foreach (IPropertySymbol item in instanceMembers.OfType<IPropertySymbol>())`
            assert_eq!(m["body"]["op"], "foreach");
            assert_eq!(m["body"]["name"], "item");
            assert_eq!(m["body"]["location"]["line"], 886);
            assert!(m["body"]["args"][0]["args"][0]["name"] == "instanceMembers");
            let ps: Vec<&str> = m["params"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
            assert!(!ps.contains(&"item") && ps.contains(&"intMembers") || ps.contains(&"stringMembers"), "{ps:?}");
            assert_eq!(m["lines"]["notModelled"], json!([]));
        }
        assert!(report_then_contract(&mo["body"]["args"][1], "MsgPack005", "dictionary-duplicate-add", false), "{os:?}");
    }
    // without a loop around the site, per-iteration lowering refuses rather than guessing
    let r = real("new");
    let mut x = real_opts("new", &r, 239);
    x.unit = "TypeCollector..ctor".into();
    x.per_iteration = true;
    assert!(lower(&r, &x).is_err());
}

#[test]
fn descriptor_id_used_when_worker_gives_it() {
    let mut r = real("old");
    // simulate the requested worker field `diagnosticId` on DiagnosticDescriptor field references
    fn tag(v: &mut Value) {
        if v["kind"] == "FieldReference" && v["type"] == "Microsoft.CodeAnalysis.DiagnosticDescriptor" {
            v["diagnosticId"] = json!("MsgPack004");
        }
        if let Some(a) = v.as_array_mut() {
            a.iter_mut().for_each(tag);
        } else if let Some(o) = v.as_object_mut() {
            o.values_mut().for_each(tag);
        }
    }
    tag(&mut r);
    let m = lower(&r, &real_opts("old", &r, 985)).unwrap();
    let rep: Vec<&Value> = nodes(&m).into_iter().filter(|n| n["op"] == "report").collect();
    assert!(!rep.is_empty() && rep.iter().all(|n| n["name"] == "MsgPack004" && n["descriptor"].as_str().unwrap().ends_with("BothStringAndIntKeyAreNull")));
    let plain = lower(&real("old"), &real_opts("old", &real("old"), 985)).unwrap();
    // the saved real reply carries the real id
    assert!(nodes(&plain).iter().any(|n| n["op"] == "report" && n["name"] == "MsgPack005" && n["descriptor"].as_str().unwrap().ends_with("BothStringAndIntKeyAreNull")));
}

/// Worker fields on the saved real replies: a positional record property reads back its constructor argument
/// (`member.IntKey` at 2234 :1023), and a lambda is named by its text hash, not its position.
#[test]
fn real_worker_fields_are_used() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/2234.new.cfg.json");
    let new: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    let o = Options { unit: "TypeCollector.GetObjectInfo".into(), version: "new".into(), region: vec![1023, 1024, 1025, 1026], ..Default::default() };
    let m = lower(&new, &o).unwrap();
    assert!(!m["opaque"].as_array().unwrap().iter().any(|x| x["symbol"].as_str().unwrap().ends_with("MemberSerializationInfo.IntKey")), "IntKey is read back, not called");
    // without the worker's mark the property is an opaque call again
    let mut bare = new.clone();
    fn strip(v: &mut Value) {
        if let Some(o) = v.as_object_mut() {
            o.remove("positionalParameter");
            o.values_mut().for_each(strip);
        } else if let Some(a) = v.as_array_mut() {
            a.iter_mut().for_each(strip);
        }
    }
    strip(&mut bare);
    let mb = lower(&bare, &o).unwrap();
    assert!(mb["opaque"].as_array().unwrap().iter().any(|x| x["symbol"].as_str().unwrap().ends_with("MemberSerializationInfo.IntKey")));
    // lambdas: `delegate:<first 16 hex of the hash>`
    let r = real("new");
    let mut x = real_opts("new", &r, 239);
    x.unit = "TypeCollector..ctor".into();
    let mc = lower(&r, &x).unwrap();
    for d in mc["opaque"].as_array().unwrap().iter().filter_map(|x| x["symbol"].as_str()).filter_map(|s| s.strip_prefix("delegate:")) {
        assert!(d.len() == 16 && d.chars().all(|c| c.is_ascii_hexdigit()), "{d}");
    }
}

#[test]
fn real_descriptor_per_site() {
    let names = |m: &Value| -> BTreeSet<String> {
        nodes(m).into_iter().filter(|n| n["op"] == "report").map(|n| n["descriptor"].as_str().unwrap().rsplit('.').next().unwrap().to_string()).collect()
    };
    for v in ["old", "new"] {
        let r = real(v);
        let m985 = lower(&r, &real_opts(v, &r, 985)).unwrap();
        assert_eq!(names(&m985), ["BothStringAndIntKeyAreNull".to_string()].into());
        // :1077 builds `diagnostic` from one of two Create calls sharing a capture; both reach the Invoke at :1080
        let m1077 = lower(&r, &real_opts(v, &r, 1077)).unwrap();
        assert_eq!(names(&m1077), ["AOTDerivedKeyAttribute".to_string(), "MemberNeedsKey".to_string()].into(), "{v}");
    }
}

#[test]
fn same_version_gives_same_model() {
    let r = old_reply();
    assert_eq!(lower(&r, &opts("old", &r)).unwrap(), lower(&r, &opts("old", &r)).unwrap());
}

/// The closed per-iteration region of real 2234's property loop (D19): the added `continue` at :1008 leaves the
/// iteration beside the fall-through, so the region grows to one entry (`if (searchFirst)` at :998) and stops
/// before the unchanged tail (:1045); the early end is an effect and the
/// `continue` line is modelled.
#[test]
fn real_2234_closed_loop_region() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/2234.new.cfg.json");
    let new: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    let g = &translator::cfg_result(&new).unwrap()["methods"][0];
    let mut r = translator::region_blocks_opt(g, &[1005, 1008, 1023, 1026, 1039, 1042], false);
    let lp = translator::enclosing_foreach(g, &r).unwrap();
    let c = translator::close_region(g, &mut r, &lp);
    assert_eq!((r.iter().next().copied(), r.iter().next_back().copied(), c.tail), (Some(181), Some(214), Some(215)));
    let m = lower(&new, &Options { unit: "TypeCollector.GetObjectInfo".into(), version: "new".into(), per_iteration: true, exact: true,
        region: vec![1005, 1008, 1023, 1026, 1039, 1042], parts: vec![vec![1008]], ..Default::default() }).unwrap();
    Schemas::contract().unwrap().validate("model", &m, "model").unwrap();
    assert_eq!(m["lines"]["modelled"], json!([1008]));
    // (the trimmed reply has no blocks after the loop header, so no local is seen as carried here; the full reply
    // carries searchFirst, isIntKey and hiddenIntKey)
    assert!(m["opaque"].as_array().unwrap().iter().any(|o| o["symbol"] == "region:skip"));
    // a string the code tests for null enters as a nullable input
    assert!(m["params"].as_array().unwrap().contains(&json!({"name": "stringKey", "sort": "OptString"})));
}
