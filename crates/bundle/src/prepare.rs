//! `witness prepare`: runs pipeline modules as programs, imports and validates their records,
//! derives differences, replays and the boundary, and keeps section states (D4, D42).

use crate::derive;
use contracts::{section_records, Schemas, BUNDLE_RECORDS};
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// One pipeline module: a program run as `command... <stageDir> <bundleDir> <repo> <old> <new>`.
/// It writes contract records (`<record>.json`) into `<stageDir>`; it may read earlier records from `<bundleDir>`.
#[derive(Clone, Debug)]
pub struct Module {
    pub name: String,
    pub section: String,
    pub command: Vec<String>,
    pub budget: Duration,
}

/// Sections in the order they run. `tests` and `rules` form the test contract and run in `finish`.
/// Staged files that are not contract records: non-solver differences, the worker's facts, edited snapshots.
const EXTRA: [&str; 3] = ["differences", "facts", "snapshots"];

pub const STAGES:[&str; 6] = ["map", "proof", "playground", "review", "tests", "rules"];

fn default_budget(section: &str) -> u64 {
    match section {
        "map" => 30,
        "proof" => 60,
        "tests" => 900,
        _ => 30,
    }
}

/// Read `{"modules": [{name, section, command: [..], budgetSeconds?}]}`.
pub fn load_modules(path: &Path) -> Result<Vec<Module>, String> {
    let v: Value = serde_json::from_str(&fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (i, m) in v["modules"].as_array().ok_or("modules: not an array")?.iter().enumerate() {
        let section = m["section"].as_str().filter(|s| STAGES.contains(s)).ok_or(format!("modules/{i}/section: unknown"))?.to_string();
        let command: Vec<String> = m["command"].as_array().ok_or(format!("modules/{i}/command: not an array"))?
            .iter().filter_map(|x| x.as_str().map(String::from)).collect();
        if command.is_empty() {
            return Err(format!("modules/{i}/command: empty"));
        }
        out.push(Module {
            name: m["name"].as_str().unwrap_or(&section).to_string(),
            budget: Duration::from_secs_f64(m["budgetSeconds"].as_f64().unwrap_or(default_budget(&section) as f64)),
            section,
            command,
        });
    }
    Ok(out)
}

#[derive(Clone, Debug, Default)]
pub struct Config {
    /// `<data>`: bundles go to `<data>/bundles/<old>-<new>/`, stage dirs and logs beside.
    pub data: PathBuf,
    pub repo: PathBuf,
    pub old: String,
    pub new: String,
    pub modules: Vec<Module>,
    /// Folder of `issue-<n>.json` (GitHub issue JSON); `None` → requirement null.
    pub issues: Option<PathBuf>,
    /// Pull request description; `None` → the new commit's message.
    pub description: Option<String>,
    pub pull_request: Option<u64>,
    /// Repository name; `None` → the repo folder's name.
    pub repository: Option<String>,
}

pub struct Builder {
    cfg: Config,
    pub dir: PathBuf,
    stage: PathBuf,
    logs: PathBuf,
    schemas: Schemas,
    bundle: Value,
}

fn is_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    let o = Command::new("git").arg("-c").arg("core.hooksPath=/dev/null").arg("-C").arg(repo).args(args)
        .stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).to_string())
}

/// Write `v` to `<dir>/<name>.json` through a temporary file and a rename.
pub fn write_atomic(dir: &Path, name: &str, v: &Value) -> Result<(), String> {
    let tmp = dir.join(format!(".{name}.json.tmp"));
    let text = serde_json::to_string_pretty(v).map_err(|e| e.to_string())? + "\n";
    fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, dir.join(format!("{name}.json"))).map_err(|e| format!("{name}.json: {e}"))
}

fn issue_number(text: &str) -> Option<String> {
    let i = text.find('#')?;
    let n: String = text[i + 1..].chars().take_while(|c| c.is_ascii_digit()).collect();
    (!n.is_empty()).then_some(n)
}

