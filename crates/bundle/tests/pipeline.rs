//! Stub modules replay the 2283 fixture through `witness prepare`.

use bundle::{group_rows, prepare, Config, Module};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

const OLD: &str = "32258fdbee4bcd653c79f51b4376b5103c6f4d27";
const NEW: &str = "9df83c38409c8bfad0517aebffe48211eda4a5b4";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn fix(name: &str) -> PathBuf {
    root().join("contracts/fixtures").join(name)
}
fn read(p: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}
fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// A shell module copying `recs` from `src` into its stage dir ($1).
fn stub(dir: &Path, name: &str, src: &Path, recs: &[&str], pre: &str) -> Vec<String> {
    let p = dir.join(format!("{name}.sh"));
    let cp: String = recs.iter().map(|r| format!("cp '{}/{r}.json' \"$1\"/\n", src.display())).collect();
    fs::write(&p, format!("#!/bin/sh\nset -e\n{pre}\n{cp}")).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    vec![p.display().to_string()]
}

fn modules(dir: &Path, src: &Path, tests_pre: &str) -> Vec<Module> {
    let m = |name: &str, section: &str, recs: &[&str], pre: &str| Module {
        name: name.into(), section: section.into(), command: stub(dir, name, src, recs, pre), budget: Duration::from_secs(20),
    };
    vec![
        m("cutter", "map", &["files", "parts", "blocks", "groups", "dependencies"], ""),
        m("differ", "proof", &["rows", "scope", "lean"], ""),
        m("lab", "playground", &["replays"], ""),
        m("model", "review", &["ai-review"], ""),
        m("testcontract", "tests", &["tests", "links"], tests_pre),
        m("rules", "rules", &["levels", "questions", "mode"], ""),
    ]
}

fn cfg(data: &Path, modules: Vec<Module>) -> Config {
    Config { data: data.into(), repo: "/nonexistent".into(), old: OLD.into(), new: NEW.into(), modules,
        description: Some("implementation for #2283".into()), repository: Some("junction26-messagepack-csharp".into()), ..Default::default() }
}

fn state(dir: &Path, s: &str) -> String {
    read(&dir.join("bundle.json"))["sections"][s]["state"].as_str().unwrap().to_string()
}

#[test]
fn prepare_2283_then_test_contract() {
    let d = tmp("full");
    let flag = d.join("tests-may-finish");
    let pre = format!("while [ ! -e '{}' ]; do sleep 0.02; done", flag.display());
    let mut b = prepare(cfg(&d, modules(&d, &fix("2283"), &pre))).unwrap();
    let dir = b.dir.clone();
    assert_eq!(dir, d.join(format!("bundles/{OLD}-{NEW}")));
    assert!(b.prepared());
    assert_eq!(state(&dir, "tests"), "running");
    assert_eq!(state(&dir, "rules"), "running");
    assert_eq!(state(&dir, "review"), "running");
    assert!(!dir.join("levels.json").exists() && !dir.join("links.json").exists());
    b.validate().unwrap();
    fs::write(&flag, "").unwrap();
    b.finish().unwrap();
    b.validate().unwrap();
    assert_eq!(state(&dir, "tests"), "ready");
    assert_eq!(state(&dir, "rules"), "ready");
    assert!(dir.join("mode.json").exists());
    assert_eq!(state(&dir, "review"), "ready");
    let declared = read(&dir.join("tests.json"))["tests"].as_array().unwrap().iter().filter(|t| t["category"] == "declared").count();
    assert_eq!(declared, 3);
    // 2.3: three differences on the right blocks; 2.4: replay turns d1 into data loss.
    let got = read(&dir.join("differences.json"));
    // The hand-written fixture groups r2 (IndexOutOfRange) and r3 (InvalidCast) into one difference;
    // the spec rule (same old and new outcome) keeps them apart, so 4 here.
    let want = read(&fix("2283/differences.json"));
    let (g, w) = (&got["differences"][0], &want["differences"][0]);
    for k in ["id", "number", "kind", "solverKind", "checker", "rows", "block", "old", "new", "location"] {
        assert_eq!(g[k], w[k], "d1 {k}");
    }
    let blocks: Vec<&str> = got["differences"].as_array().unwrap().iter().map(|d| d["block"].as_str().unwrap()).collect();
    assert_eq!(blocks, ["b1", "b2", "b2", "b3"]);
    // 2.4: boundary from block states; the filter block b1 sits inside "Declared".
    let links = read(&dir.join("links.json"));
    assert_eq!(links, json!({"provenance": "pipeline", "links": read(&fix("2283/links.json"))["links"]}));
    assert_eq!(links["links"][0]["block"], "b1");
    assert_eq!(links["links"][0]["placement"], "declared-region");
    assert_eq!(read(&dir.join("bundle.json"))["provenance"], "pipeline");
}

