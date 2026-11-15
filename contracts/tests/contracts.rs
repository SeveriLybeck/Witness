use contracts::{recording_key, validate_dir, Options, Schemas};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn fx(n: &str) -> PathBuf {
    root().join("fixtures").join(n)
}
fn rec(n: &str, r: &str) -> Value {
    contracts::read_json(&fx(n).join(format!("{r}.json"))).unwrap()
}
fn items<'a>(v: &'a Value, k: &str) -> &'a Vec<Value> {
    v[k].as_array().unwrap()
}
const SESSION: &str = "sessions/32258fdbee4bcd653c79f51b4376b5103c6f4d27-9df83c38409c8bfad0517aebffe48211eda4a5b4/reviewer-a";

fn copy_dir(from: &Path, to: &Path) {
    let _ = fs::remove_dir_all(to);
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        if e.path().is_file() {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}
fn tmp(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}
fn write(p: &Path, v: &Value) {
    fs::write(p, serde_json::to_string_pretty(v).unwrap()).unwrap();
}

#[test]
fn all_fixtures_pass() {
    for d in ["2283", "2234", "2297", "2129", "recordings", SESSION] {
        validate_dir(&fx(d), &Options::default()).unwrap_or_else(|f| panic!("{d}: {f}"));
    }
}

#[test]
fn locations_exist_in_the_sample_repository() {
    // runs only where the sandbox clone exists (work/sb/repo, made by contracts/spike/tools/sb)
    let repo = root().join("../work/sb/repo");
    if !repo.join(".git").exists() {
        eprintln!("skipped: no clone at {}", repo.display());
        return;
    }
    let opts = Options { repo: Some(repo) };
    for d in ["2283", "2234", "2297", "2129"] {
        validate_dir(&fx(d), &opts).unwrap_or_else(|f| panic!("{d}: {f}"));
    }
    let bad = tmp("bad-loc");
    copy_dir(&fx("2283"), &bad);
    let mut d = rec("2283", "differences");
    d["differences"][0]["location"]["endLine"] = json!(99999);
    write(&bad.join("differences.json"), &d);
    let f = validate_dir(&bad, &opts).unwrap_err();
    assert!(f.message.contains("beyond"), "{f}");
}

fn count(v: &Value, k: &str, f: impl Fn(&Value) -> bool) -> usize {
    items(v, k).iter().filter(|x| f(x)).count()
}

#[test]
fn ground_truth_2283() {
    let scope = rec("2283", "scope");
    assert_eq!(scope["changedLines"], json!({"production": 29, "test": 189, "docs": 10, "snapshot": 0, "generated": 0}));
    let parts = rec("2283", "parts");
    assert_eq!(count(&parts, "parts", |p| p["category"] == "production"), 5);
    let blocks = rec("2283", "blocks");
    assert_eq!(count(&blocks, "blocks", |b| b["category"] == "production"), 3);
    assert_eq!(items(&rec("2283", "differences"), "differences").len(), 3);
    assert_eq!(count(&rec("2283", "tests"), "tests", |t| t["category"] == "declared"), 3);
    // b1 = statement at :239 + filter method, b2 = lookup helper + call at :985, b3 = :1077
    let part = |id: &str| items(&parts, "parts").iter().find(|p| p["id"] == id).unwrap().clone();
    let holds = |b: usize| -> Vec<(Value, Value)> {
        items(&blocks, "blocks")[b]["parts"].as_array().unwrap().iter()
            .map(|p| { let p = part(p.as_str().unwrap()); (p["new"]["line"].clone(), p["member"].clone()) }).collect()
    };
    assert_eq!(holds(0), vec![(json!(239), Value::Null), (json!(1436), json!("ReportIfLocatedInCompilation"))]);
    assert_eq!(holds(1), vec![(json!(985), Value::Null), (json!(1429), json!("GetPropertyIdentifierLocation"))]);
    assert_eq!(holds(2), vec![(json!(1077), Value::Null)]);
    let links = rec("2283", "links");
    assert_eq!(items(&links, "links")[0]["block"], "b1");
    assert_eq!(items(&links, "links")[0]["state"], "declared");
    let b = rec("2283", "bundle");
    assert_eq!(b["oldCommit"], "32258fdbee4bcd653c79f51b4376b5103c6f4d27");
    assert_eq!(b["newCommit"], "9df83c38409c8bfad0517aebffe48211eda4a5b4");
    assert_eq!(b["provenance"], "fixture");
}

#[test]
fn worked_example_2283() {
    let tc = json!({"file": "src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs", "line": 1077, "endLine": 1077});
    let d = items(&rec("2283", "differences"), "differences").iter().find(|d| d["kind"] == "crash-fixed" && d["block"] == "b3").unwrap().clone();
    let mut want = json!({"id": "d3", "number": 3, "kind": "crash-fixed", "solverKind": "crash-fixed", "checker": "solver",
        "rows": d["rows"].clone(), "block": "b3", "condition": "metadata field, no syntax reference",
        "old": "throws IndexOutOfRangeException", "new": "nothing reported", "location": tc});
    want["id"] = d["id"].clone();
    want["number"] = d["number"].clone();
    assert_eq!(d, want);
    let rid = d["rows"][0].as_str().unwrap();
    let r = items(&rec("2283", "rows"), "rows").iter().find(|r| r["id"] == rid).unwrap().clone();
    assert_eq!(r, json!({"id": rid, "difference": d["id"], "unit": "TypeCollector.GetObjectInfo",
        "condition": "no derived key attribute, no syntax reference, public",
        "literals": ["(not derivedKey)", "(= dsrLen 0)", "isPublic"],
        "old": {"tag": "throws", "text": "IndexOutOfRangeException"}, "new": {"tag": "nothing", "text": "nothing reported"},
        "property": null, "perIteration": true, "assumptions": ["index-empty"], "firstChanged": tc}));
    let l = items(&rec("2283", "links"), "links").iter().find(|l| l["block"] == "b3").unwrap().clone();
    assert_eq!(l["state"], "declared");
    assert_eq!(l["tests"], json!(["GenericsFormatterTests.ClosedGenericStruct_FromMetadataReference"]));
    let rp = items(&rec("2283", "replays"), "replays").iter().find(|x| x["difference"] == d["id"]).unwrap().clone();
    assert_eq!(rp["input"]["source"], "issue");
    assert_eq!(rp["old"], "CS8785, AD0001: IndexOutOfRangeException");
    assert_eq!(rp["new"], "0 warnings; round trip [\"s\",[\"k\"]]");
    assert_eq!(rp["state"], "seen-live");
    assert_eq!(rp["roundTrip"], Value::Null);
    // the data-loss difference: 42 comes back as 0
    let replays = rec("2283", "replays");
    let d1 = &items(&replays, "replays")[0];
    assert_eq!(d1["roundTrip"], json!({"sent": "42", "received": "0"}));
    assert_eq!(items(&rec("2283", "differences"), "differences")[0]["kind"], "data-loss");
}

#[test]
fn ground_truth_2234() {
    let files = rec("2234", "files");
    let prod: Vec<&Value> = items(&files, "files").iter().filter(|f| f["category"] == "production").collect();
    assert_eq!(prod.iter().map(|f| f["added"].as_u64().unwrap()).sum::<u64>(), 18);
    assert_eq!(prod.iter().map(|f| f["deleted"].as_u64().unwrap()).sum::<u64>(), 4);
    assert_eq!(rec("2234", "scope")["changedLines"]["production"], 22);
    assert_eq!(count(&rec("2234", "parts"), "parts", |p| p["category"] == "production"), 6);
    let t = rec("2234", "tests");
    assert_eq!(count(&t, "tests", |t| t["category"] == "declared"), 4);
    assert_eq!(count(&t, "tests", |t| t["category"] == "hanging"), 4);
    assert_eq!(count(&t, "tests", |t| t["category"] == "not-run"), 2);
}

#[test]
fn ground_truth_2297() {
    let g = rec("2297", "groups");
    let g1 = &items(&g, "groups")[0];
    assert_eq!(g1["rule"], "identical-edit");
    assert_eq!(g1["count"], 402);
    let parts = rec("2297", "parts");
    let files: std::collections::BTreeSet<String> = g1["parts"].as_array().unwrap().iter().map(|id| {
        let p = items(&parts, "parts").iter().find(|p| &p["id"] == id).unwrap();
        assert_eq!(p["category"], "snapshot");
        p["new"]["file"].as_str().unwrap().to_string()
    }).collect();
    assert_eq!(files.len(), 222);
    let d = rec("2297", "differences");
    assert_eq!(items(&d, "differences").len(), 1);
    assert_eq!(items(&d, "differences")[0]["kind"], "output-changed");
    assert!(items(&d, "differences")[0]["location"]["file"].as_str().unwrap().contains("/GenericDataTypeWithPrivateMember/"));
    assert_eq!(rec("2297", "bundle")["sections"]["playground"]["state"], "not-run");
}

#[test]
fn ground_truth_2129() {
    let d = rec("2129", "differences");
    let api: Vec<&Value> = items(&d, "differences").iter().filter(|d| d["kind"] == "public-api").collect();
    assert_eq!(api.len(), 1);
    assert_eq!(api[0]["location"]["file"], "src/MessagePack.Annotations/Attributes.cs");
    assert_eq!(api[0]["new"], "MessagePack.GeneratedMessagePackFormatterAttribute");
    let t = rec("2129", "tests");
    let project: Vec<&Value> = items(&t, "tests").iter()
        .filter(|t| t["location"]["file"].as_str().unwrap().starts_with("tests/MessagePack.SourceGenerator.ExecutionTests/GeneratedFormatterForExternalTypesTests")).collect();
    assert_eq!(project.len(), 7);
    assert!(project.iter().all(|t| t["category"] == "not-run"));
}

/// An instance of each record for the missing-field test.
fn instance(schema: &str) -> Value {
    match schema {
        "session" | "review" => contracts::read_json(&fx(SESSION).join(format!("{schema}.json"))).unwrap(),
        "recording" => {
            let p = fs::read_dir(fx("recordings")).unwrap().flatten().next().unwrap().path();
            contracts::read_json(&p).unwrap()
        }
        "groups" => rec("2297", "groups"),
        "facts" => contracts::read_json(&fx("2129/map-extra/facts.json")).unwrap(),
        "snapshots" => contracts::read_json(&fx("2297/tests-extra/snapshots.json")).unwrap(),
        "suggested-tests" | "reverted" => contracts::read_json(&fx(&format!("2283/tests-extra/{schema}.json"))).unwrap(),
        _ => rec("2283", schema),
    }
}

#[test]
fn every_schema_rejects_a_missing_field() {
    let s = Schemas::contract().unwrap();
    let names: Vec<String> = fs::read_dir(root().join("schemas")).unwrap().flatten()
        .map(|e| e.file_name().to_string_lossy().trim_end_matches(".schema.json").to_string())
        .filter(|n| n != "common").collect();
    assert_eq!(names.len(), 24, "{names:?}");
    for name in names {
        let v = instance(&name);
        s.validate(&name, &v, &name).unwrap_or_else(|f| panic!("{name}: {f}"));
        let sch = s.schema(&name).unwrap(); let sch = if sch.get("required").is_some() { sch } else { &sch["anyOf"][0] };
        let mut checked = 0;
        for f in sch["required"].as_array().unwrap() {
            let f = f.as_str().unwrap();
            let mut w = v.clone();
            w.as_object_mut().unwrap().remove(f);
            let e = s.validate(&name, &w, &name).expect_err(&format!("{name} without {f} passed"));
            assert!(e.message.contains(&format!("`{f}`")), "{name}: {e}");
            checked += 1;
        }
        // required fields of array items
        for (k, p) in sch["properties"].as_object().unwrap() {
            let Some(req) = p["items"]["required"].as_array() else { continue };
            let v = if v[k].as_array().map(|a| a.is_empty()).unwrap_or(false) && !matches!(name.as_str(), "session" | "review" | "recording") { ["2234", "2297", "2129"].iter().filter(|n| fx(n).join(format!("{name}.json")).exists()).map(|n| rec(n, &name)).find(|x| !x[k].as_array().unwrap().is_empty()).unwrap_or_else(|| { eprintln!("{name}: no {k} item in any fixture"); v.clone() }) } else { v.clone() };
            if v[k].as_array().map(|a| a.is_empty()).unwrap_or(true) { continue; }
            for f in req {
                let f = f.as_str().unwrap();
                let mut w = v.clone();
                w[k][0].as_object_mut().unwrap().remove(f);
                let e = s.validate(&name, &w, &name).expect_err(&format!("{name}/{k} without {f} passed"));
                assert!(e.message.contains(&format!("`{f}`")) && e.path.contains(&format!("/{k}/0")), "{name}: {e}");
                checked += 1;
            }
        }
        assert!(checked > 0, "{name}");
    }
}

#[test]
fn line_in_two_parts_fails_naming_file_and_line() {
    let bad = tmp("bad-two-parts");
    copy_dir(&fx("2283"), &bad);
    let mut parts = rec("2283", "parts");
    let last = 0;
    parts["parts"][last]["addedLines"].as_array_mut().unwrap().push(json!(985));
    write(&bad.join("parts.json"), &parts);
    let out = Command::new(env!("CARGO_BIN_EXE_validate")).arg(&bad).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs:985") , "{text}");
    let ok = Command::new(env!("CARGO_BIN_EXE_validate")).args([fx("2283"), fx("2234"), fx("2297"), fx("2129")]).output().unwrap();
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stdout));
}

