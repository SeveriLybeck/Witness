//! Importance levels (D33), review mode (D47) and the decide list (D40).
//! Pure functions over contract records as `serde_json::Value`; thresholds come from `rules.json`.

use serde_json::{json, Value};
use std::path::Path;

/// Thresholds and kind lists, read from one config file (`rules.json`).
#[derive(Debug, Clone)]
pub struct Config {
    /// Reach rule fires when incoming call sites exceed this number.
    pub reach_sites: usize,
    pub severe_kinds: Vec<String>,
    pub architectural_kinds: Vec<String>,
    /// Worker fact kinds that fire `architectural` on the block holding the fact's location.
    pub fact_kinds: Vec<String>,
    /// Test categories that form a decide-list group and force full mode, in order.
    pub test_groups: Vec<String>,
}

pub const DEFAULT_CONFIG: &str = include_str!("../rules.json");

impl Config {
    pub fn parse(text: &str) -> Result<Config, String> {
        let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let strs = |k: &str| -> Result<Vec<String>, String> {
            v[k].as_array().ok_or(format!("config: {k} missing"))?
                .iter().map(|s| s.as_str().map(String::from).ok_or(format!("config: {k} not strings"))).collect()
        };
        Ok(Config {
            reach_sites: v["reachSites"].as_u64().ok_or("config: reachSites missing")? as usize,
            severe_kinds: strs("severeKinds")?,
            architectural_kinds: strs("architecturalKinds")?,
            fact_kinds: strs("factKinds")?,
            test_groups: strs("testGroups")?,
        })
    }
    pub fn load(path: &Path) -> Result<Config, String> {
        Config::parse(&std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?)
    }
}

impl Default for Config {
    fn default() -> Self { Config::parse(DEFAULT_CONFIG).expect("bundled rules.json") }
}

/// The records the rules read, each the whole file as JSON.
pub struct Inputs {
    pub blocks: Value,
    pub parts: Value,
    pub differences: Value,
    pub links: Value,
    pub tests: Value,
    pub scope: Value,
    pub dependencies: Value,
    /// The worker's `facts` list (`[]` when there is no `facts.json`).
    pub facts: Value,
}

impl Inputs {
    /// Read `blocks parts differences links tests scope dependencies` from a bundle folder.
    pub fn read_dir(dir: &Path) -> Result<Inputs, String> { Inputs::read_dirs(&[dir]) }
    /// Each record from the first folder that has it.
    pub fn read_dirs(dirs: &[&Path]) -> Result<Inputs, String> {
        let r = |n: &str| -> Result<Value, String> {
            let f = format!("{n}.json");
            let p = dirs.iter().map(|d| d.join(&f)).find(|p| p.is_file()).ok_or(format!("{f}: not found"))?;
            contracts::read_json(&p).map_err(|e| format!("{}: {}", e.path, e.message))
        };
        Ok(Inputs {
            blocks: r("blocks")?, parts: r("parts")?, differences: r("differences")?, links: r("links")?,
            tests: r("tests")?, scope: r("scope")?, dependencies: r("dependencies")?,
            facts: read_facts(dirs)?,
        })
    }
}

/// `facts.json` or `map-extra/facts.json` from the first folder that has one;
/// the worker reply `{id, method, result: {facts}}` or `{facts}`. Absent gives `[]`.
pub fn read_facts(dirs: &[&Path]) -> Result<Value, String> {
    let p = dirs.iter().flat_map(|d| [d.join("facts.json"), d.join("map-extra/facts.json")]).find(|p| p.is_file());
    let Some(p) = p else { return Ok(Value::Array(vec![])) };
    let v = contracts::read_json(&p).map_err(|e| format!("{}: {}", e.path, e.message))?;
    let f = if v["result"]["facts"].is_array() { &v["result"]["facts"] } else { &v["facts"] };
    Ok(if f.is_array() { f.clone() } else { Value::Array(vec![]) })
}

fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] { v[k].as_array().map(|a| a.as_slice()).unwrap_or(&[]) }
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn strings(v: &Value, k: &str) -> Vec<String> {
    arr(v, k).iter().filter_map(|x| x.as_str().map(String::from)).collect()
}

/// One block's level and the rules that fired on it (all of them, in table order).
#[derive(Debug, Clone, PartialEq)]
pub struct BlockLevel { pub block: String, pub level: u8, pub rules: Vec<String> }

