use sandbox::{workspace_root, Queue, Sandbox, BASE_COMMIT};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use testcontract::{diff_tests, prepare, snapshot_entries, write, Change, Inputs, Output};

const UNKEYED: &str = "GenericsFormatterTests.ClosedGenericStruct_FromMetadataReference_UnkeyedMember";

static GIT: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn wc(change: &str, rev: &str) -> sandbox::WorkingCopy {
    let _g = GIT.lock().unwrap_or_else(|e| e.into_inner());
    testcontract::working_copy_locked(&sb(change), rev).unwrap()
}

// one sandbox root per change, as the CLI default (`testcontract-<new12>`)
fn sb(change: &str) -> Sandbox {
    let ws = workspace_root();
    Sandbox::new(ws.join(format!("work/sandbox/testcontract-{change}")), ws.join("work/nuget")).expect("sandbox")
}

fn fixture(c: &str) -> PathBuf {
    workspace_root().join("contracts/fixtures").join(c)
}

// 8.1
#[test]
fn fallbacks_line_one_is_contract_hash_and_fixtures_validate() {
    let ws = workspace_root();
    let out = Command::new("git").args(["log", "-1", "--format=%H", "--", "contracts/"]).current_dir(&ws).output().unwrap();
    let hash = String::from_utf8(out.stdout).unwrap().trim().to_string();
    assert_eq!(hash.len(), 40);
    let text = fs::read_to_string(ws.join("crates/testcontract/FALLBACKS.md")).unwrap();
    assert_eq!(text.lines().next().unwrap().trim(), hash);
    for c in ["2283", "2234", "2129", "2297"] {
        contracts::validate_dir(&fixture(c), &contracts::Options { repo: None }).unwrap_or_else(|f| panic!("{c}: {f}"));
    }
    let q = Queue::shared();
    let slot = q.acquire();
    assert!(slot.index < q.slots);
}

// 8.2 (light part): the pull request's tests come from the test-file diff alone.
#[test]
fn diff_of_2283_test_file_gives_three_added_tests() {
    let old = wc("2283", BASE_COMMIT);
    let new = wc("2283", "origin/ju26-issue-2283");
    let f = "tests/MessagePack.SourceGenerator.Tests/GenericsFormatterTests.cs";
    let t = diff_tests(f, fs::read_to_string(old.abs.join(f)).ok().as_deref(), fs::read_to_string(new.abs.join(f)).ok().as_deref());
    let names: Vec<&str> = t.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "GenericsFormatterTests.ClosedGenericStruct_FromMetadataReference",
            "GenericsFormatterTests.ClosedGenericRecordStruct_FromMetadataReference",
            UNKEYED
        ]
    );
    assert!(t.iter().all(|t| t.change == Change::Added));
}

// 8.3 (snapshots): the 2297 snapshot outside the mechanical group is "output changed".
#[test]
fn snapshot_of_2297_is_output_changed() {
    let old = wc("2297", BASE_COMMIT);
    let new = wc("2297", "origin/ju26-issue-2297");
    let parts: Value = serde_json::from_str(&fs::read_to_string(fixture("2297").join("parts.json")).unwrap()).unwrap();
    let groups: Value = serde_json::from_str(&fs::read_to_string(fixture("2297").join("groups.json")).unwrap()).unwrap();
    let grouped: BTreeSet<String> = groups["groups"].as_array().unwrap().iter().flat_map(|g| g["parts"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string())).collect();
    let v = snapshot_entries(parts["parts"].as_array().unwrap(), &grouped, &|f| fs::read_to_string(old.abs.join(f)).ok(), &|f| fs::read_to_string(new.abs.join(f)).ok());
    let snaps = v["snapshots"].as_array().unwrap();
    let hit = snaps.iter().find(|x| x["file"].as_str().unwrap().contains("GenericDataTypeWithPrivateMember")).expect("GenericDataTypeWithPrivateMember snapshot");
    assert_eq!(hit["kind"], "output-changed");
    assert!(hit["after"].as_str().is_some_and(|t| !t.is_empty()), "{hit}");
    assert!(snaps.len() < 10, "grouped snapshots must not appear: {}", snaps.len());
}

