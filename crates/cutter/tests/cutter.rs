use cutter::{cut_repo, references_from_reply, Cut, Options};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const BASE: &str = "32258fdb";

fn repo() -> Option<PathBuf> {
    let p = std::env::var("WITNESS_SAMPLE_REPO").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from("/home/slopsoftwareagent1/Documents/junction/challenge_material/junction26-messagepack-csharp")
    });
    if p.join(".git").exists() || p.join("HEAD").exists() {
        Some(p)
    } else {
        eprintln!("skip: no sample repository at {}", p.display());
        None
    }
}
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn cut(change: &str, refs: &BTreeMap<String, Vec<String>>) -> Option<Cut> {
    let r = repo()?;
    Some(cut_repo(&r, BASE, &format!("origin/ju26-issue-{change}"), refs, &Options::default()).expect("cut"))
}
fn no_refs() -> BTreeMap<String, Vec<String>> {
    BTreeMap::new()
}
fn refs_2283() -> BTreeMap<String, Vec<String>> {
    let p = root().join("contracts/fixtures/2283/inputs/worker-reply.references.json");
    references_from_reply(&serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap())
}
fn prod(c: &Cut) -> Vec<&cutter::Part> {
    c.parts.iter().filter(|p| p.category == "production").collect()
}

#[test]
fn fallbacks_line_one_is_contract_hash() {
    let out = std::process::Command::new("git").arg("-C").arg(root())
        .args(["log", "-1", "--format=%H", "--", "contracts/"]).output().unwrap();
    let hash = String::from_utf8(out.stdout).unwrap();
    let f = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("FALLBACKS.md")).unwrap();
    assert_eq!(f.lines().next().unwrap().trim(), hash.trim());
}

#[test]
fn grammar_loads_and_parses() {
    assert_eq!(cutter::csharp::parse_errors("class A { void M() { if (x) { y(); } } }"), 0);
}

#[test]
fn every_changed_line_in_exactly_one_part() {
    for ch in ["2283", "2234", "2297", "2129"] {
        let Some(c) = cut(ch, &no_refs()) else { return };
        let mut seen: BTreeMap<(String, char, u32), usize> = BTreeMap::new();
        for p in &c.parts {
            for l in &p.added {
                *seen.entry((p.new_file.clone().unwrap(), '+', *l)).or_default() += 1;
            }
            for l in &p.deleted {
                *seen.entry((p.old_file.clone().unwrap(), '-', *l)).or_default() += 1;
            }
        }
        assert!(seen.values().all(|&n| n == 1), "{ch}: a line in two parts");
        let total: usize = c.files.iter().map(|f| f.added + f.deleted).sum();
        assert_eq!(seen.len(), total, "{ch}: lines missing from parts");
    }
}

#[test]
fn parts_2283() {
    let Some(c) = cut("2283", &no_refs()) else { return };
    let count = |cat: &str| c.files.iter().filter(|f| f.category == cat).map(|f| f.added + f.deleted).sum::<usize>();
    assert_eq!((count("production"), count("test"), count("docs")), (29, 189, 10));
    let fx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root().join("contracts/fixtures/2283/parts.json")).unwrap()).unwrap();
    let p = prod(&c);
    assert_eq!(p.len(), 5);
    for (mine, want) in p.iter().zip(fx["parts"].as_array().unwrap()) {
        assert_eq!(mine.id, want["id"].as_str().unwrap());
        assert_eq!(mine.kind, want["kind"].as_str().unwrap(), "{}", mine.id);
        assert_eq!(serde_json::json!(mine.added), want["addedLines"], "{}", mine.id);
        assert_eq!(serde_json::json!(mine.deleted), want["deletedLines"], "{}", mine.id);
        assert_eq!(serde_json::json!(mine.member), want["member"], "{}", mine.id);
    }
}

