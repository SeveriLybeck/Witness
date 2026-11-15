use rules::*;
use serde_json::{json, Value};
use std::path::PathBuf;

fn root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..") }
fn fixture(n: &str) -> PathBuf { root().join("contracts/fixtures").join(n) }
fn read(p: PathBuf) -> Value { serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap() }
fn strip(mut v: Value) -> Value { v.as_object_mut().unwrap().remove("provenance"); v }
fn out(n: &str) -> (Value, Value, Value) { run(&Inputs::read_dir(&fixture(n)).unwrap(), &Config::default()) }

#[test]
fn fallbacks_line_one_is_contract_hash() {
    let o = std::process::Command::new("git").args(["log", "-1", "--format=%H", "--", "contracts/"])
        .current_dir(root()).output().unwrap();
    let hash = String::from_utf8(o.stdout).unwrap().trim().to_string();
    let text = std::fs::read_to_string(root().join("crates/rules/FALLBACKS.md")).unwrap();
    assert_eq!(text.lines().next().unwrap().trim(), hash);
}

#[test]
fn outputs_validate_on_all_fixtures() {
    let s = contracts::Schemas::contract().map_err(|f| f.message).unwrap();
    for n in ["2283", "2234", "2297", "2129"] {
        let (l, m, q) = out(n);
        s.validate("levels", &l, n).map_err(|f| format!("{}: {}", f.path, f.message)).unwrap();
        s.validate("mode", &m, n).map_err(|f| format!("{}: {}", f.path, f.message)).unwrap();
        s.validate("questions", &q, n).map_err(|f| format!("{}: {}", f.path, f.message)).unwrap();
    }
}

#[test]
fn levels_2283_match_fixture() {
    assert_eq!(strip(out("2283").0), strip(read(fixture("2283/levels.json"))));
}

#[test]
fn questions_and_mode_2283_match_fixture() {
    let (_, m, q) = out("2283");
    assert_eq!(strip(q), strip(read(fixture("2283/questions.json"))));
    assert_eq!(strip(m), strip(read(fixture("2283/mode.json"))));
}

#[test]
fn mode_2234_full_for_hanging_tests() {
    let (_, m, q) = out("2234");
    assert_eq!(strip(m), strip(read(fixture("2234/mode.json"))));
    assert_eq!(strip(q), strip(read(fixture("2234/questions.json"))));
}

#[test]
fn level_numbers_mode_questions_match_other_fixtures() {
    // Hand-written fixtures differ in some rule names (see STATUS.md); levels themselves must match.
    let nums = |v: Value| -> Vec<(Value, Value)> {
        v["levels"].as_array().unwrap().iter().map(|b| (b["block"].clone(), b["level"].clone())).collect()
    };
    for n in ["2234", "2297", "2129"] {
        let (l, m, q) = out(n);
        assert_eq!(nums(l), nums(read(fixture(n).join("levels.json"))), "{n} levels");
        assert_eq!(strip(m), strip(read(fixture(n).join("mode.json"))), "{n} mode");
        assert_eq!(strip(q), strip(read(fixture(n).join("questions.json"))), "{n} questions");
    }
}

/// 2283 with the solver's differences removed and the tests declared: no level 3, light, empty list.
#[test]
fn no_level_3_is_light_and_list_is_empty() {
    let mut inp = Inputs::read_dir(&fixture("2283")).unwrap();
    inp.differences = json!({"differences": []});
    for p in inp.parts["parts"].as_array_mut().unwrap() { p["largeComment"] = json!(false); }
    inp.dependencies.as_object_mut().unwrap().remove("reach");
    let (l, m, q) = run(&inp, &Config::default());
    assert!(l["levels"].as_array().unwrap().iter().all(|b| b["level"] == 1));
    assert_eq!(m, json!({"provenance": "pipeline", "mode": "light", "rules": []}));
    assert_eq!(q["questions"], json!([]));
}