fn with_pipeline(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.insert("provenance".into(), json!("pipeline"));
    }
    v
}

impl Builder {
    /// Resolve commits, create the bundle directory and write `bundle.json` with every section `running`.
    pub fn start(cfg: Config) -> Result<Builder, String> {
        let mut cfg = cfg;
        for c in [&mut cfg.old, &mut cfg.new] {
            if !is_sha(c) {
                *c = git(&cfg.repo, &["rev-parse", "--verify", &format!("{c}^{{commit}}")])
                    .map(|s| s.trim().to_string()).filter(|s| is_sha(s)).ok_or(format!("cannot resolve commit `{c}`"))?;
            }
        }
        let key = format!("{}-{}", cfg.old, cfg.new);
        let dir = cfg.data.join("bundles").join(&key);
        let stage = cfg.data.join("stage").join(&key);
        let logs = cfg.data.join("logs").join(&key);
        for d in [&dir, &logs] {
            fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        for rec in BUNDLE_RECORDS {
            let _ = fs::remove_file(dir.join(format!("{rec}.json")));
        }
        let _ = fs::remove_dir_all(&stage);
        let _ = fs::remove_file(dir.join("facts.json"));
        for extra in ["map-extra", "tests-extra"] {
            let _ = fs::remove_dir_all(dir.join(extra));
        }
        let description = cfg.description.clone()
            .or_else(|| git(&cfg.repo, &["log", "-1", "--format=%B", &cfg.new]).map(|s| s.trim_end().to_string()))
            .unwrap_or_default();
        let requirement = match (&cfg.issues, issue_number(&description)) {
            (Some(d), Some(n)) => fs::read_to_string(d.join(format!("issue-{n}.json"))).ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .map(|i| json!({"id": n, "title": i["title"].as_str().unwrap_or(""), "text": i["body"].as_str().unwrap_or("")}))
                .unwrap_or(Value::Null),
            _ => Value::Null,
        };
        let repository = cfg.repository.clone().unwrap_or_else(|| {
            cfg.repo.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "repository".into())
        });
        let running = json!({"state": "running"});
        let bundle = json!({
            "provenance": "pipeline", "repository": repository, "pullRequest": cfg.pull_request,
            "oldCommit": cfg.old, "newCommit": cfg.new, "requirement": requirement, "description": description,
            "sections": {"proof": running, "map": running, "playground": running, "tests": running, "rules": running, "review": running},
        });
        let b = Builder { schemas: Schemas::contract().map_err(|e| e.to_string())?, cfg, dir, stage, logs, bundle };
        b.save()?;
        Ok(b)
    }

    /// Reopen an existing bundle (for `finish`).
    pub fn reopen(cfg: Config) -> Result<Builder, String> {
        let key = format!("{}-{}", cfg.old, cfg.new);
        let dir = cfg.data.join("bundles").join(&key);
        let bundle = contracts::read_json(&dir.join("bundle.json")).map_err(|e| e.to_string())?;
        Ok(Builder { schemas: Schemas::contract().map_err(|e| e.to_string())?, stage: cfg.data.join("stage").join(&key),
            logs: cfg.data.join("logs").join(&key), cfg, dir, bundle })
    }

    pub fn bundle_old(&self) -> String {
        self.cfg.old.clone()
    }
    pub fn bundle_new(&self) -> String {
        self.cfg.new.clone()
    }

    pub fn log(&self, msg: &str) {
        eprintln!("witness: {msg}");
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(self.logs.join("witness.log")) {
            let _ = writeln!(f, "{msg}");
        }
    }

    fn save(&self) -> Result<(), String> {
        write_atomic(&self.dir, "bundle", &self.bundle)
    }

    pub fn state(&self, section: &str) -> String {
        self.bundle["sections"][section]["state"].as_str().unwrap_or("").to_string()
    }