#[test]
fn blocks_2283() {
    let Some(c) = cut("2283", &refs_2283()) else { return };
    let sets: BTreeSet<Vec<String>> = c.blocks.iter().filter(|b| b.category == "production").map(|b| b.parts.clone()).collect();
    let want: BTreeSet<Vec<String>> = [vec!["p1", "p5"], vec!["p2", "p4"], vec!["p3"]]
        .iter().map(|v| v.iter().map(|s| s.to_string()).collect()).collect();
    assert_eq!(sets, want);
    let b1 = c.blocks.iter().find(|b| b.parts.contains(&"p5".to_string())).unwrap();
    assert_eq!(b1.name, "ReportIfLocatedInCompilation");
    for b in c.blocks.iter().filter(|b| b.category != "production") {
        assert_eq!(b.parts.len(), 1);
        let p = c.parts.iter().find(|p| p.id == b.parts[0]).unwrap();
        assert_ne!(p.category, "production");
    }
    // names are stable
    let again = cut("2283", &refs_2283()).unwrap();
    let names = |c: &Cut| c.blocks.iter().map(|b| b.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&c), names(&again));
    assert!(c.groups.is_empty());
}

#[test]
fn large_comments_2283() {
    let Some(c) = cut("2283", &no_refs()) else { return };
    let lc: Vec<bool> = prod(&c).iter().map(|p| !p.large_comment.is_empty()).collect();
    assert_eq!(lc, vec![false, false, false, true, true]);
    assert_eq!(prod(&c)[3].large_comment, vec![1429, 1430, 1431, 1432, 1433]);
}

#[test]
fn deterministic_2234() {
    let Some(c) = cut("2234", &no_refs()) else { return };
    let d = cut("2234", &no_refs()).unwrap();
    assert_eq!(serde_json::to_string(&c.parts_json()).unwrap(), serde_json::to_string(&d.parts_json()).unwrap());
    let p = prod(&c);
    assert_eq!(p.len(), 6);
    assert!(p.iter().all(|x| x.enclosing.as_deref() == Some("GetObjectInfo")));
    let blocks: Vec<Vec<String>> = c.blocks.iter().filter(|b| b.category == "production").map(|b| b.parts.clone()).collect();
    assert_eq!(blocks, vec![vec!["p1".to_string(), "p4".into()], vec!["p2".into(), "p5".into()], vec!["p3".into(), "p6".into()]]);
}

#[test]
fn mechanical_2297() {
    let Some(c) = cut("2297", &no_refs()) else { return };
    let g = c.groups.iter().find(|g| g.rule == "identical-edit" && g.parts.len() == 402).expect("402 group");
    assert_eq!(g.variants, vec![311, 91]);
    let in_group: BTreeSet<&String> = c.groups.iter().flat_map(|g| g.parts.iter()).collect();
    let real: Vec<&cutter::Part> = c.parts.iter().filter(|p| !in_group.contains(&p.id)).collect();
    assert!(real.iter().any(|p| p.file().ends_with("Transforms/TransformUtilities.cs")));
    assert!(real.iter().any(|p| p.category == "snapshot" && p.file().contains("GenericDataTypeWithPrivateMember")));
    let snap_real = real.iter().filter(|p| p.category == "snapshot").count();
    assert_eq!(c.parts.iter().filter(|p| p.category == "snapshot").count() - snap_real, 402);
}

#[test]
fn records_validate() {
    let schemas = contracts::Schemas::contract().unwrap();
    for ch in ["2283", "2234", "2297", "2129"] {
        let refs = if ch == "2283" { refs_2283() } else { no_refs() };
        let Some(c) = cut(ch, &refs) else { return };
        for (n, v) in [("files", c.files_json()), ("parts", c.parts_json()), ("blocks", c.blocks_json()), ("groups", c.groups_json())] {
            schemas.validate(n, &v, n).unwrap_or_else(|e| panic!("{ch} {n}: {e:?}"));
        }
    }
}

fn production_blocks(dir: &Path) -> BTreeSet<Vec<String>> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("blocks.json")).unwrap()).unwrap();
    v["blocks"].as_array().unwrap().iter().filter(|b| b["category"] == "production")
        .map(|b| b["parts"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()).collect()).collect()
}