// ---- full run on 2283 (8.2-8.5), shared by the tests below ----

fn run_2283() -> &'static Output {
    static O: OnceLock<Output> = OnceLock::new();
    O.get_or_init(|| {
        let _g = GIT.lock().unwrap_or_else(|e| e.into_inner());
        let s = sb("2283");
        let q = Queue::shared();
        let head_before = s.resolve("origin/ju26-issue-2283").unwrap();
        let o = prepare(&s, &q, &Inputs { records: fixture("2283"), old: BASE_COMMIT.into(), new: "origin/ju26-issue-2283".into() }, "tc").expect("prepare 2283");
        assert_eq!(s.resolve("origin/ju26-issue-2283").unwrap(), head_before, "author branch moved");
        let out = workspace_root().join("work/testcontract-out/2283");
        write(&o, &out).unwrap();
        o
    })
}

fn test_entry<'a>(o: &'a Output, name: &str) -> &'a Value {
    o.tests["tests"].as_array().unwrap().iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("{name} missing: {}", o.tests))
}

// 8.2
#[test]
fn tests_2283_declared_and_records_validate() {
    let o = run_2283();
    let tests = o.tests["tests"].as_array().unwrap();
    assert_eq!(tests.len(), 3, "{}", o.tests);
    for t in tests {
        assert_eq!(t["category"], "declared", "{t}");
        assert_eq!(t["old"], "fail");
        assert_eq!(t["new"], "pass");
    }
    // our records replace the fixture's and the bundle still validates
    let dir = workspace_root().join("work/testcontract-out/2283-bundle");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for e in fs::read_dir(fixture("2283")).unwrap() {
        let e = e.unwrap();
        if e.path().is_file() {
            fs::copy(e.path(), dir.join(e.file_name())).unwrap();
        }
    }
    fs::write(dir.join("tests.json"), o.tests.to_string()).unwrap();
    fs::write(dir.join("links.json"), o.links.to_string()).unwrap();
    contracts::validate_dir(&dir, &contracts::Options { repo: None }).unwrap_or_else(|f| panic!("{f}"));
    // the test step ran with network on, builds with network off
    let steps = sb("2283").steps();
    assert!(steps.iter().any(|s| s["step"] == "test" && s["network"] == "on"));
    assert!(steps.iter().filter(|s| s["step"] == "build").all(|s| s["network"] == "off"));
}

// 8.3
#[test]
fn unkeyed_test_shows_msgpack004_and_no_diagnostics_assertion() {
    let o = run_2283();
    let t = test_entry(o, UNKEYED);
    let msg = t["failureMessage"].as_str().expect("failure message");
    assert!(msg.contains("MsgPack004"), "{msg}");
    let after = t["assertion"]["after"].as_str().expect("assertion");
    let new = wc("2283", "origin/ju26-issue-2283");
    let src = fs::read_to_string(new.abs.join("tests/MessagePack.SourceGenerator.Tests/GenericsFormatterTests.cs")).unwrap();
    let flat: Vec<&str> = src.lines().map(str::trim).collect();
    assert!(after.lines().all(|l| flat.contains(&l)), "not verbatim: {after}");
    assert!(t["assertion"]["before"].is_null());
}