#[test]
fn grouping_ignores_input_order_and_ids() {
    let f = fix("2283");
    let mut rows = read(&f.join("rows.json"));
    let a = rows["rows"].as_array_mut().unwrap();
    a.reverse();
    for (i, r) in a.iter_mut().enumerate() {
        r["id"] = json!(format!("r{}", 9 - i));
        r["difference"] = json!("d9");
    }
    let g = group_rows(&rows, &[], &read(&f.join("parts.json")), &read(&f.join("blocks.json"))).unwrap();
    let ds = g.differences["differences"].as_array().unwrap();
    let got: Vec<(Value, Value, Value)> = ds.iter().map(|d| (d["block"].clone(), d["kind"].clone(), d["rows"].clone())).collect();
    // Reversed input: on b2 the InvalidCast row comes first; different exception texts stay apart.
    assert_eq!(got, vec![
        (json!("b1"), json!("silent-failure"), json!(["r1"])),
        (json!("b2"), json!("crash-fixed"), json!(["r2"])),
        (json!("b2"), json!("crash-fixed"), json!(["r3"])),
        (json!("b3"), json!("crash-fixed"), json!(["r4"])),
    ]);
    assert_eq!(g.rows["rows"][1]["old"]["text"], "InvalidCastException");
}

fn data(c: &str, r: &str) -> Value {
    read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(c).join(format!("{r}.json")))
}

#[test]
fn real_rows_group_across_units() {
    // Copies of the real 2234 and 2283 bundles' rows, parts, blocks (replays/<c>/bundle/).
    let count = |c: &str| {
        let g = group_rows(&data(c, "rows"), &[], &data(c, "parts"), &data(c, "blocks")).unwrap();
        g.differences["differences"].as_array().unwrap().iter()
            .map(|d| format!("{} {} | {} -> {}", d["block"].as_str().unwrap(), d["kind"].as_str().unwrap(), d["old"].as_str().unwrap(), d["new"].as_str().unwrap()))
            .collect::<Vec<_>>()
    };
    let d2234 = count("2234");
    assert_eq!(d2234, vec![
        "b2 crash-fixed | throws ArgumentException -> nothing reported",
        "b2 crash-fixed | reports MsgPack005, throws ArgumentException -> reports MsgPack005",
        "b3 crash-fixed | throws ArgumentException -> nothing reported",
        "b3 crash-fixed | reports MsgPack005, throws ArgumentException -> reports MsgPack005",
    ]);
    assert_eq!(count("2283").len(), 7, "{:#?}", count("2283"));
}