#[test]
fn builder_form_2283() {
    let Some(r) = repo() else { return };
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("builder_form_2283");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let reply = root().join("contracts/fixtures/2283/inputs/worker-reply.references.json");
    let want: BTreeSet<Vec<String>> = [vec!["p1", "p5"], vec!["p2", "p4"], vec!["p3"]]
        .iter().map(|v| v.iter().map(|s| s.to_string()).collect()).collect();
    let one_line = |p: PathBuf, to: &str| {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
        std::fs::write(tmp.join(to), serde_json::to_string(&v).unwrap() + "\n").unwrap();
    };
    one_line(reply.clone(), "refs.json");
    one_line(root().join("contracts/fixtures/2283/inputs/worker-reply.dependencies.json"), "deps.json");
    std::fs::write(tmp.join("empty.json"), "{\"id\":1,\"method\":\"references\",\"result\":{\"parts\":[]}}\n").unwrap();
    // Fake worker: answers references with the fixture reply only when the worktree it is handed
    // holds the new code (else no references, like the real worker on a wrong checkout).
    let script = tmp.join("worker.sh");
    std::fs::write(&script, format!(r#"#!/bin/sh
T='{0}'
read -r req; printf '%s\n' "$req" > "$T/req1.json"
wt=$(printf '%s' "$req" | sed 's/.*"worktree":"\([^"]*\)".*/\1/')
if grep -q ReportIfLocatedInCompilation "$wt/src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs" 2>/dev/null; then cat "$T/refs.json"; else cat "$T/empty.json"; fi
read -r req; printf '%s\n' "$req" > "$T/req2.json"
cat "$T/deps.json"
"#, tmp.display())).unwrap();
    let run = |name: &str, opts: Vec<String>, envs: Vec<(&str, String)>| -> (bool, PathBuf) {
        let stage = tmp.join(name);
        std::fs::create_dir_all(&stage).unwrap();
        let st = std::process::Command::new(env!("CARGO_BIN_EXE_cutter")).args(&opts)
            .arg(&stage).arg(tmp.join("bundle")).arg(&r).arg(BASE).arg("origin/ju26-issue-2283")
            .env_remove("WITNESS_REFERENCES").env_remove("WITNESS_WORKER")
            .env_remove("WITNESS_WT_OLD").env_remove("WITNESS_WT_NEW")
            .envs(envs).status().unwrap();
        (st.success(), stage)
    };
    let schemas = contracts::Schemas::contract().unwrap();
    // 1. references file
    let (ok, stage) = run("refs", vec!["--references".into(), reply.display().to_string()], vec![]);
    assert!(ok);
    assert_eq!(production_blocks(&stage), want);
    assert!(!stage.join("dependencies.json").exists());
    // 2. worker on worktrees the cutter checks out itself
    let worker = format!("sh '{}'", script.display());
    let (ok, stage) = run("worker", vec!["--worker".into(), worker.clone()], vec![]);
    assert!(ok, "worker run");
    assert_eq!(production_blocks(&stage), want);
    for f in ["files", "parts", "blocks", "groups", "dependencies"] {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(stage.join(format!("{f}.json"))).unwrap()).unwrap();
        schemas.validate(f, &v, f).unwrap();
    }
    let d: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(stage.join("dependencies.json")).unwrap()).unwrap();
    assert_eq!(d["provenance"], "pipeline");
    assert_eq!(d["dependencies"].as_array().unwrap().len(), 2);
    assert_eq!(d["reach"][0]["callSites"], 28);
    for (f, n) in [("req1.json", 5), ("req2.json", 3)] {
        let req: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(tmp.join(f)).unwrap()).unwrap();
        schemas.validate("worker-request", &req, f).unwrap();
        let k = if f == "req1.json" { "parts" } else { "blocks" };
        assert_eq!(req["params"][k].as_array().unwrap().len(), n, "{f}");
    }
    let req2: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(tmp.join("req2.json")).unwrap()).unwrap();
    let old_wt = PathBuf::from(req2["params"]["old"]["worktree"].as_str().unwrap());
    assert_ne!(old_wt, r);
    assert!(!old_wt.exists(), "temporary worktree removed");
    // 3. supplied worktree pointing at the wrong code: no references is an error, not wrong blocks
    let (ok, stage) = run("wrong", vec![], vec![("WITNESS_WORKER", worker.clone()), ("WITNESS_WT_OLD", r.display().to_string()), ("WITNESS_WT_NEW", tmp.display().to_string())]);
    assert!(!ok);
    assert!(!stage.join("blocks.json").exists());
}