#[test]
fn binary_module_command_line_on_2283() {
    let base = std::env::temp_dir().join(format!("rules-stage-{}", std::process::id()));
    let stage = base.join("nested");
    let _ = std::fs::remove_dir_all(&base);
    let st = std::process::Command::new(env!("CARGO_BIN_EXE_rules"))
        .arg(&stage).arg(fixture("2283")).args(["/nonexistent", "old", "new"]).status().unwrap();
    assert!(st.success());
    for n in ["levels", "mode", "questions"] {
        assert_eq!(strip(read(stage.join(format!("{n}.json")))), strip(read(fixture("2283").join(format!("{n}.json")))), "{n}");
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn reach_call_sites_raise_b1_on_2283() {
    let mut inp = Inputs::read_dir(&fixture("2283")).unwrap();
    inp.dependencies["reach"] = json!([{"block": "b1", "callSites": 28}]);
    let cfg = Config::default();
    assert!(fired(&inp, &cfg, &inp.blocks["blocks"][0].clone()).contains(&"reach".to_string()));
    assert!(!fired(&inp, &cfg, &inp.blocks["blocks"][1].clone()).contains(&"reach".to_string()));
}

#[test]
fn rules_only_raise_and_threshold_from_config() {
    assert_eq!(level_of(&[]), 1);
    assert_eq!(level_of(&["large-comment".into(), "severe-difference".into()]), 3);
    let mut inp = Inputs::read_dir(&fixture("2283")).unwrap();
    let mut cfg = Config::default();
    cfg.reach_sites = 0;
    let has = |inp: &Inputs, cfg: &Config, i: usize, r: &str| fired(inp, cfg, &inp.blocks["blocks"][i].clone()).contains(&r.to_string());
    // Edges b1->b2, b1->b3: b2 and b3 use b1, so b1 has reach 2 and b2 has none.
    assert!(has(&inp, &cfg, 0, "reach"));
    assert!(!has(&inp, &cfg, 1, "reach"));
    inp.dependencies = json!({"dependencies": []});
    assert!(!has(&inp, &cfg, 0, "reach"));
    assert!(level_of(&["architectural:new-file".into()]) == 3);
}

/// Real 2129 b15: reach.callSites 1 with many incoming edges must not fire reach.
#[test]
fn incoming_edges_do_not_count_as_reach() {
    let mut inp = Inputs::read_dir(&fixture("2283")).unwrap();
    let site = json!({"file": "x.cs", "line": 1, "endLine": 1});
    inp.dependencies = json!({
        "dependencies": (0..7).map(|_| json!({"from": "b1", "to": "b2", "version": "new", "via": "value", "changed": true, "sites": [site]})).collect::<Vec<_>>(),
        "reach": [{"block": "b2", "callSites": 1}]});
    assert!(!fired(&inp, &Config::default(), &inp.blocks["blocks"][1].clone()).contains(&"reach".to_string()));
}

#[test]
fn facts_fire_architectural_2283_changed_wiring() {
    let mut inp = Inputs::read_dir(&fixture("2283")).unwrap();
    inp.facts = read_facts(&[&fixture("2283/inputs")]).unwrap();
    assert_eq!(inp.facts.as_array().unwrap().len(), 0, "inputs folder holds worker-reply.facts.json, not facts.json");
    let reply = read(fixture("2283/inputs/worker-reply.facts.json"));
    inp.facts = reply["result"]["facts"].clone();
    let r = fired(&inp, &Config::default(), &inp.blocks["blocks"][0].clone());
    assert!(r.contains(&"architectural".to_string()) && r.contains(&"architectural:changed-wiring".to_string()), "{r:?}");
    assert!(!fired(&inp, &Config::default(), &inp.blocks["blocks"][1].clone()).contains(&"architectural".to_string()));
}

#[test]
fn facts_json_read_from_bundle_dir() {
    let d = std::env::temp_dir().join(format!("rules-facts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("map-extra")).unwrap();
    std::fs::copy(fixture("2283/inputs/worker-reply.facts.json"), d.join("map-extra/facts.json")).unwrap();
    assert_eq!(read_facts(&[&d]).unwrap().as_array().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn facts_fire_architectural_2129_new_file_and_diagnostics() {
    let mut inp = Inputs::read_dir(&fixture("2129")).unwrap();
    let an = "src/MessagePack.SourceGenerator/Analyzers/MsgPack00xMessagePackAnalyzer.cs";
    let col = "src/MessagePack.SourceGenerator/CodeAnalysis/GeneratedFormatterCollector.cs";
    let mut facts = vec![json!({"kind": "new-file", "location": {"file": col, "line": 1, "endLine": 300}})];
    for l in [32, 33, 34, 35] { facts.push(json!({"kind": "new-diagnostic", "location": {"file": an, "line": l, "endLine": l}})); }
    inp.facts = json!(facts);
    let cfg = Config::default();
    let holds = |b: &Value, file: &str| b["parts"].as_array().unwrap().iter().any(|p| {
        inp.parts["parts"].as_array().unwrap().iter().any(|x| x["id"] == *p && x["new"]["file"] == file)
    });
    let mut seen = (false, false);
    for b in inp.blocks["blocks"].as_array().unwrap() {
        let r = fired(&inp, &cfg, b);
        if holds(b, col) && r.contains(&"architectural:new-file".to_string()) { seen.0 = true; }
        if holds(b, an) && r.contains(&"architectural:new-diagnostic".to_string()) { seen.1 = true; }
    }
    assert!(seen.0 && seen.1);
}