#[test]
fn invalid_rows_leave_proof_not_run() {
    let d = tmp("invalid");
    let src = d.join("src");
    fs::create_dir_all(&src).unwrap();
    for e in fs::read_dir(fix("2283")).unwrap().flatten() {
        if e.path().is_file() {
            fs::copy(e.path(), src.join(e.file_name())).unwrap();
        }
    }
    let mut rows = read(&src.join("rows.json"));
    rows["rows"][0].as_object_mut().unwrap().remove("condition");
    fs::write(src.join("rows.json"), rows.to_string()).unwrap();
    let b = prepare(cfg(&d, modules(&d, &src, ""))).unwrap();
    let bj = read(&b.dir.join("bundle.json"));
    assert_eq!(bj["sections"]["proof"]["state"], "not-run");
    let detail = bj["sections"]["proof"]["detail"].as_str().unwrap();
    assert!(detail.contains("differ") && detail.contains("condition"), "{detail}");
    assert_eq!(state(&b.dir, "map"), "ready");
    assert_eq!(state(&b.dir, "playground"), "ready");
    assert!(!b.prepared());
    b.validate().unwrap();
}

#[test]
fn time_budget_and_missing_modules() {
    let d = tmp("budget");
    let mut ms = modules(&d, &fix("2283"), "");
    ms.retain(|m| m.section != "review");
    let slow = ms.iter_mut().find(|m| m.section == "playground").unwrap();
    slow.command = stub(&d, "slowlab", &fix("2283"), &["replays"], "sleep 5");
    slow.budget = Duration::from_millis(200);
    let mut b = prepare(cfg(&d, ms)).unwrap();
    b.finish().unwrap();
    let bj = read(&b.dir.join("bundle.json"));
    assert_eq!(bj["sections"]["playground"]["state"], "not-run");
    assert!(bj["sections"]["playground"]["detail"].as_str().unwrap().contains("time budget"));
    assert_eq!(bj["sections"]["review"]["state"], "unavailable");
    assert_eq!(bj["sections"]["proof"]["state"], "ready");
}

#[test]
fn requirement_and_description() {
    let issues = root().join("../challenge_material/issues");
    let issues = if issues.exists() { issues } else { PathBuf::from("/home/slopsoftwareagent1/Documents/junction/challenge_material/issues") };
    if !issues.join("issue-2234.json").exists() {
        eprintln!("skip: no issues folder");
        return;
    }
    let d = tmp("req");
    let mut c = cfg(&d, vec![]);
    c.description = Some("Implementation for #2234\n\nmore".into());
    c.issues = Some(issues.clone());
    let b = prepare(c).unwrap();
    let bj = read(&b.dir.join("bundle.json"));
    let issue = read(&issues.join("issue-2234.json"));
    assert_eq!(bj["requirement"]["id"], "2234");
    assert_eq!(bj["requirement"]["text"], issue["body"]);
    assert!(bj["description"].as_str().unwrap().starts_with("Implementation for #2234"));
    assert_eq!(bj["sections"]["map"]["state"], "unavailable");
}

#[test]
fn cli_prepare_wait() {
    let d = tmp("cli");
    let ms: Vec<Value> = modules(&d, &fix("2283"), "").iter()
        .map(|m| json!({"name": m.name, "section": m.section, "command": m.command, "budgetSeconds": 20})).collect();
    fs::write(d.join("modules.json"), json!({"modules": ms}).to_string()).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_witness"))
        .args(["prepare", "--wait", "--description", "implementation for #2283", "--data"]).arg(&d)
        .arg("--modules").arg(d.join("modules.json")).args(["/nonexistent", OLD, NEW]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let dir = PathBuf::from(String::from_utf8(out.stdout).unwrap().trim());
    contracts::validate_bundle(&dir, &Default::default()).unwrap();
    assert_eq!(state(&dir, "rules"), "ready");
}

#[test]
fn first_hour_checks() {
    let line1 = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("FALLBACKS.md")).unwrap().lines().next().unwrap().to_string();
    match std::process::Command::new("git").arg("-C").arg(root()).args(["log", "-1", "--format=%H", "--", "contracts/"]).output() {
        Ok(o) if o.status.success() => assert_eq!(line1, String::from_utf8_lossy(&o.stdout).trim()),
        _ => eprintln!("skip: no git"),
    }
    for f in ["2283", "2234", "2297", "2129"] {
        contracts::validate_bundle(&fix(f), &Default::default()).unwrap();
    }
}