    fn set(&mut self, section: &str, state: &str, detail: Option<String>) -> Result<(), String> {
        let mut st = json!({"state": state});
        if let Some(d) = detail {
            self.log(&format!("{section}: {state}: {d}"));
            st["detail"] = json!(d);
        }
        self.bundle["sections"][section] = st;
        self.save()
    }

    /// Run every module of `section`; return the records they wrote, or the first failure.
    fn run_section(&self, section: &str) -> Result<Option<Vec<(String, Value)>>, String> {
        let mods: Vec<&Module> = self.cfg.modules.iter().filter(|m| m.section == section).collect();
        if mods.is_empty() {
            return Ok(None);
        }
        let allowed: Vec<&str> = section_records(section).iter().copied()
            .chain(match section {
                "proof" => vec!["differences", "facts"],
                "map" => vec!["facts"],
                "tests" => vec!["snapshots"],
                _ => vec![],
            }).collect();
        let mut out = Vec::new();
        for m in mods {
            let sd = self.stage.join(&m.name);
            let _ = fs::remove_dir_all(&sd);
            fs::create_dir_all(&sd).map_err(|e| e.to_string())?;
            let log = fs::File::create(self.logs.join(format!("{}.log", m.name))).map_err(|e| e.to_string())?;
            let t0 = Instant::now();
            let mut child = Command::new(&m.command[0]).args(&m.command[1..])
                .arg(&sd).arg(&self.dir).arg(&self.cfg.repo).arg(&self.cfg.old).arg(&self.cfg.new)
                .env("WITNESS_STAGE", &sd).env("WITNESS_BUNDLE", &self.dir).env("WITNESS_REPO", &self.cfg.repo)
                .stdin(Stdio::null()).stdout(log.try_clone().map_err(|e| e.to_string())?).stderr(log)
                .spawn().map_err(|e| format!("module {}: {e}", m.name))?;
            let status = loop {
                if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
                    break st;
                }
                if t0.elapsed() > m.budget {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("module {}: over time budget {:?}", m.name, m.budget));
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            self.log(&format!("module {} ({section}): {status} in {:.2}s", m.name, t0.elapsed().as_secs_f64()));
            if !status.success() {
                return Err(format!("module {}: {status}", m.name));
            }
            for rec in &allowed {
                let p = sd.join(format!("{rec}.json"));
                if p.exists() {
                    let v = contracts::read_json(&p).map_err(|e| format!("module {}: {e}", m.name))?;
                    if !EXTRA.contains(rec) {
                        self.schemas.validate(rec, &v, &format!("{rec}.json")).map_err(|e| format!("module {}: {e}", m.name))?;
                    }
                    out.push((rec.to_string(), if *rec == "facts" { v } else { with_pipeline(v) }));
                }
            }
        }
        Ok(Some(out))
    }

