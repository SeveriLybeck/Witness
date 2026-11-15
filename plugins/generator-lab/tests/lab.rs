//! Generator lab on the real sample: tasks 11.1–11.4.
mod common;

use common::*;
use generator_lab::{outcome, Config, Lab, RunInput};
use plugin_host::{Manifest, PluginProcess};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Instant;

/// Timing targets are checked only when the shared machine is not overloaded (load under 1 per core).
fn busy() -> bool {
    let load: f64 = std::fs::read_to_string("/proc/loadavg").ok().and_then(|t| t.split_whitespace().next()?.parse().ok()).unwrap_or(0.0);
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
    let b = load > cores;
    if b {
        eprintln!("load {load}: timing not checked");
    }
    b
}

fn manifest() -> Manifest {
    Manifest::load(&here().join("plugin.json")).expect("manifest")
}

fn diag_ids(results: &[Value], side: &str) -> Vec<String> {
    results.iter().filter(|r| r["output"] == "diagnostics" && r["side"] == side)
        .flat_map(|r| r["value"].as_array().cloned().unwrap_or_default()).map(|d| d["id"].as_str().unwrap().to_string()).collect()
}
fn result<'a>(results: &'a [Value], output: &str, side: &str) -> &'a Value {
    &results.iter().find(|r| r["output"] == output && r["side"] == side).unwrap_or_else(|| panic!("no {output}/{side}"))["value"]
}

// 11.1 ---------------------------------------------------------------------------------------

#[test]
fn checklist_contract_hash_and_fixtures() {
    ensure_git();
    let o = std::process::Command::new("git").arg("-C").arg(repo()).args(["log", "-1", "--format=%H", "--", "contracts/"]).output().unwrap();
    let hash = String::from_utf8_lossy(&o.stdout).trim().to_string();
    let first = std::fs::read_to_string(here().join("FALLBACKS.md")).unwrap().lines().next().unwrap_or("").trim().to_string();
    assert_eq!(first, hash, "FALLBACKS.md line 1 must be the contract hash");
    for n in ["2283", "2234", "2297", "2129"] {
        contracts::validate_dir(&fixture(n), &contracts::Options::default()).unwrap_or_else(|e| panic!("{n}: {e:?}"));
    }
}

#[test]
fn worker_starts_in_plugin_process_without_network() {
    prep();
    let mut p = PluginProcess::start(&manifest(), &here().join("work/plugins/check")).expect("start");
    let r = p.request(json!({"op": "describe", "bundle": {"oldCommit": OLD, "newCommit": NEW_2283}, "workdir": "."})).expect("describe");
    assert_eq!(r["ok"], true, "{r}");
    assert!(r["pid"].as_u64().unwrap() > 0);
    p.shutdown().ok();
}

// 11.2 ---------------------------------------------------------------------------------------

#[test]
fn manifest_detects_the_sample() {
    let p = prep();
    let m = manifest();
    assert_eq!(m.name(), "generator-lab");
    let old = p.root.join("wt").join(OLD);
    assert!(m.detect(&old) && m.detect(&old), "detects the old commit, twice");
    assert!(!m.detect(&repo().join("contracts")), "no generator in contracts/");
    let cfg = Config::load().unwrap();
    assert_eq!(cfg.adapter["consumerPackages"]["Microsoft.Net.Compilers.Toolset"], "4.12.0");
    let ins: Vec<&str> = m.json["inputs"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap()).collect();
    assert_eq!(ins, ["library", "app", "buildProperties", "roundTrip", "generatedResolverOnly"]);
}