/// The rule table: (name, level). Rules only raise; level 1 is the floor.
pub const RULE_TABLE: &[(&str, u8)] = &[
    ("undeclared-difference", 3),
    ("severe-difference", 3),
    ("architectural", 3),
    ("not-modelled-line", 3),
    ("difference", 2),
    ("reach", 2),
    ("more-differences-than-tests", 2),
    ("large-comment", 2),
];

fn link_of<'a>(inp: &'a Inputs, block: &str) -> Option<&'a Value> {
    arr(&inp.links, "links").iter().find(|l| s(l, "block") == block)
}
fn declared(inp: &Inputs, block: &str) -> bool { link_of(inp, block).is_some_and(|l| s(l, "state") == "declared") }
fn diffs_of<'a>(inp: &'a Inputs, block: &str) -> Vec<&'a Value> {
    arr(&inp.differences, "differences").iter().filter(|d| s(d, "block") == block).collect()
}

fn overlaps(loc: &Value, file: &str, lines: &[u64]) -> bool {
    let (a, b) = (loc["line"].as_u64().unwrap_or(0), loc["endLine"].as_u64().unwrap_or(0));
    s(loc, "file") == file && lines.iter().any(|l| *l >= a && *l <= b)
}

/// Rules that fire on one block, in table order.
pub fn fired(inp: &Inputs, cfg: &Config, block: &Value) -> Vec<String> {
    let id = s(block, "id");
    let parts: Vec<&Value> = strings(block, "parts").iter()
        .filter_map(|p| arr(&inp.parts, "parts").iter().find(|x| s(x, "id") == p)).collect();
    let diffs = diffs_of(inp, id);
    let decl = declared(inp, id);
    let undeclared = !decl && !diffs.is_empty();
    let severe = diffs.iter().any(|d| cfg.severe_kinds.iter().any(|k| k == s(d, "kind")));
    let new_file = parts.iter().any(|p| s(p, "category") == "production" && s(p, "kind") == "file" && p["old"].is_null());
    let architectural = new_file || diffs.iter().any(|d| cfg.architectural_kinds.iter().any(|k| k == s(d, "kind")));
    let mut fact_kinds: Vec<String> = vec![];
    for f in inp.facts.as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
        let (k, loc) = (s(f, "kind"), &f["location"]);
        if loc.is_null() || !cfg.fact_kinds.iter().any(|x| x == k) || fact_kinds.iter().any(|x| x == k) { continue; }
        let (a, b) = (loc["line"].as_u64().unwrap_or(0), loc["endLine"].as_u64().unwrap_or(0));
        let held = parts.iter().any(|p| {
            let n = &p["new"];
            !n.is_null() && s(n, "file") == s(loc, "file")
                && n["line"].as_u64().unwrap_or(0) <= b && n["endLine"].as_u64().unwrap_or(0) >= a
        });
        if held { fact_kinds.push(k.to_string()); }
    }
    let not_modelled = arr(&inp.scope, "notModelled").iter().any(|nm| {
        parts.iter().any(|p| {
            let n = &p["new"];
            let mut lines: Vec<u64> = arr(p, "addedLines").iter().filter_map(|l| l.as_u64()).collect();
            if !n.is_null() { lines.extend(n["line"].as_u64()); lines.extend(n["endLine"].as_u64()); }
            !n.is_null() && overlaps(&nm["location"], s(n, "file"), &lines)
        })
    });
    // Who uses this block: sites of edges from it (edge A->B means B uses A),
    // plus the worker's `reach.callSites` (uses outside the block) when present.
    let reach: usize = arr(&inp.dependencies, "dependencies").iter()
        .filter(|d| s(d, "from") == id).map(|d| arr(d, "sites").len()).sum::<usize>()
        + arr(&inp.dependencies, "reach").iter().filter(|r| s(r, "block") == id)
            .filter_map(|r| r["callSites"].as_u64()).sum::<u64>() as usize;
    let tests = link_of(inp, id).map(|l| arr(l, "tests").len()).unwrap_or(0);
    let large_comment = parts.iter().any(|p| p["largeComment"].as_bool() == Some(true));
    let on = [
        undeclared,
        severe && !undeclared,
        architectural || !fact_kinds.is_empty(),
        not_modelled,
        !diffs.is_empty() && !undeclared && !severe,
        reach > cfg.reach_sites,
        decl && diffs.len() > tests,
        large_comment,
    ];
    let mut out = vec![];
    for ((n, _), f) in RULE_TABLE.iter().zip(on) {
        if !f { continue; }
        out.push(n.to_string());
        if *n == "architectural" { out.extend(fact_kinds.iter().map(|k| format!("architectural:{k}"))); }
    }
    out
}

