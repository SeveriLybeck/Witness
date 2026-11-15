//! Snapshot (2297) and public-API (2129) differences from staged non-record inputs.

use bundle::{prepare, Config, Module};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

const OLD: &str = "32258fdbee4bcd653c79f51b4376b5103c6f4d27";
const NEW: &str = "9df83c38409c8bfad0517aebffe48211eda4a5b4";

fn fix(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/fixtures").join(name)
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

/// A module copying `<dir>/<file>` paths into its stage dir ($1).
fn module(dir: &Path, name: &str, section: &str, files: &[PathBuf]) -> Module {
    let p = dir.join(format!("{name}.sh"));
    let cp: String = files.iter().map(|f| format!("cp '{}' \"$1\"/\n", f.display())).collect();
    fs::write(&p, format!("#!/bin/sh\nset -e\n{cp}")).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    Module { name: name.into(), section: section.into(), command: vec![p.display().to_string()], budget: Duration::from_secs(20) }
}

fn recs(f: &Path, names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(|n| f.join(format!("{n}.json"))).collect()
}

fn run(d: &Path, fx: &str, map_extra: Vec<PathBuf>, tests_extra: Vec<PathBuf>) -> PathBuf {
    let f = fix(fx);
    let mut map = recs(&f, &["files", "parts", "blocks", "groups", "dependencies"]);
    map.extend(map_extra);
    let mut tests = recs(&f, &["tests", "links"]);
    tests.extend(tests_extra);
    // Empty replays: the lab ran and replayed nothing.
    fs::write(d.join("replays.json"), r#"{"replays": []}"#).unwrap();
    let ms = vec![
        module(d, "cutter", "map", &map),
        module(d, "differ", "proof", &recs(&f, &["rows", "scope"])),
        module(d, "lab", "playground", &[d.join("replays.json")]),
        module(d, "testcontract", "tests", &tests),
        module(d, "rules", "rules", &recs(&f, &["levels", "questions", "mode"])),
    ];
    let cfg = Config { data: d.into(), repo: "/nonexistent".into(), old: OLD.into(), new: NEW.into(), modules: ms,
        description: Some("x".into()), repository: Some("r".into()), ..Default::default() };
    let mut b = prepare(cfg).unwrap();
    assert!(b.prepared());
    b.finish().unwrap();
    b.validate().unwrap();
    b.dir.clone()
}

#[test]
fn snapshot_2297_gives_output_changed_on_transform_utilities_block() {
    let d = tmp("snap");
    let file = "tests/MessagePack.SourceGenerator.Tests/Resources/GenericDataTypeWithPrivateMember/Formatters.X.g.cs";
    let snaps = json!({"snapshots": [
        {"part": "p144", "kind": "output-changed", "checker": "file-comparison", "file": file,
         "location": {"file": file, "line": 9, "endLine": 13}, "before": "{\tinternal sealed class X", "after": "{\n#if A\n#endif\n\tinternal sealed class X"},
        {"part": "p146", "kind": "output-changed", "checker": "file-comparison", "file": file,
         "location": {"file": file, "line": 1, "endLine": 1}, "before": null, "after": ""}]});
    fs::write(d.join("snapshots.json"), snaps.to_string()).unwrap();
    let dir = run(&d, "2297", vec![], vec![d.join("snapshots.json")]);
    let ds = read(&dir.join("differences.json"))["differences"].as_array().unwrap().clone();
    assert_eq!(ds.len(), 1);
    let x = &ds[0];
    assert_eq!((x["id"].as_str(), x["kind"].as_str(), x["checker"].as_str(), x["block"].as_str()),
        (Some("d1"), Some("output-changed"), Some("file-comparison"), Some("b9")));
    assert_eq!(x["old"], "{\tinternal sealed class X");
    assert_eq!(x["blockRule"], "first-cs-block");
    assert_eq!(x["condition"], "snapshot GenericDataTypeWithPrivateMember");
    let rp = read(&dir.join("replays.json"));
    assert_eq!(rp["replays"][0]["state"], "not-replayed");
    let links = read(&dir.join("links.json"));
    let b9 = links["links"].as_array().unwrap().iter().find(|l| l["block"] == "b9").unwrap();
    assert_eq!(b9["placement"], "outside");
}

#[test]
fn differ_grouping_kept_for_report_names() {
    let f = fix("2283");
    let loc = json!({"file": "src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs", "line": 1077, "endLine": 1077});
    let row = |id: &str, d: &str, old: Value| json!({"id": id, "difference": d, "unit": "TypeCollector.GetObjectInfo#1077",
        "condition": format!("c{id}"), "literals": [], "old": old, "new": {"tag": "nothing", "text": "nothing reported"},
        "property": null, "perIteration": false, "assumptions": [], "firstChanged": loc});
    let rows = json!({"rows": [
        row("r1", "d1", json!({"tag": "reports", "text": "MemberNeedsKey"})),
        row("r2", "d2", json!({"tag": "reports", "text": "AOTDerivedKeyAttribute"})),
        row("r3", "d1", json!({"tag": "reports", "text": "MemberNeedsKey"})),
        row("r4", "d3", json!({"tag": "reports", "text": "reports MsgPack004, throws InvalidCastException"}))]});
    let g = bundle::group_rows(&rows, &[], &read(&f.join("parts.json")), &read(&f.join("blocks.json"))).unwrap();
    let ds = g.differences["differences"].as_array().unwrap();
    let got: Vec<(&str, &str, &str, Value)> = ds.iter()
        .map(|d| (d["block"].as_str().unwrap(), d["kind"].as_str().unwrap(), d["old"].as_str().unwrap(), d["rows"].clone())).collect();
    assert_eq!(got, vec![
        ("b3", "silent-failure", "reports MemberNeedsKey", json!(["r1", "r2"])),
        ("b3", "silent-failure", "reports AOTDerivedKeyAttribute", json!(["r3"])),
        ("b3", "silent-failure", "reports MsgPack004, throws InvalidCastException", json!(["r4"])),
    ]);
}

#[test]
fn facts_2129_real_shape_one_per_addition() {
    let f = fix("2129");
    let at = |file: &str, l: u64| json!({"file": file, "line": l, "endLine": l});
    let an = "src/MessagePack.SourceGenerator/Analyzers/MsgPack00xMessagePackAnalyzer.cs";
    let ana = "T:MessagePack.SourceGenerator.Analyzers.MsgPack00xMessagePackAnalyzer";
    let mut fs = vec![
        json!({"kind": "public-type", "symbol": "T:MessagePack.GeneratedMessagePackFormatterAttribute",
               "location": {"file": "src/MessagePack.Annotations/Attributes.cs", "line": 142, "endLine": 177}}),
        json!({"kind": "public-type", "symbol": "T:MessagePack.SourceGenerator.CodeAnalysis.GeneratedFormatterRequest",
               "location": at("src/MessagePack.SourceGenerator/CodeAnalysis/GeneratedFormatterRequest.cs", 1)}),
        json!({"kind": "public-member", "symbol": "P:MessagePack.GeneratedMessagePackFormatterAttribute.SurrogateType",
               "container": "T:MessagePack.GeneratedMessagePackFormatterAttribute", "location": at("src/MessagePack.Annotations/Attributes.cs", 176)}),
        json!({"kind": "public-member", "symbol": "P:MessagePack.SourceGenerator.CodeAnalysis.ObjectSerializationInfo.IsUserDeclaredFormatter",
               "container": "T:MessagePack.SourceGenerator.CodeAnalysis.ObjectSerializationInfo",
               "location": at("src/MessagePack.SourceGenerator/CodeAnalysis/ObjectSerializationInfo.cs", 13)}),
        json!({"kind": "new-file", "symbol": "src/MessagePack.SourceGenerator/CodeAnalysis/GeneratedFormatterRequest.cs", "location": null}),
        json!({"kind": "new-file", "symbol": "src/MessagePack.SourceGenerator/CodeAnalysis/GeneratedFormatterCollector.cs", "location": null}),
        json!({"kind": "new-project", "symbol": "tests/X.csproj", "location": null}),
        json!({"kind": "new-dependency", "symbol": "PackageReference:Y", "location": null}),
    ];
    for (i, (id, line)) in [("MsgPack019", 32u64), ("MsgPack020", 318), ("MsgPack021", 381), ("MsgPack022", 32)].iter().enumerate() {
        fs.push(json!({"kind": "new-diagnostic", "symbol": format!("F:{}.D{i}", &ana[2..]), "new": id, "diagnosticId": id, "location": at(an, *line)}));
        for m in [format!("F:{}.D{i}", &ana[2..]), format!("F:{}.D{i}Id", &ana[2..])] {
            fs.push(json!({"kind": "public-member", "symbol": m, "container": ana, "diagnostic": id, "location": at(an, *line)}));
        }
    }
    let ds = bundle::derive::api_differences(&json!({"facts": fs}), &read(&f.join("parts.json")), &read(&f.join("blocks.json")));
    let conds: Vec<&str> = ds.iter().map(|d| d["condition"].as_str().unwrap()).collect();
    assert_eq!(conds, ["new public type", "new public type", "new public member",
        "new diagnostic MsgPack019", "new diagnostic MsgPack020", "new diagnostic MsgPack021", "new diagnostic MsgPack022"]);
    assert!(ds.iter().all(|d| d["kind"] == "public-api" && d["checker"] == "api-comparison"));
}

#[test]
fn facts_2129_give_public_api_and_architectural() {
    let d = tmp("facts");
    // One-line type span (as the worker gives it): the member at :150 is covered by its symbol's owner.
    let loc = json!({"file": "src/MessagePack.Annotations/Attributes.cs", "line": 142, "endLine": 142});
    let facts = json!({"id": 4, "method": "facts", "result": {"facts": [
        {"kind": "public-type", "symbol": "T:MessagePack.GeneratedMessagePackFormatterAttribute", "old": null, "new": null, "location": loc},
        {"kind": "public-member", "symbol": "P:MessagePack.GeneratedMessagePackFormatterAttribute.Type",
         "location": {"file": "src/MessagePack.Annotations/Attributes.cs", "line": 150, "endLine": 150}},
        {"kind": "new-diagnostic", "symbol": "MsgPack020", "location": null},
        {"kind": "public-type", "symbol": "T:Elsewhere", "location": null}]}});
    fs::write(d.join("facts.json"), facts.to_string()).unwrap();
    let dir = run(&d, "2129", vec![d.join("facts.json")], vec![]);
    let ds = read(&dir.join("differences.json"))["differences"].as_array().unwrap().clone();
    assert_eq!(ds.len(), 1);
    let x = &ds[0];
    assert_eq!((x["kind"].as_str(), x["checker"].as_str(), x["block"].as_str(), x["new"].as_str(), x["old"].as_str()),
        (Some("public-api"), Some("api-comparison"), Some("b1"), Some("MessagePack.GeneratedMessagePackFormatterAttribute"), Some("absent")));
    assert_eq!(x["location"], loc);
    assert_eq!(read(&dir.join("facts.json")), facts);
    // The rules crate fires `architectural` on b1 from this difference alone.
    let inp = rules::Inputs::read_dir(&dir).unwrap();
    let (levels, _, _) = rules::run(&inp, &rules::Config::default());
    let b1 = levels["levels"].as_array().unwrap().iter().find(|l| l["block"] == "b1").unwrap();
    assert_eq!(b1["level"], 3);
    assert!(b1["rules"].as_array().unwrap().iter().any(|r| r == "architectural"), "{b1}");
}