#[test]
fn consumer_project_builds_with_toolset_4_12() {
    let p = prep();
    let cfg = Config::load().unwrap();
    let side = cfg.tree(OLD).expect("old build");
    let dir = p.root.join("consumer/a1");
    let _ = std::fs::remove_dir_all(&dir);
    let input = RunInput::from_text(&cfg.reproduction("#2283").unwrap());
    generator_lab::write_consumer(&cfg, &dir, &side, &input).unwrap();
    let csproj = std::fs::read_to_string(dir.join("app/app.csproj")).unwrap();
    assert!(csproj.contains("Microsoft.Net.Compilers.Toolset\" Version=\"4.12.0\""));
    let q = sandbox::Queue::shared();
    let r = p.sb.run_heavy(&q, &sandbox::Step::new("consumer-restore", sandbox::Net::On, &["dotnet", "restore", "app/app.csproj", "-v", "q"]).cwd("consumer/a1"));
    assert!(r.ok(), "{}\n{}", r.stdout, r.stderr);
    let r = p.sb.run_heavy(&q, &sandbox::Step::new("consumer-build", sandbox::Net::Off,
        &["dotnet", "build", "app/app.csproj", "--no-restore", "-p:UseSharedCompilation=false", "-nologo"]).cwd("consumer/a1"));
    // the old generator is loaded (no CS9057) and crashes as the issue says (P1)
    assert!(r.stdout.contains("CS8785") && !r.stdout.contains("CS9057"), "{}", r.stdout);
}

// 11.3 ---------------------------------------------------------------------------------------

fn lab_for(n: &str) -> Lab {
    prep();
    Lab::for_bundle(Config::load().unwrap(), &fixture(n)).unwrap()
}

#[test]
fn plugin_2283_presets_runs_and_revert() {
    prep();
    let mut p = PluginProcess::start(&manifest(), &here().join("work/plugins/generator-lab").join(NEW_2283)).expect("start");
    let d = p.request(json!({"op": "describe", "bundle": {"oldCommit": OLD, "newCommit": NEW_2283}, "workdir": "."})).unwrap();
    assert_eq!(d["ok"], true, "{d}");
    let subj = &d["subjects"][0];
    let presets = subj["presets"].as_array().unwrap();
    let kinds: Vec<String> = presets.iter().map(|x| format!("{}:{}", x["kind"].as_str().unwrap(), x["label"].as_str().unwrap())).collect();
    assert_eq!(kinds, ["difference:1", "difference:2", "difference:3", "issue:2283", "revert:revert b1", "free:free"]);
    let unkeyed = presets[0]["inputs"].clone();
    let issue = presets[3]["inputs"].clone();

    // unkeyed member, round trip of 42
    let t = Instant::now();
    let r = p.request(json!({"op": "run", "subject": "consumer", "sides": ["old", "new"], "preset": {"kind": "difference", "difference": "d1"}, "inputs": unkeyed})).unwrap();
    eprintln!("unkeyed round trip {:?}: {r}", t.elapsed());
    assert!(busy() || t.elapsed().as_secs() < 10, "round trip over 10 s");
    assert!(!r.to_string().contains(&repo().display().to_string()), "no host path in the reply");
    let res = r["results"].as_array().unwrap();
    assert!(diag_ids(res, "old").contains(&"MsgPack004".to_string()));
    assert!(diag_ids(res, "new").is_empty());
    let rt = result(res, "roundTrip", "new");
    assert!(rt["sent"].as_str().unwrap().contains("42") && rt["received"].as_str().unwrap().contains("\"Unkeyed\":0"), "{rt}");
    assert_eq!(rt["lost"], "Member.Unkeyed 42 -> 0");
    assert_ne!(rt["pid"], r["pid"], "round trip runs in a bwrap child, not the worker");

    // generate only, warm
    let mut gen_only = presets[0]["inputs"].clone();
    gen_only["roundTrip"] = json!(false);
    let t = Instant::now();
    let g = p.request(json!({"op": "run", "subject": "consumer", "sides": ["old", "new"], "inputs": gen_only})).unwrap();
    eprintln!("generate only {:?}", t.elapsed());
    assert!(busy() || t.elapsed().as_millis() < 1000, "generate only over 1 s");
    assert_eq!(g["pid"], r["pid"], "same worker process");

    // closed generic struct (the issue)
    let r = p.request(json!({"op": "run", "subject": "consumer", "sides": ["old", "new"], "preset": {"kind": "issue"}, "inputs": issue})).unwrap();
    let res = r["results"].as_array().unwrap();
    let old = diag_ids(res, "old");
    assert!(old.contains(&"CS8785".into()) && old.contains(&"AD0001".into()), "{old:?}");
    assert!(diag_ids(res, "new").is_empty());
    let rt = result(res, "roundTrip", "new");
    assert!(rt["error"].is_null() && rt.get("lost").is_none() && rt["sent"] == rt["received"], "{rt}");
    assert!(result(res, "generated", "new")["new"].as_str().unwrap().contains("GeneratedMessagePackResolver"));

    // generated resolver only exposes the missing formatter the dynamic resolver hides
    let mut gro = issue.clone();
    gro["generatedResolverOnly"] = json!(true);
    let r = p.request(json!({"op": "run", "subject": "consumer", "sides": ["old"], "inputs": gro})).unwrap();
    let e = result(r["results"].as_array().unwrap(), "roundTrip", "old")["error"].as_str().unwrap_or("").to_string();
    assert!(e.contains("GeneratedMessagePackResolver"), "{e}");
    gro["generatedResolverOnly"] = json!(false);
    let r = p.request(json!({"op": "run", "subject": "consumer", "sides": ["old"], "inputs": gro})).unwrap();
    let v = result(r["results"].as_array().unwrap(), "roundTrip", "old");
    assert!(v["error"].is_null() && v["resolver"] == "standard", "{v}");

    // revert b1 and rerun: the reverted side reports MsgPack004 as old does
    let r = p.request(json!({"op": "run", "subject": "consumer", "sides": ["old", "new"], "preset": {"kind": "revert", "block": "b1"}, "inputs": presets[0]["inputs"]})).unwrap();
    assert_eq!(r["ok"], true, "{r}");
    assert!(diag_ids(r["results"].as_array().unwrap(), "reverted").contains(&"MsgPack004".to_string()));
    p.shutdown().ok();
}