#[test]
fn corruptions_fail() {
    let cases: Vec<(&str, Box<dyn Fn(&mut Value)>, &str)> = vec![
        ("differences", Box::new(|v| v["differences"][0]["block"] = json!("b9")), "`b9` does not exist"),
        ("differences", Box::new(|v| v["differences"][0]["number"] = json!(7)), "must equal 1"),
        ("differences", Box::new(|v| v["differences"][1]["block"] = json!("b4")), "production block"),
        ("rows", Box::new(|v| v["rows"][2]["new"]["tag"] = json!("throws")), "outcome differs"),
        ("scope", Box::new(|v| v["changedLines"]["production"] = json!(30)), "parts sum to 29"),
        ("blocks", Box::new(|v| { let p = v["blocks"][0]["parts"][1].clone(); v["blocks"][1]["parts"].as_array_mut().unwrap().push(p) }), "also in"),
        ("links", Box::new(|v| { v["links"].as_array_mut().unwrap().remove(1); }), "`b2` has no entry"),
        ("ai-review", Box::new(|v| v["statements"][0]["cites"] = json!(["d7"])), "`d7` does not exist"),
        ("bundle", Box::new(|v| v["oldCommit"] = json!("32258fdb")), "does not match"),
    ];
    for (i, (r, f, want)) in cases.iter().enumerate() {
        let bad = tmp(&format!("bad-{i}"));
        copy_dir(&fx("2283"), &bad);
        let mut v = rec("2283", r);
        f(&mut v);
        write(&bad.join(format!("{r}.json")), &v);
        let e = validate_dir(&bad, &Options::default()).expect_err(want);
        assert!(e.to_string().contains(want), "case {i}: {e}");
    }
}