/// Level of a rule list; `name:detail` entries count as `name`.
pub fn level_of(rules: &[String]) -> u8 {
    rules.iter().filter_map(|r| RULE_TABLE.iter().find(|(n, _)| *n == r.split(':').next().unwrap_or("")).map(|(_, l)| *l)).fold(1, u8::max)
}

pub fn levels(inp: &Inputs, cfg: &Config) -> Vec<BlockLevel> {
    arr(&inp.blocks, "blocks").iter().map(|b| {
        let rules = fired(inp, cfg, b);
        BlockLevel { block: s(b, "id").into(), level: level_of(&rules), rules }
    }).collect()
}

/// `mode.json` rules: `level-3-block`, then `<category>-tests` per test group present.
pub fn mode(levels: &[BlockLevel], inp: &Inputs, cfg: &Config) -> (&'static str, Vec<String>) {
    let mut rules = vec![];
    if levels.iter().any(|l| l.level == 3) { rules.push("level-3-block".to_string()); }
    for g in &cfg.test_groups {
        if arr(&inp.tests, "tests").iter().any(|t| s(t, "category") == g) { rules.push(format!("{g}-tests")); }
    }
    (if rules.is_empty() { "light" } else { "full" }, rules)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Question { pub id: String, pub rule: String, pub block: Option<String>, pub differences: Vec<String>, pub tests: Vec<String> }

/// The decide list (D40): one question per undeclared difference, per declared severe difference,
/// per other level-3 block, per group of hanging, withdrawn or not-run tests. Answers: accept or return.
pub fn questions(levels: &[BlockLevel], inp: &Inputs, cfg: &Config) -> Vec<Question> {
    let mut q: Vec<Question> = vec![];
    for d in arr(&inp.differences, "differences") {
        let b = s(d, "block");
        let rule = if !declared(inp, b) { "undeclared-difference" }
            else if cfg.severe_kinds.iter().any(|k| k == s(d, "kind")) { "declared-severe-difference" }
            else { continue };
        let tests = link_of(inp, b).map(|l| strings(l, "tests")).unwrap_or_default();
        q.push(Question { id: String::new(), rule: rule.into(), block: Some(b.into()), differences: vec![s(d, "id").into()], tests });
    }
    for l in levels.iter().filter(|l| l.level == 3) {
        if !q.iter().any(|x| x.block.as_deref() == Some(&l.block)) {
            q.push(Question { id: String::new(), rule: "level-3-block".into(), block: Some(l.block.clone()), differences: vec![], tests: vec![] });
        }
    }
    for g in &cfg.test_groups {
        let tests: Vec<String> = arr(&inp.tests, "tests").iter().filter(|t| s(t, "category") == g).map(|t| s(t, "name").to_string()).collect();
        if !tests.is_empty() {
            q.push(Question { id: String::new(), rule: "test-group".into(), block: None, differences: vec![], tests });
        }
    }
    for (i, x) in q.iter_mut().enumerate() { x.id = format!("q{}", i + 1); }
    q
}

/// The three output records, as written to `levels.json`, `mode.json`, `questions.json`.
pub fn run(inp: &Inputs, cfg: &Config) -> (Value, Value, Value) {
    let lv = levels(inp, cfg);
    let (m, mr) = mode(&lv, inp, cfg);
    let qs = questions(&lv, inp, cfg);
    (
        json!({"provenance": "pipeline", "levels": lv.iter().map(|l| json!({"block": l.block, "level": l.level, "rules": l.rules.iter().map(|r| match r.split_once(':') {
            Some((rule, fact)) => json!({"rule": rule, "fact": fact}),
            None => json!(r),
        }).collect::<Vec<_>>()})).collect::<Vec<_>>()}),
        json!({"provenance": "pipeline", "mode": m, "rules": mr}),
        json!({"provenance": "pipeline", "questions": qs.iter().map(|q| json!({"id": q.id, "rule": q.rule, "block": q.block, "differences": q.differences, "tests": q.tests})).collect::<Vec<_>>()}),
    )
}