#[test]
fn nowarn_2234_duplicate_keys() {
    let mut lab = lab_for("2234");
    let mut i = RunInput::from_text("using MessagePack;\nnamespace App;\n[MessagePackObject]\npublic class T { [Key(0)] public int A { get; set; } [Key(0)] public int B { get; set; } }\npublic static class Probe { public static object Make() => new T { A = 1, B = 2 }; }\n");
    i.build_properties = json!({"NoWarn": "MsgPack005"});
    let r = lab.run(&i, None).unwrap();
    eprintln!("2234 old {} | new {}", outcome(&r["old"]), outcome(&r["new"]));
    let ids = |s: &Value| s["diagnostics"].as_array().unwrap().iter().map(|d| d["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert!(ids(&r["old"]).contains(&"CS8785".into()), "old crashes");
    assert!(ids(&r["new"]).iter().all(|d| d != "MsgPack005" && d != "CS8785"));
    let gen: String = r["new"]["generated"].as_array().unwrap().iter().map(|g| g["text"].as_str().unwrap()).collect();
    assert!(gen.contains("value.A") && !gen.contains("value.B"), "formatter missing B");
}

// 11.4 ---------------------------------------------------------------------------------------

fn copy_bundle(n: &str, dst: &Path) {
    let _ = std::fs::remove_dir_all(dst);
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(fixture(n)).unwrap().flatten() {
        if e.path().is_file() {
            std::fs::copy(e.path(), dst.join(e.file_name())).unwrap();
        }
    }
}

#[test]
fn replay_2283() {
    let mut lab = lab_for("2283");
    let out = lab.replay(&[]).unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&out).unwrap());
    let rs = out["replays"].as_array().unwrap();
    let d1 = &rs[0];
    assert_eq!(d1["state"], "seen-live", "{d1}");
    assert_eq!(d1["roundTrip"], json!({"sent": "42", "received": "0"}), "data loss");
    assert_eq!(rs[1]["state"], "seen-live", "{}", rs[1]);
    assert_eq!(rs[1]["input"]["source"], "model", "d2 keeps its own input");
    assert_eq!(rs[2]["state"], "seen-live");
    // valid as a bundle record
    let dir = here().join("work/test-bundles/2283");
    copy_bundle("2283", &dir);
    let mut rec = out.clone();
    rec.as_object_mut().unwrap().remove("issue");
    std::fs::write(dir.join("replays.json"), rec.to_string()).unwrap();
    contracts::validate_dir(&dir, &contracts::Options::default()).unwrap_or_else(|e| panic!("{e:?}"));
}