    fn get(recs: &[(String, Value)], name: &str) -> Option<Value> {
        recs.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.clone())
    }

    fn read(&self, name: &str) -> Value {
        contracts::read_json(&self.dir.join(format!("{name}.json"))).unwrap_or(Value::Null)
    }

    fn put(&self, name: &str, v: &Value) -> Result<(), String> {
        self.schemas.validate(name, v, &format!("{name}.json")).map_err(|e| e.to_string())?;
        write_atomic(&self.dir, name, v)
    }

    /// Run one section and record its state. Errors inside a section never stop the others.
    pub fn section(&mut self, section: &str) -> Result<(), String> {
        let result = self.run_section(section).and_then(|recs| match recs {
            None => Ok(Some("no module".to_string())),
            Some(recs) => self.import(section, recs).map(|_| None),
        });
        match (section, result) {
            ("review", Ok(None)) if self.read("ai-review")["recording"].is_null() => self.set("review", "unavailable", Some("no recording".into())),
            (_, Ok(None)) => self.set(section, "ready", None),
            (_, Ok(Some(d))) => self.set(section, "unavailable", Some(d)),
            (_, Err(e)) => self.set(section, "not-run", Some(e)),
        }
    }

    fn import(&self, section: &str, recs: Vec<(String, Value)>) -> Result<(), String> {
        match section {
            "map" => {
                for rec in ["files", "parts", "blocks"] {
                    Self::get(&recs, rec).ok_or(format!("no {rec}.json"))?;
                }
                for (n, v) in &recs {
                    if n == "facts" {
                        let d = self.dir.clone();
                        fs::create_dir_all(&d).map_err(|e| e.to_string())?;
                        write_atomic(&d, "facts", v)?;
                    } else {
                        self.put(n, v)?;
                    }
                }
            }
            "proof" => {
                if self.state("map") != "ready" {
                    return Err("map not ready".into());
                }
                let (parts, blocks) = (self.read("parts"), self.read("blocks"));
                let rows = Self::get(&recs, "rows").unwrap_or(json!({"rows": []}));
                let mut scope = Self::get(&recs, "scope").ok_or("no scope.json")?;
                let extra: Vec<Value> = Self::get(&recs, "differences").and_then(|d| d["differences"].as_array().cloned()).unwrap_or_default()
                    .into_iter().filter(|d| d["checker"].as_str() != Some("solver")).collect();
                let staged = Self::get(&recs, "facts");
                if let Some(f) = &staged {
                    write_atomic(&self.dir, "facts", f)?;
                }
                let facts = staged.or_else(|| contracts::read_json(&self.dir.join("facts.json")).ok());
                let mut extra = extra;
                if let Some(f) = facts {
                    extra.extend(derive::api_differences(&f, &parts, &blocks));
                }
                let g = derive::group_rows(&rows, &extra, &parts, &blocks)?;
                derive::remap_rows(&mut scope, &g.row_ids);
                let lean = Self::get(&recs, "lean").map(|mut l| { derive::remap_rows(&mut l, &g.row_ids); l });
                // Validate everything before writing anything.
                for (n, v) in [("rows", &g.rows), ("differences", &g.differences), ("scope", &scope)] {
                    self.schemas.validate(n, v, &format!("{n}.json")).map_err(|e| e.to_string())?;
                }
                self.put("rows", &g.rows)?;
                self.put("scope", &scope)?;
                if let Some(l) = lean {
                    self.put("lean", &l)?;
                }
                self.put("differences", &g.differences)?;
            }
            "playground" => {
                let replays = Self::get(&recs, "replays").unwrap_or(json!({"replays": []}));
                let mut diffs = if self.state("proof") == "ready" { self.read("differences") } else { json!({"differences": []}) };
                let out = derive::apply_replays(&mut diffs, &replays);
                self.put("replays", &out)?;
                if self.state("proof") == "ready" {
                    self.put("differences", &diffs)?;
                }
            }
            "review" => {
                let r = Self::get(&recs, "ai-review").ok_or("no ai-review.json")?;
                self.put("ai-review", &r)?;
                if let Err(e) = self.validate() {
                    if e.starts_with("ai-review") {
                        let _ = fs::remove_file(self.dir.join("ai-review.json"));
                        return Err(e);
                    }
                }
            }
            _ => unreachable!("tests and rules import in finish"),
        }
        Ok(())
    }

    /// Prepared: proof, map and playground are `ready` (Terms).
    pub fn prepared(&self) -> bool {
        ["proof", "map", "playground"].iter().all(|s| self.state(s) == "ready")
    }

    /// The test contract (tests, links, then levels, questions, mode, then states), then the AI review (it cites tests, so it follows them; D46).
    pub fn finish(&mut self) -> Result<(), String> {
        self.test_contract()?;
        self.section("review")
    }

    /// Edited snapshots become `output-changed` differences appended after the existing ones (IDs stay),
    /// each with a `not-replayed` replay entry. Needs proof `ready`; otherwise skipped with a log line.
    fn add_snapshot_differences(&self, snaps: &Value, blocks: &Value) -> Result<(), String> {
        let new = derive::snapshot_differences(snaps, &self.read("parts"), blocks);
        if new.is_empty() {
            return Ok(());
        }
        if self.state("proof") != "ready" {
            self.log(&format!("snapshots: {} output-changed entries skipped, proof not ready", new.len()));
            return Ok(());
        }
        let mut diffs = self.read("differences");
        let ids = derive::append_differences(&mut diffs, new);
        self.log(&format!("snapshots: added {}", ids.join(", ")));
        self.schemas.validate("differences", &diffs, "differences.json").map_err(|e| e.to_string())?;
        if self.state("playground") == "ready" {
            let replays = derive::apply_replays(&mut diffs, &self.read("replays"));
            self.put("replays", &replays)?;
        }
        self.put("differences", &diffs)
    }

    fn test_contract(&mut self) -> Result<(), String> {
        let tests = self.run_section("tests").and_then(|r| match r {
            None => Ok(None),
            Some(recs) => {
                if self.state("map") != "ready" {
                    return Err("map not ready".into());
                }
                let t = Self::get(&recs, "tests").ok_or("no tests.json")?;
                let mut l = Self::get(&recs, "links").ok_or("no links.json")?;
                let blocks = self.read("blocks");
                derive::check_links(&l, &blocks)?;
                let snaps = Self::get(&recs, "snapshots")
                    .or_else(|| contracts::read_json(&self.dir.join("tests-extra/snapshots.json")).ok());
                if let Some(sn) = snaps {
                    self.add_snapshot_differences(&sn, &blocks)?;
                }
                derive::place_links(&mut l, &self.read("differences"), &self.read("scope"), &self.read("parts"), &blocks);
                self.schemas.validate("tests", &t, "tests.json").map_err(|e| e.to_string())?;
                self.schemas.validate("links", &l, "links.json").map_err(|e| e.to_string())?;
                Ok(Some((t, l)))
            }
        });
        let (t, l) = match tests {
            Ok(Some(x)) => x,
            Ok(None) => return self.set("tests", "unavailable", Some("no module".into())).and_then(|_| self.set("rules", "running", Some("tests not ready".into()))),
            Err(e) => return self.set("tests", "not-run", Some(e)).and_then(|_| self.set("rules", "running", Some("tests not ready".into()))),
        };
        // Rules modules read tests and links from the stage of the test contract via the bundle dir,
        // so these are written first (the order the spec gives).
        self.put("tests", &t)?;
        self.put("links", &l)?;
        let rules = self.run_section("rules").and_then(|r| match r {
            None => Ok(None),
            Some(recs) => {
                let mut out = Vec::new();
                for rec in ["levels", "questions", "mode"] {
                    out.push((rec, Self::get(&recs, rec).ok_or(format!("no {rec}.json"))?));
                }
                Ok(Some(out))
            }
        });
        match rules {
            Ok(Some(out)) => {
                for (n, v) in &out {
                    self.put(n, v)?;
                }
                self.bundle["sections"]["tests"] = json!({"state": "ready"});
                self.set("rules", "ready", None)
            }
            Ok(None) => {
                self.bundle["sections"]["tests"] = json!({"state": "ready"});
                self.set("rules", "unavailable", Some("no module".into()))
            }
            Err(e) => {
                self.bundle["sections"]["tests"] = json!({"state": "ready"});
                self.set("rules", "not-run", Some(e))
            }
        }
    }

    /// Validate the bundle as it stands against schemas and invariants.
    pub fn validate(&self) -> Result<(), String> {
        contracts::validate_bundle(&self.dir, &contracts::Options::default()).map_err(|e| e.to_string())
    }
}

/// Everything up to "prepared": bundle.json, then map, proof, playground, review. `tests` and `rules` stay `running`.
pub fn prepare(cfg: Config) -> Result<Builder, String> {
    let mut b = Builder::start(cfg)?;
    for s in ["map", "proof", "playground"] {
        b.section(s)?;
    }
    if let Err(e) = b.validate() {
        b.log(&format!("validate: {e}"));
    }
    Ok(b)
}
