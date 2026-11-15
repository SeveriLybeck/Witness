use intake::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("intake-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sh(dir: &Path, args: &[&str]) -> String {
    let mut a = vec!["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"];
    a.extend_from_slice(args);
    git(dir, &a).unwrap()
}

fn write(dir: &Path, p: &str, s: &str) {
    let f = dir.join(p);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(f, s).unwrap();
}

/// Origin repository with main (one commit) and a pull ref `refs/pull/7/head`.
fn origin(name: &str, base_files: &[&str]) -> (PathBuf, String) {
    let o = tmp(name).join("origin");
    std::fs::create_dir_all(&o).unwrap();
    sh(&o, &["init", "--quiet"]);
    for f in base_files {
        write(&o, f, "x\n");
    }
    write(&o, "README.md", "r\n");
    sh(&o, &["add", "-A"]);
    sh(&o, &["commit", "--quiet", "-m", "base"]);
    sh(&o, &["checkout", "--quiet", "-b", "feature"]);
    write(&o, "src/Foo.cs", "class Foo {}\n");
    write(&o, "web/a.ts", "export const a = 1;\n");
    write(&o, "tool/main.go", "package main\n");
    write(&o, "data/blob.xyz", "?\n");
    sh(&o, &["add", "-A"]);
    sh(&o, &["commit", "--quiet", "-m", "change"]);
    let head = sh(&o, &["rev-parse", "HEAD"]);
    sh(&o, &["update-ref", "refs/pull/7/head", &head]);
    sh(&o, &["checkout", "--quiet", "main"]);
    (o, head)
}

#[test]
fn fallbacks_line1_is_contract_hash() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(root.join("crates/intake/FALLBACKS.md")).unwrap();
    let hash = git(&root, &["log", "-1", "--format=%H", "--", "contracts/"]).unwrap();
    assert_eq!(text.lines().next().unwrap(), hash);
}

#[test]
fn parses_links() {
    let l = parse_link("https://github.com/o/r/pull/12/files?x=1").unwrap();
    assert_eq!(l.repository, "https://github.com/o/r");
    assert_eq!(l.target, Target::Pull(12));
    let l = parse_link("file:///tmp/r/compare/main...feat").unwrap();
    assert_eq!(l.target, Target::Compare("main".into(), "feat".into()));
    assert!(parse_link("https://github.com/o/r").is_err());
    assert!(parse_link("https://github.com/o/r/pull/x").is_err());
}

#[test]
fn pull_by_local_url_plans_map_proof_and_progress() {
    let (o, head) = origin("pull", &["App.sln", "App/App.csproj"]);
    let link = format!("file://{}/pull/7", o.display());
    let l = parse_link(&link).unwrap();
    let mut events = Vec::new();
    let f = fetch(&l, &o.parent().unwrap().join("work"), &mut |s| events.push(s.to_string())).unwrap();
    assert_eq!(f.new, head);
    assert_eq!(events, ["fetch: clone", "fetch: refs"]);
    let mut p = plan(&link, &f).unwrap();
    let langs = p["languages"].as_array().unwrap();
    let get = |n: &str| langs.iter().find(|x| x["language"] == n).unwrap().clone();
    assert_eq!(get("csharp")["proof"], "planned");
    assert_eq!(get("csharp")["frontEnd"], "translator");
    assert_eq!(get("typescript")["frontEnd"], "translator-ts");
    assert_eq!(get("go")["map"], "planned");
    assert_eq!(get("go")["proof"], "no front end");
    assert_eq!(p["otherFiles"][0], "data/blob.xyz");
    // playground offered, but not started without a click
    assert_eq!(p["playground"]["stack"], "dotnet");
    assert_eq!(p["playground"]["offered"], true);
    assert!(start_playground(&mut p, false).is_err());
    assert_eq!(p["playground"]["started"], false);
    assert_eq!(start_playground(&mut p, true).unwrap(), "dotnet");
    assert_eq!(p["playground"]["started"], true);

    // prepare runs with visible progress; a stub stands in for `witness prepare`
    let cmd: Vec<String> = ["sh", "-c", "echo map $WITNESS_FRONT_ENDS; echo proof $2; exit 0", "stub"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut lines = Vec::new();
    let code = run_prepare(&cmd, &f, &p, &mut |s| lines.push(s.to_string())).unwrap();
    assert_eq!(code, 0);
    assert_eq!(lines[1], "prepare: map translator,translator-ts");
    assert_eq!(lines[2], format!("prepare: proof {}", f.old));
    assert_eq!(lines.last().unwrap(), "prepare: exit 0");
}

#[test]
fn unknown_stack_offers_no_playground() {
    let (o, _) = origin("nostack", &["Makefile"]);
    let link = format!("{}/compare/main...feature", o.display());
    let f = fetch(&parse_link(&link).unwrap(), &o.parent().unwrap().join("work"), &mut |_| {}).unwrap();
    let mut p = plan(&link, &f).unwrap();
    assert_eq!(p["playground"]["stack"], serde_json::Value::Null);
    assert_eq!(p["playground"]["offered"], false);
    assert_eq!(start_playground(&mut p, true).unwrap_err(), "no playground for this stack");
}