#[test]
fn replay_model_snippet_kept_or_dropped() {
    prep();
    let dir = here().join("work/test-bundles/2283-proposed");
    copy_bundle("2283", &dir);
    std::fs::write(dir.join("replays.json"), r#"{"replays": []}"#).unwrap();
    let d1_text = read(&fixture("2283/replays.json"))["replays"][0]["input"]["text"].as_str().unwrap().to_string();
    // with the PR's declared tests, d1 (reports MsgPack004 -> nothing reported) is seen live from a test, losing 42
    let mut lab = Lab::for_bundle(Config::load().unwrap(), &dir).unwrap();
    // turned off: no replay carries source `test`, and d1 has candidates that do not match
    assert!(lab.test_candidates, "on by default");
    lab.test_candidates = false;
    let out = lab.replay(&[]).unwrap();
    assert!(out["replays"].as_array().unwrap().iter().all(|r| r["input"]["source"] != "test"));
    assert_eq!(out["replays"][0]["state"], "not-replayed");
    assert_eq!(out["replays"][0]["reason"], "no-match");
    assert!(out["replays"][0]["detail"].as_str().unwrap().contains("old "));
    // snippets were asked for and none is recorded for d1
    lab.snippets_expected = true;
    let out = lab.replay(&[]).unwrap();
    assert_eq!(out["replays"][0]["reason"], "no-recording");
    lab.snippets_expected = false;
    lab.test_candidates = true;
    let out = lab.replay(&[]).unwrap();
    let d1 = &out["replays"][0];
    assert_eq!(d1["state"], "seen-live", "{d1}");
    assert_eq!(d1["input"]["source"], "test");
    assert_eq!(d1["roundTrip"], json!({"sent": "42", "received": "0"}));
    // without test links, d1's candidates (issue reproduction, a snippet with no difference) do not match its row
    std::fs::write(dir.join("links.json"), r#"{"links": []}"#).unwrap();
    let mut lab = Lab::for_bundle(Config::load().unwrap(), &dir).unwrap();
    let bogus = vec![json!({"difference": "d1", "text": "using MessagePack;\nnamespace App;\n[MessagePackObject]\npublic class Ok { [Key(0)] public int A { get; set; } }\n"})];
    let out = lab.replay(&bogus).unwrap();
    let d1 = &out["replays"][0];
    assert_eq!(d1["state"], "not-replayed", "{d1}");
    assert!(d1["input"].is_null());
    let out = lab.replay(&[json!({"difference": "d1", "text": d1_text})]).unwrap();
    let d1 = &out["replays"][0];
    assert_eq!(d1["state"], "seen-live");
    assert_eq!(d1["input"]["source"], "model");
}

#[test]
fn replay_real_2283_with_test_candidates() {
    prep();
    let bundle = repo().join("replays/2283/bundle");
    let mut lab = Lab::for_bundle(Config::load().unwrap(), &bundle).unwrap();
    lab.test_candidates = true;
    let c = lab.test_candidates("b3");
    eprintln!("candidates b3: {}", c.len());
    let unkeyed = c.iter().find(|t| t.contains("Unkeyed")).expect("input from the unkeyed-member test");
    let r = lab.run(&RunInput::from_text(unkeyed), None).unwrap();
    let (o, n) = (outcome(&r["old"]), outcome(&r["new"]));
    eprintln!("unkeyed test input: old {o} | new {n}");
    assert!(o.contains("MsgPack004"), "{o}");
    assert!(n.contains("Unkeyed 42 -> 0"), "{n}");
    let out = lab.replay(&[]).unwrap();
    for x in out["replays"].as_array().unwrap() {
        eprintln!("REPLAY {} {} {} | {} | {}", x["difference"], x["state"], x["input"]["source"], x["old"], x["new"]);
    }
}

#[test]
fn replay_2234_issue_as_written() {
    let mut lab = lab_for("2234");
    let out = lab.replay(&[]).unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&out).unwrap());
    assert_eq!(out["issue"]["differs"], false, "old and new give the same diagnostic");
    assert_eq!(out["issue"]["old"], out["issue"]["new"]);
    assert!(out["replays"].as_array().unwrap().iter().all(|r| r["input"]["source"] != "issue"));
}