// 8.4
#[test]
fn filter_block_declared_and_every_block_has_one_state() {
    let o = run_2283();
    let links = o.links["links"].as_array().unwrap();
    let ids: Vec<&str> = links.iter().map(|l| l["block"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["b1", "b2", "b3"]);
    for l in links {
        assert!(["declared", "not-declared", "not-linked"].contains(&l["state"].as_str().unwrap()), "{l}");
        if l["state"] != "declared" {
            assert!(l["tests"].as_array().unwrap().is_empty());
        }
    }
    let b1 = &links[0];
    assert_eq!(b1["state"], "declared", "{}", o.links);
    assert!(b1["tests"].as_array().unwrap().iter().any(|t| t == UNKEYED), "{b1}");
    for r in o.reverted["reverted"].as_array().unwrap() {
        if r["compiles"] == true {
            let p = PathBuf::from(r["path"].as_str().unwrap());
            assert!(p.join("bin/MessagePack.SourceGenerator").exists(), "reverted build kept: {}", p.display());
        }
    }
}

// 8.5
#[test]
fn generated_test_for_undeclared_difference_compiles() {
    let o = run_2283();
    let states: Vec<(String, String)> = o.links["links"].as_array().unwrap().iter().map(|l| (l["block"].as_str().unwrap().into(), l["state"].as_str().unwrap().into())).collect();
    let undeclared_blocks: Vec<&String> = states.iter().filter(|(_, s)| s != "declared").map(|(b, _)| b).collect();
    let sug = o.suggested["suggested"].as_array().unwrap();
    // one per undeclared difference with a replay (every 2283 difference has one)
    let diffs: Value = serde_json::from_str(&fs::read_to_string(fixture("2283").join("differences.json")).unwrap()).unwrap();
    let want: Vec<&str> = diffs["differences"].as_array().unwrap().iter().filter(|d| undeclared_blocks.iter().any(|b| d["block"] == b.as_str())).map(|d| d["id"].as_str().unwrap()).collect();
    let got: Vec<&str> = sug.iter().map(|s| s["difference"].as_str().unwrap()).collect();
    assert_eq!(got, want, "{}", o.links);
    assert!(!sug.is_empty(), "2283 has no undeclared difference: {}", o.links);
    for s in sug {
        assert_eq!(s["compiles"], true, "{s}");
        assert!(s["patch"].as_str().unwrap().contains("+    [Fact]"));
    }
}

// ---- real bundles from `replays/` (one full run at a time: GIT lock) ----

fn run_replay(change: &str) -> Output {
    let _g = GIT.lock().unwrap_or_else(|e| e.into_inner());
    let records = workspace_root().join("replays").join(change).join("bundle");
    let b: Value = serde_json::from_str(&fs::read_to_string(records.join("bundle.json")).unwrap()).unwrap();
    let (old, new) = (b["oldCommit"].as_str().unwrap().to_string(), b["newCommit"].as_str().unwrap().to_string());
    prepare(&sb(change), &Queue::shared(), &Inputs { records, old, new }, "tc").unwrap_or_else(|e| panic!("prepare {change}: {e}"))
}

// Reverts of 2234 compile (another test project that never builds must not make them not-linked).
#[test]
fn reverts_of_2234_compile_and_link() {
    let o = run_replay("2234");
    for r in o.reverted["reverted"].as_array().unwrap() {
        assert_eq!(r["compiles"], true, "{}", o.reverted);
        assert!(r["errors"].as_array().unwrap().is_empty());
    }
    let links = o.links["links"].as_array().unwrap();
    assert!(links.iter().all(|l| l["state"] != "not-linked"), "{}", o.links);
    assert!(links.iter().any(|l| l["state"] == "declared"), "{}", o.links);
}

// 2297: the template block reverts with its generated twin, so its declared tests link.
#[test]
fn template_block_of_2297_links_with_generated_twin() {
    let o = run_replay("2297");
    let declared: Vec<&str> = o.tests["tests"].as_array().unwrap().iter().filter(|t| t["category"] == "declared").map(|t| t["name"].as_str().unwrap()).collect();
    assert!(!declared.is_empty(), "{}", o.tests);
    let linked: BTreeSet<&str> = o.links["links"].as_array().unwrap().iter().flat_map(|l| l["tests"].as_array().unwrap().iter().map(|t| t.as_str().unwrap())).collect();
    for d in &declared {
        assert!(linked.contains(d), "{d} links to no block: {}", o.links);
    }
}