#[test]
fn prepared_before_tests_finish() {
    let d = tmp("tests-running");
    copy_dir(&fx("2283"), &d);
    let mut b = rec("2283", "bundle");
    b["sections"]["tests"] = json!({"state": "running"});
    b["sections"]["rules"] = json!({"state": "running"});
    write(&d.join("bundle.json"), &b);
    b["sections"]["review"] = json!({"state": "running"});
    write(&d.join("bundle.json"), &b);
    for r in ["links", "levels", "questions", "mode", "ai-review"] {
        fs::remove_file(d.join(format!("{r}.json"))).unwrap();
    }
    validate_dir(&d, &Options::default()).unwrap();
    write(&d.join("levels.json"), &rec("2283", "levels"));
    let e = validate_dir(&d, &Options::default()).unwrap_err();
    assert_eq!(e.path, "levels.json");
}

#[test]
fn recording_key_is_canonical() {
    let p = fs::read_dir(fx("recordings")).unwrap().flatten().next().unwrap().path();
    let v = contracts::read_json(&p).unwrap();
    let k = recording_key(v["purpose"].as_str().unwrap(), v["prompt"].as_str().unwrap(), &v["schema"]);
    assert_eq!(p.file_stem().unwrap().to_string_lossy(), k);
    let k2 = recording_key(v["purpose"].as_str().unwrap(), &format!("{} ", v["prompt"].as_str().unwrap()), &v["schema"]);
    assert_ne!(k, k2);
    assert_eq!(contracts::canonical_json(&json!({"b": 1, "a": [1, {"d": 2, "c": "x"}]})), r#"{"a":[1,{"c":"x","d":2}],"b":1}"#);
}

#[test]
fn one_example_per_message_validates() {
    let s = Schemas::contract().unwrap();
    let mut seen = std::collections::BTreeSet::new();
    let mut gateway_defs = std::collections::BTreeSet::new();
    for dir in [root().join("messages/examples"), fx("2283/inputs")] {
        for e in fs::read_dir(&dir).unwrap().flatten() {
            let file = e.file_name().to_string_lossy().to_string();
            let (schema, rest) = file.split_once('.').unwrap();
            let v = contracts::read_json(&e.path()).unwrap();
            if schema == "gateway" {
                let def = rest.trim_end_matches(".json");
                s.validate_def("gateway", def, &v, &file).unwrap_or_else(|f| panic!("{f}"));
                gateway_defs.insert(def.to_string());
            } else {
                s.validate(schema, &v, &file).unwrap_or_else(|f| panic!("{f}"));
            }
            seen.insert(schema.to_string());
            if let Some(m) = v.get("method") {
                seen.insert(format!("{schema}:{}", m.as_str().unwrap()));
            }
        }
    }
    let messages: Vec<String> = fs::read_dir(root().join("messages")).unwrap().flatten()
        .filter_map(|e| e.file_name().to_string_lossy().strip_suffix(".schema.json").map(String::from)).collect();
    for m in &messages {
        assert!(seen.contains(m), "no example for {m}");
    }
    for m in ["cfg", "references", "dependencies", "facts", "run"] {
        assert!(seen.contains(&format!("worker-request:{m}")) && seen.contains(&format!("worker-reply:{m}")), "worker {m}");
    }
    let defs = s.schema("gateway").unwrap()["$defs"].as_object().unwrap();
    for d in defs.keys() {
        assert!(gateway_defs.contains(d), "no gateway example for {d}");
    }
}

#[test]
fn manifest_output_list_is_closed_and_names_no_sandbox() {
    let s = Schemas::contract().unwrap();
    let m = contracts::read_json(&root().join("messages/examples/plugin-manifest.generator-lab.json")).unwrap();
    let mut chart = m.clone();
    chart["outputs"][0]["type"] = json!("chart");
    let e = s.validate("plugin-manifest", &chart, "plugin.json").unwrap_err();
    assert!(e.path.ends_with("/outputs/0/type") && e.message.contains("chart"), "{e}");
    let mut sb = m.clone();
    sb["sandbox"] = json!("bwrap");
    assert!(s.validate("plugin-manifest", &sb, "plugin.json").is_err());
    let text = fs::read_to_string(root().join("messages/plugin-manifest.schema.json")).unwrap();
    assert!(!text.contains("\"sandbox\""));
    let types = s.schema("plugin-manifest").unwrap()["$defs"]["resultType"]["enum"].as_array().unwrap().len();
    assert_eq!(types, 8);
}
