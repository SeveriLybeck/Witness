//! Generator lab (D34, D37): the plugin side of the C# worker's two-version host, its presets,
//! consumer inputs, and the replay runner that decides `seen-live` / `not-replayed` (D23).

use serde_json::{json, Map, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// Largest accepted code input (P3).
pub const MAX_INPUT: usize = 64 * 1024;
/// Time allowed for one worker request before the worker is restarted (P3; round trips run up to 30 s in the worker).
pub const WORKER_TIMEOUT: Duration = Duration::from_secs(60);

pub fn plugin_dir() -> PathBuf {
    std::env::var_os("GENERATOR_LAB_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

fn read_json(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

// ---------------------------------------------------------------- configuration

/// The manifest, the repository adapter (`messagepack.json`) and the paths they name.
pub struct Config {
    pub dir: PathBuf,
    pub repo: PathBuf,
    pub manifest: Value,
    pub adapter: Value,
}

impl Config {
    pub fn load() -> Result<Config, String> {
        Config::load_from(&plugin_dir())
    }

    pub fn load_from(dir: &Path) -> Result<Config, String> {
        let manifest = read_json(&dir.join("plugin.json")).ok_or("plugin.json unreadable")?;
        let adapter = manifest.get("config").and_then(Value::as_str).and_then(|c| read_json(&dir.join(c))).unwrap_or(json!({}));
        let repo = dir.join("../..").canonicalize().unwrap_or_else(|_| dir.join("../.."));
        Ok(Config { dir: dir.to_path_buf(), repo, manifest, adapter })
    }

    /// Path templates: `$<env>` (colon separated) first, then the adapter's list; relative to the repository root.
    fn templates(&self, env: &str, key: &str) -> Vec<String> {
        let mut out: Vec<String> = std::env::var(env).map(|v| v.split(':').filter(|x| !x.is_empty()).map(String::from).collect()).unwrap_or_default();
        out.extend(self.adapter[key].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)));
        out.into_iter().map(|t| if t.starts_with('/') { t } else { self.repo.join(t).display().to_string() }).collect()
    }

    fn first_in(&self, base: &Path, key: &str) -> Option<PathBuf> {
        self.adapter[key].as_array()?.iter().filter_map(Value::as_str).map(|r| base.join(r)).find(|p| p.exists())
    }

    /// `{generator, runtime}` of a built working copy, as the worker's `run` takes it.
    pub fn side(&self, tree: &Path) -> Option<Value> {
        let g = self.first_in(tree, "generator")?;
        let r = self.first_in(tree, "runtime")?;
        Some(json!({"generator": g.display().to_string(), "runtime": r.display().to_string()}))
    }

    fn find(&self, env: &str, key: &str, fill: &dyn Fn(&str) -> String) -> Option<Value> {
        self.templates(env, key).iter().map(|t| PathBuf::from(fill(t))).find_map(|p| self.side(&p))
    }

    /// The built working copy of one commit.
    pub fn tree(&self, commit: &str) -> Option<Value> {
        self.find("GENERATOR_LAB_TREES", "trees", &|t| t.replace("{commit}", commit))
    }

    /// The folder of the built working copy of one commit.
    pub fn tree_dir(&self, commit: &str) -> Option<PathBuf> {
        self.templates("GENERATOR_LAB_TREES", "trees").iter().map(|t| PathBuf::from(t.replace("{commit}", commit))).find(|p| self.side(p).is_some())
    }

    /// The kept reverted build of one block (D27).
    pub fn reverted(&self, new_commit: &str, block: &str) -> Option<Value> {
        let n12 = &new_commit[..new_commit.len().min(12)];
        self.find("GENERATOR_LAB_REVERTED", "reverted", &|t| t.replace("{new12}", n12).replace("{block}", block).replace("{commit}", new_commit))
    }

    pub fn worker_dll(&self) -> Option<PathBuf> {
        self.templates("GENERATOR_LAB_WORKER", "worker").into_iter().map(PathBuf::from).find(|p| p.exists())
    }

    pub fn nuget(&self) -> Option<PathBuf> {
        if let Some(n) = std::env::var_os("NUGET_PACKAGES") {
            return Some(n.into());
        }
        self.templates("GENERATOR_LAB_NUGET", "nuget").into_iter().map(PathBuf::from).find(|p| p.exists())
    }

    /// The bundle folder whose `bundle.json` has these commits: a template root itself or one of its subfolders.
    pub fn bundle_dir(&self, old: &str, new: &str) -> Option<PathBuf> {
        let hit = |d: &Path| read_json(&d.join("bundle.json")).map(|b| s(&b, "oldCommit") == old && s(&b, "newCommit") == new).unwrap_or(false);
        if let Some(d) = std::env::var_os("PLUGIN_BUNDLE_DIR").map(PathBuf::from) {
            if hit(&d) {
                return Some(d);
            }
        }
        for root in self.templates("GENERATOR_LAB_BUNDLES", "bundles").iter().map(PathBuf::from) {
            if hit(&root) {
                return Some(root);
            }
            let mut subs: Vec<PathBuf> = std::fs::read_dir(&root).into_iter().flatten().flatten().map(|e| e.path()).collect();
            subs.sort();
            if let Some(d) = subs.into_iter().find(|d| hit(d)) {
                return Some(d);
            }
        }
        None
    }

    /// The issue reproduction for a requirement ID (`#2283`), `// lib` / `// app` text.
    pub fn reproduction(&self, requirement: &str) -> Option<String> {
        let r = &self.adapter["reproductions"];
        let bare = requirement.trim_start_matches('#');
        r[bare].as_str().or_else(|| r[format!("#{bare}").as_str()].as_str()).map(String::from)
    }
}

/// Split replay text at the `// lib` and `// app` markers; without markers all of it is app code,
/// since the generator runs on the app project only.
pub fn split_replay(text: &str) -> (String, String) {
    if !text.lines().any(|l| matches!(l.trim(), "// lib" | "// app")) {
        return (String::new(), text.to_string());
    }
    let (mut lib, mut app, mut cur) = (String::new(), String::new(), 0);
    for l in text.lines() {
        match l.trim() {
            "// lib" => cur = 0,
            "// app" => cur = 1,
            _ => {
                let t = if cur == 0 { &mut lib } else { &mut app };
                t.push_str(l);
                t.push('\n');
            }
        }
    }
    (lib, app)
}

// ---------------------------------------------------------------- host paths

fn hex(s: &str, n: usize) -> bool {
    s.len() == n && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Relative form of one absolute path: the part after a working copy folder (`wt/<sha>/`,
/// `rev/<sha12>/<block>/`, `mix/<a>-<b>/`, or a given root), or `consumer/<file>` for a
/// worker temp folder (`witness-run-*`). `None` leaves an unknown path as it is.
fn relative(path: &str, roots: &[String]) -> Option<String> {
    for r in roots {
        let r = r.trim_end_matches('/');
        if let Some(rest) = path.strip_prefix(r).and_then(|x| x.strip_prefix('/')) {
            return Some(rest.to_string());
        }
    }
    let segs: Vec<&str> = path.split('/').collect();
    for i in (0..segs.len()).rev() {
        let s = segs[i];
        let tail = |k: usize| (i + k < segs.len()).then(|| segs[i + k..].join("/"));
        if s.starts_with("witness-run-") {
            return tail(1).map(|t| format!("consumer/{t}"));
        }
        if s == "wt" && segs.get(i + 1).map(|x| hex(x, 40)).unwrap_or(false) {
            return tail(2);
        }
        if s == "rev" && segs.get(i + 1).map(|x| hex(x, 12)).unwrap_or(false) {
            return tail(3);
        }
        if s == "mix" && segs.get(i + 1).map(|x| x.split('-').all(|h| hex(h, 12))).unwrap_or(false) {
            return tail(2);
        }
    }
    None
}

/// Rewrite every absolute host path in a text to its short relative form (no host path reaches the page).
pub fn scrub(text: &str, roots: &[String]) -> String {
    let mut out = String::with_capacity(text.len());
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let starts = b[i] == b'/' && (i == 0 || matches!(b[i - 1], b' ' | b'\t' | b'\n' | b'\r' | b'"' | b'\'' | b'(' | b'=' | b':' | b'['))
            && i + 1 < b.len() && b[i + 1].is_ascii_alphanumeric();
        if starts {
            let mut j = i;
            while j < b.len() && !matches!(b[j], b' ' | b'\t' | b'\n' | b'\r' | b'"' | b'\'' | b')' | b'(' | b',' | b';' | b']') {
                j += 1;
            }
            // keep a trailing `:line N` / `(l,c)` out of the path
            let p = &text[i..j];
            let (p, keep) = match p.rfind(':') { Some(k) if k > 0 && p[k + 1..].chars().all(|c| c.is_ascii_digit()) => (&p[..k], &p[k..]), _ => (p, "") };
            match relative(p, roots) {
                Some(r) => { out.push_str(&r); out.push_str(keep); }
                None => out.push_str(&text[i..j]),
            }
            i = j;
        } else {
            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// `scrub` on every string in a JSON value.
pub fn scrub_value(v: &Value, roots: &[String]) -> Value {
    match v {
        Value::String(t) => Value::String(scrub(t, roots)),
        Value::Array(a) => Value::Array(a.iter().map(|x| scrub_value(x, roots)).collect()),
        Value::Object(m) => Value::Object(m.iter().map(|(k, x)| (k.clone(), scrub_value(x, roots))).collect()),
        x => x.clone(),
    }
}

/// Replay text without its leading `// Key=Value` build-property lines.
pub fn strip_properties(text: &str) -> String {
    let mut lines = text.lines().peekable();
    let mut out = Vec::new();
    while let Some(l) = lines.peek() {
        let c = l.trim().strip_prefix("//").map(str::trim);
        match c.and_then(|c| c.split_once('=')) {
            Some((k, _)) if !k.trim().is_empty() && k.trim().chars().all(|c| c.is_ascii_alphanumeric()) => { lines.next(); }
            _ => break,
        }
    }
    out.extend(lines);
    let mut t = out.join("\n");
    if text.ends_with('\n') && !t.is_empty() {
        t.push('\n');
    }
    t
}

/// Build properties from an object or from `Key=Value` lines.
pub fn build_properties(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), json!(v.as_str().map(String::from).unwrap_or_else(|| v.to_string())))).collect()),
        Value::String(t) => {
            let mut m = Map::new();
            for l in t.lines() {
                if let Some((k, v)) = l.split_once('=') {
                    if !k.trim().is_empty() {
                        m.insert(k.trim().into(), json!(v.trim()));
                    }
                }
            }
            Value::Object(m)
        }
        _ => json!({}),
    }
}

// ---------------------------------------------------------------- the worker

/// One long-lived `dotnet witness-worker.dll`; restarted after a timeout or exit (P3).
pub struct Worker {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    next: u64,
}

fn which(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(name)).find(|p| p.is_file())
}

impl Worker {
    pub fn start(cfg: &Config) -> Result<Worker, String> {
        let dll = cfg.worker_dll().ok_or("worker not built")?;
        let dotnet = std::env::var_os("WITNESS_DOTNET").map(PathBuf::from).or_else(|| which("dotnet")).ok_or("no dotnet")?;
        let mut c = Command::new(&dotnet);
        c.arg(&dll).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
            .env("DOTNET_NOLOGO", "1").env("DOTNET_CLI_TELEMETRY_OPTOUT", "1").env("WITNESS_DOTNET", &dotnet);
        if std::env::var_os("WITNESS_BWRAP").is_none() {
            if let Some(b) = which("bwrap") {
                c.env("WITNESS_BWRAP", b);
            }
        }
        if let Some(n) = cfg.nuget() {
            c.env("NUGET_PACKAGES", n);
        }
        if std::env::var_os("DOTNET_CLI_HOME").is_none() {
            c.env("DOTNET_CLI_HOME", std::env::temp_dir());
        }
        let mut child = c.spawn().map_err(|e| format!("spawn worker: {e}"))?;
        let stdin = child.stdin.take().unwrap();
        let out = BufReader::new(child.stdout.take().unwrap());
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for l in out.lines() {
                let Ok(l) = l else { break };
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
        Ok(Worker { child, stdin, lines: rx, next: 1 })
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// One worker request; the `result` or the error message.
    pub fn call(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = self.next;
        self.next += 1;
        let line = json!({"id": id, "method": method, "params": params}).to_string();
        writeln!(self.stdin, "{line}").and_then(|_| self.stdin.flush()).map_err(|e| format!("worker gone: {e}"))?;
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            let l = self.lines.recv_timeout(left).map_err(|_| "timeout".to_string())?;
            let Ok(v) = serde_json::from_str::<Value>(&l) else { continue };
            if v["id"].as_u64() != Some(id) {
                continue;
            }
            if let Some(e) = v.get("error") {
                return Err(s(e, "message").to_string());
            }
            return Ok(v["result"].clone());
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------- outcomes

fn crash_exception(diags: &[Value]) -> Option<String> {
    for d in diags {
        let m = s(d, "message");
        if let Some(i) = m.find("of type '") {
            let rest = &m[i + 9..];
            if let Some(j) = rest.find('\'') {
                let e = rest[..j].rsplit('.').next().unwrap_or("");
                if e.ends_with("Exception") {
                    return Some(e.to_string());
                }
            }
        }
    }
    None
}

fn ids(diags: &[Value], sev: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for d in diags.iter().filter(|d| s(d, "severity") == sev) {
        let id = s(d, "id").to_string();
        if !v.contains(&id) {
            v.push(id);
        }
    }
    v
}

/// Values that did not survive a round trip: `(path, sent, received)` per differing leaf.
pub fn lost_values(sent: &str, received: &str) -> Vec<(String, String, String)> {
    fn walk(path: &str, a: &Value, b: &Value, out: &mut Vec<(String, String, String)>) {
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                for (k, va) in x {
                    let p = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                    walk(&p, va, y.get(k).unwrap_or(&Value::Null), out);
                }
            }
            (Value::Array(x), Value::Array(y)) => {
                for (i, va) in x.iter().enumerate() {
                    walk(&format!("{path}[{i}]"), va, y.get(i).unwrap_or(&Value::Null), out);
                }
            }
            _ if a != b => {
                let t = |v: &Value| v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
                out.push((path.to_string(), t(a), t(b)));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    match (serde_json::from_str::<Value>(sent), serde_json::from_str::<Value>(received)) {
        (Ok(a), Ok(b)) => walk("", &a, &b, &mut out),
        _ if sent != received => out.push((String::new(), sent.into(), received.into())),
        _ => {}
    }
    out
}

/// `(exception, diagnostic IDs)` named in an outcome string.
pub fn parts(outcome: &str) -> (Option<String>, std::collections::BTreeSet<String>) {
    let words: Vec<&str> = outcome.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.')).filter(|w| !w.is_empty()).collect();
    let exc = words.iter().find(|w| w.ends_with("Exception")).map(|e| e.rsplit('.').next().unwrap().to_string());
    let ids = words.iter().filter(|w| {
        let letters = w.trim_end_matches(|c: char| c.is_ascii_digit());
        !letters.is_empty() && w.len() - letters.len() >= 3 && letters.chars().all(|c| c.is_ascii_alphabetic())
    }).map(|w| w.to_string()).collect();
    (exc, ids)
}

/// The model crate's rule (D23).
pub use model::{outcome_matches, snippet_matches};

/// The real outcome of one side as one line, in the vocabulary `model::classify` reads:
/// errors by ID; else a generator crash with its exception; else warnings and the round trip.
pub fn outcome(side: &Value) -> String {
    let diags: Vec<Value> = side["diagnostics"].as_array().cloned().unwrap_or_default();
    let errors = ids(&diags, "error");
    let warnings = ids(&diags, "warning");
    if !errors.is_empty() {
        let mut t = format!("errors {}", errors.join(", "));
        if !warnings.is_empty() {
            t += &format!("; warnings {}", warnings.join(", "));
        }
        if let Some(e) = crash_exception(&diags) {
            t += &format!(": {e}");
        }
        return t;
    }
    if warnings.iter().any(|w| w == "CS8785" || w == "AD0001") {
        return format!("{}: {}", warnings.join(", "), crash_exception(&diags).unwrap_or_else(|| "generator failed".into()));
    }
    let mut t = if warnings.is_empty() { "0 warnings".to_string() } else { format!("warnings {}", warnings.join(", ")) };
    let rt = &side["roundTrip"];
    if rt.is_object() {
        if let Some(e) = rt["error"].as_str() {
            t += &format!("; round trip fails: {}", e.split(':').next().unwrap_or(e));
        } else {
            let lost = lost_values(s(rt, "sent"), s(rt, "received"));
            if lost.is_empty() {
                t += "; round trip preserved";
            } else {
                let l: Vec<String> = lost.iter().map(|(p, a, b)| format!("{} {a} -> {b}", p.rsplit('.').next().unwrap_or(p))).collect();
                t += &format!("; round trip {}", l.join(", "));
            }
        }
    }
    t
}

/// `{sent, received}` of the first value a side's round trip lost, for `replays.json` (data loss, D21).
pub fn lossy_round_trip(side: &Value) -> Value {
    let rt = &side["roundTrip"];
    if !rt.is_object() || !rt["error"].is_null() {
        return Value::Null;
    }
    match lost_values(s(rt, "sent"), s(rt, "received")).into_iter().next() {
        Some((_, a, b)) => json!({"sent": a, "received": b}),
        None => Value::Null,
    }
}

// ---------------------------------------------------------------- the lab

pub struct Lab {
    pub cfg: Config,
    worker: Option<Worker>,
    pub old: String,
    pub new: String,
    pub bundle: Option<PathBuf>,
    /// Try the PR's declared tests as replay inputs (source `test`). On by default;
    /// off with `GENERATOR_LAB_TEST_CANDIDATES=0` or `replay --no-test-candidates`.
    pub test_candidates: bool,
    /// Model snippets were asked for (`replay --proposed`): a difference with none gets `no-recording`.
    pub snippets_expected: bool,
}

/// One run's inputs.
#[derive(Clone, Debug)]
pub struct RunInput {
    pub library: String,
    pub app: String,
    pub build_properties: Value,
    pub round_trip: bool,
    pub generated_resolver_only: bool,
}

impl RunInput {
    /// Replay text; leading `// Key=Value` comment lines are build properties (`// NoWarn=MsgPack005`).
    pub fn from_text(text: &str) -> RunInput {
        let (library, app) = split_replay(text);
        let mut props = Map::new();
        props.insert("NoWarn".into(), json!(""));
        for l in text.lines() {
            let Some(c) = l.trim().strip_prefix("//") else { break };
            if let Some((k, v)) = c.trim().split_once('=') {
                if !k.trim().is_empty() && k.trim().chars().all(|c| c.is_ascii_alphanumeric()) {
                    props.insert(k.trim().into(), json!(v.trim()));
                }
            }
        }
        RunInput { library, app, build_properties: Value::Object(props), round_trip: text.contains("Make()"), generated_resolver_only: true }
    }

    /// A replay entry's input: text (old `// Key=Value` header lines still read) plus `buildProperties`.
    pub fn from_entry(text: &str, props: &Value) -> RunInput {
        let mut i = RunInput::from_text(text);
        if let (Some(m), Some(p)) = (i.build_properties.as_object_mut(), props.as_object()) {
            for (k, v) in p {
                m.insert(k.clone(), json!(v.as_str().unwrap_or("")));
            }
        }
        i
    }

    /// Values by declared input name, as a preset carries them; build properties as `Key=Value` lines.
    pub fn preset_inputs(&self) -> Value {
        let props: Vec<String> = self.build_properties.as_object().into_iter().flatten()
            .map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or(""))).collect();
        json!({"library": self.library, "app": self.app, "buildProperties": props.join("\n"),
            "roundTrip": self.round_trip, "generatedResolverOnly": self.generated_resolver_only})
    }
}

impl Lab {
    pub fn new(cfg: Config) -> Lab {
        Lab { cfg, worker: None, old: String::new(), new: String::new(), bundle: None,
            test_candidates: std::env::var("GENERATOR_LAB_TEST_CANDIDATES").map(|v| v != "0").unwrap_or(true), snippets_expected: false }
    }

    pub fn for_bundle(cfg: Config, bundle_dir: &Path) -> Result<Lab, String> {
        let b = read_json(&bundle_dir.join("bundle.json")).ok_or("bundle.json unreadable")?;
        let mut lab = Lab::new(cfg);
        lab.old = s(&b, "oldCommit").into();
        lab.new = s(&b, "newCommit").into();
        lab.bundle = Some(bundle_dir.to_path_buf());
        Ok(lab)
    }

    pub fn worker_pid(&mut self) -> Result<u32, String> {
        self.ensure_worker()?;
        Ok(self.worker.as_ref().unwrap().pid())
    }

    fn ensure_worker(&mut self) -> Result<(), String> {
        if self.worker.as_mut().map(|w| w.child.try_wait().ok().flatten().is_some()).unwrap_or(true) {
            self.worker = Some(Worker::start(&self.cfg)?);
        }
        Ok(())
    }

    /// The worker's `run` on old and new (and a reverted build): its `result`.
    pub fn run(&mut self, input: &RunInput, reverted: Option<Value>) -> Result<Value, String> {
        if input.library.len() + input.app.len() > MAX_INPUT {
            return Err("input over 64 kB".into());
        }
        let old = self.cfg.tree(&self.old).ok_or_else(|| format!("no build of {}", self.old))?;
        let new = self.cfg.tree(&self.new).ok_or_else(|| format!("no build of {}", self.new))?;
        let params = json!({"old": old, "new": new, "reverted": reverted, "library": input.library, "app": input.app,
            "buildProperties": input.build_properties, "roundTrip": input.round_trip, "generatedResolverOnly": input.generated_resolver_only});
        self.ensure_worker()?;
        let r = self.worker.as_mut().unwrap().call("run", params, WORKER_TIMEOUT);
        if r.as_ref().err().map(|e| e == "timeout" || e.starts_with("worker gone")).unwrap_or(false) {
            self.worker = None; // killed on drop; the next request starts a fresh worker
            return Err(format!("{}; worker restarted", r.unwrap_err()));
        }
        r
    }

    fn records(&self, name: &str, key: &str) -> Vec<Value> {
        self.bundle.as_ref().and_then(|b| read_json(&b.join(format!("{name}.json")))).and_then(|v| v[key].as_array().cloned()).unwrap_or_default()
    }

    fn requirement(&self) -> String {
        self.bundle.as_ref().and_then(|b| read_json(&b.join("bundle.json"))).map(|b| s(&b["requirement"], "id").to_string()).unwrap_or_default()
    }

    /// Presets (D34): one per difference with a replay input, the issue reproduction, one revert per
    /// production block whose reverted build exists, and free input.
    pub fn presets(&self) -> Vec<Value> {
        let diffs = self.records("differences", "differences");
        let replays = self.records("replays", "replays");
        let mut out = Vec::new();
        let mut ds = diffs.clone();
        ds.sort_by_key(|d| d["number"].as_u64().unwrap_or(0));
        for d in &ds {
            let id = s(d, "id");
            if let Some(r) = replays.iter().find(|r| s(r, "difference") == id && r["input"]["text"].is_string()) {
                let i = RunInput::from_entry(s(&r["input"], "text"), &r["input"]["buildProperties"]);
                out.push(json!({"kind": "difference", "label": d["number"].to_string(), "difference": id, "inputs": i.preset_inputs()}));
            }
        }
        let req = self.requirement();
        let issue = self.cfg.reproduction(&req)
            .or_else(|| replays.iter().find(|r| s(&r["input"], "source") == "issue").map(|r| s(&r["input"], "text").to_string()));
        if let Some(t) = issue {
            let i = RunInput::from_text(&t);
            out.push(json!({"kind": "issue", "label": if req.is_empty() { "issue".to_string() } else { req.clone() }, "inputs": i.preset_inputs()}));
        }
        for b in self.records("blocks", "blocks").iter().filter(|b| s(b, "category") == "production") {
            let id = s(b, "id");
            if self.cfg.reverted(&self.new, id).is_some() {
                out.push(json!({"kind": "revert", "label": format!("revert {id}"), "block": id, "inputs": {}}));
            }
        }
        out.push(json!({"kind": "free", "label": "free", "inputs": {}}));
        out
    }

    fn subject(&self) -> Value {
        let a = &self.cfg.adapter["subject"];
        let mut locs: Vec<Value> = Vec::new();
        for d in self.records("differences", "differences") {
            if d["location"].is_object() && !locs.contains(&d["location"]) {
                locs.push(d["location"].clone());
            }
        }
        let inputs: Vec<Value> = self.cfg.manifest["inputs"].as_array().into_iter().flatten().map(|i| {
            let field = match s(i, "type") { "code" => "code", "bool" => "bool", "int" => "int", "string" => "string", "choice" => "choice", _ => "text" };
            json!({"name": i["name"], "role": i["role"], "type": i["type"], "constructible": true, "field": field})
        }).collect();
        json!({"id": s(a, "id"), "name": s(a, "name"), "definition": {"text": s(a, "definition"), "locations": locs},
            "inputs": inputs, "outputs": self.cfg.manifest["outputs"], "presets": self.presets()})
    }

    fn default_input(&self, name: &str) -> Value {
        self.cfg.manifest["inputs"].as_array().into_iter().flatten().find(|i| s(i, "name") == name).map(|i| i["default"].clone()).unwrap_or(Value::Null)
    }

    /// Prepared tree folders of this bundle (old, new), stripped from texts before they leave the plugin.
    pub fn roots(&self) -> Vec<String> {
        [&self.old, &self.new].iter().filter_map(|c| self.cfg.tree_dir(c)).map(|p| p.display().to_string()).collect()
    }

    /// One plugin request (plugin-request.schema.json) to one reply (plugin-reply.schema.json).
    pub fn handle(&mut self, req: &Value) -> Value {
        let id = req["id"].clone();
        let op = s(req, "op").to_string();
        let fail = |e: String| json!({"id": id, "op": if op.is_empty() { "run" } else { &op }, "ok": false, "error": e});
        match op.as_str() {
            "describe" => {
                self.old = s(&req["bundle"], "oldCommit").into();
                self.new = s(&req["bundle"], "newCommit").into();
                self.bundle = self.cfg.bundle_dir(&self.old, &self.new);
                let pid = match self.worker_pid() {
                    Ok(p) => p,
                    Err(e) => return fail(e),
                };
                json!({"id": id, "op": "describe", "ok": true, "pid": pid, "subjects": [self.subject()]})
            }
            "run" => match self.run_request(req) {
                Ok((results, pid)) => json!({"id": id, "op": "run", "ok": true, "pid": pid, "results": results}),
                Err(e) => fail(e),
            },
            "shutdown" => {
                self.worker = None;
                json!({"id": id, "op": "shutdown", "ok": true})
            }
            _ => fail(format!("unknown op {op:?}")),
        }
    }

    fn run_request(&mut self, req: &Value) -> Result<(Vec<Value>, u64), String> {
        let inp = &req["inputs"];
        let get = |k: &str| if inp.get(k).map(|v| !v.is_null()).unwrap_or(false) { inp[k].clone() } else { self.default_input(k) };
        let input = RunInput {
            library: get("library").as_str().unwrap_or("").into(),
            app: get("app").as_str().unwrap_or("").into(),
            build_properties: build_properties(&get("buildProperties")),
            round_trip: get("roundTrip").as_bool().unwrap_or(false),
            generated_resolver_only: get("generatedResolverOnly").as_bool().unwrap_or(true),
        };
        let mut sides: Vec<String> = req["sides"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
        if sides.is_empty() {
            sides = vec!["old".into(), "new".into()];
        }
        let block = if s(&req["preset"], "kind") == "revert" { s(&req["preset"], "block").to_string() } else { String::new() };
        if !block.is_empty() && !sides.iter().any(|x| x == "reverted") {
            sides.push("reverted".into());
        }
        let reverted = if sides.iter().any(|x| x == "reverted") {
            if block.is_empty() {
                return Err("reverted side needs a revert preset".into());
            }
            Some(self.cfg.reverted(&self.new, &block).ok_or_else(|| format!("no reverted build of {block}"))?)
        } else {
            None
        };
        let r = self.run(&input, reverted)?;
        let text = |side: &Value| -> String {
            side["generated"].as_array().into_iter().flatten().map(|g| format!("// {}\n{}\n", s(g, "hintName"), s(g, "text"))).collect()
        };
        let old_text = text(&r["old"]);
        let mut results = Vec::new();
        for side in &sides {
            let v = &r[side.as_str()];
            if !v.is_object() {
                continue;
            }
            results.push(json!({"output": "diagnostics", "side": side, "type": "diagnostics", "value": v["diagnostics"]}));
            results.push(json!({"output": "generated", "side": side, "type": "text-diff", "value": {"old": old_text, "new": text(v)}}));
            if input.round_trip {
                let rt = &v["roundTrip"];
                let facts = if rt.is_object() {
                    let mut f = json!({"resolver": rt["resolver"], "sent": rt["sent"], "received": rt["received"], "wire": rt["wire"], "error": rt["error"], "pid": v["pid"]});
                    let lost = lost_values(s(rt, "sent"), s(rt, "received"));
                    if rt["error"].is_null() && !lost.is_empty() {
                        f["lost"] = json!(lost.iter().map(|(p, a, b)| format!("{p} {a} -> {b}")).collect::<Vec<_>>().join(", "));
                    }
                    f
                } else {
                    json!({"state": "not run", "reason": "build errors"})
                };
                results.push(json!({"output": "roundTrip", "side": side, "type": "facts", "value": facts}));
            }
        }
        Ok((results, r["workerPid"].as_u64().unwrap_or(0)))
    }

    // ------------------------------------------------------------ replays (D23)

    /// Replay every difference of the bundle: candidates are the bundle's existing replay input, the
    /// issue reproduction and model-proposed snippets (`{difference, text}`). A candidate is kept only
    /// when the real old and new outcomes match the difference's row (`snippet_matches`, the rule of the model crate).
    /// Returns `replays.json`, with `issue` holding the reproduction's own old and new outcome.
    pub fn replay(&mut self, proposed: &[Value]) -> Result<Value, String> {
        let diffs = self.records("differences", "differences");
        let existing = self.records("replays", "replays");
        let req = self.requirement();
        let repro = self.cfg.reproduction(&req)
            .or_else(|| existing.iter().find(|r| s(&r["input"], "source") == "issue").map(|r| s(&r["input"], "text").to_string()));
        let mut cache: Vec<(String, Result<Value, String>)> = Vec::new();
        let mut run_text = |lab: &mut Lab, text: &str, props: &Value| -> Result<Value, String> {
            let key = format!("{props}\n{text}");
            if let Some((_, r)) = cache.iter().find(|(t, _)| *t == key) {
                return r.clone();
            }
            let r = lab.run(&RunInput::from_entry(text, props), None);
            cache.push((key, r.clone()));
            r
        };
        let none = json!({});
        let issue = match &repro {
            Some(t) => match run_text(self, t, &none) {
                Ok(r) => {
                    let (o, n) = (outcome(&r["old"]), outcome(&r["new"]));
                    json!({"text": t, "old": o, "new": n, "differs": parts(&o) != parts(&n)})
                }
                Err(e) => json!({"text": t, "error": e}),
            },
            None => Value::Null,
        };
        let mut names = Vec::new();
        for c in [self.old.clone(), self.new.clone()] {
            if let Some(t) = self.cfg.tree_dir(&c) {
                names.extend(descriptor_ids(&t.join("src")));
            }
        }
        let mut replays = Vec::new();
        for d in &diffs {
            let id = s(d, "id");
            let not_replayed = |reason: &str, detail: String| json!({"difference": id, "input": null, "old": null, "new": null,
                "state": "not-replayed", "roundTrip": null, "reason": reason, "detail": detail});
            if matches!(s(d, "kind"), "public-api" | "output-changed") {
                replays.push(not_replayed("not-replayable", format!("kind {}", s(d, "kind"))));
                continue;
            }
            let mut cands: Vec<(String, String, Value)> = Vec::new();
            for r in existing.iter().filter(|r| s(r, "difference") == id && r["input"]["text"].is_string()) {
                cands.push((s(&r["input"], "source").into(), s(&r["input"], "text").into(), r["input"]["buildProperties"].clone()));
            }
            if let Some(t) = &repro {
                cands.push(("issue".into(), t.clone(), none.clone()));
            }
            let tests = if self.test_candidates { self.test_candidates(s(d, "block")) } else { vec![] };
            for t in tests {
                cands.push(("test".into(), t, none.clone()));
            }
            let snippets: Vec<&Value> = proposed.iter().filter(|p| s(p, "difference") == id).collect();
            for p in &snippets {
                cands.push(("model".into(), s(p, "text").into(), p["buildProperties"].clone()));
            }
            let d = &normalize_row(d, &names);
            let mut entry = None;
            let mut closest: Option<(u8, String, String, String)> = None;
            for (source, text, props) in cands {
                let Ok(r) = run_text(self, &text, &props) else { continue };
                let (o, n) = (outcome(&r["old"]), outcome(&r["new"]));
                if snippet_matches(d, &o, &n) {
                    let i = RunInput::from_entry(&text, &props);
                    let mut input = json!({"source": source, "text": strip_properties(&text)});
                    let bp: Map<String, Value> = i.build_properties.as_object().into_iter().flatten()
                        .filter(|(_, v)| !v.as_str().unwrap_or("").is_empty()).map(|(k, v)| (k.clone(), v.clone())).collect();
                    if !bp.is_empty() {
                        input["buildProperties"] = Value::Object(bp);
                    }
                    entry = Some(json!({"difference": id, "input": input, "old": o, "new": n,
                        "state": "seen-live", "roundTrip": lossy_round_trip(&r["new"])}));
                    break;
                }
                let score = model::outcome_matches(s(d, "old"), &o) as u8 + model::outcome_matches(s(d, "new"), &n) as u8;
                if closest.as_ref().map(|c| score > c.0).unwrap_or(true) {
                    closest = Some((score, source, o, n));
                }
            }
            replays.push(entry.unwrap_or_else(|| match closest {
                None if snippets.is_empty() && self.snippets_expected => not_replayed("no-recording", "no model snippet recorded".into()),
                None => not_replayed("no-candidate", "no input".into()),
                Some((_, src, o, n)) if snippets.is_empty() && self.snippets_expected =>
                    not_replayed("no-recording", format!("no model snippet recorded; closest {src}: old {o} | new {n}")),
                Some((_, src, o, n)) => not_replayed("no-match", format!("closest {src}: old {o} | new {n}")),
            }));
        }
        Ok(scrub_value(&json!({"provenance": "pipeline", "replays": replays, "issue": issue}), &self.roots()))
    }

    /// Replay texts from the declared (and new-API) tests linked to a block, read from the new working copy.
    pub fn test_candidates(&self, block: &str) -> Vec<String> {
        let Some(tree) = self.cfg.tree_dir(&self.new) else { return vec![] };
        let linked: Vec<String> = self.records("links", "links").iter().filter(|l| s(l, "block") == block)
            .flat_map(|l| l["tests"].as_array().cloned().unwrap_or_default()).filter_map(|t| t.as_str().map(String::from)).collect();
        let mut out = Vec::new();
        for t in self.records("tests", "tests") {
            let name = s(&t, "name");
            if !linked.iter().any(|l| l == name) || !matches!(s(&t, "category"), "declared" | "new-api") {
                continue;
            }
            let file = t["location"]["file"].as_str().or(t["file"].as_str()).unwrap_or("");
            let Ok(src) = std::fs::read_to_string(tree.join(file)) else { continue };
            if let Some(x) = test_input(&src, name.rsplit('.').next().unwrap_or(name)) {
                if !out.contains(&x) {
                    out.push(x);
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------- inputs from tests

/// The C# raw string literals (`"""…"""`) of one test method that hold MessagePack code, as
/// replay text: with two or more, the first is the library and the last the app; a
/// `[GeneratedMessagePackResolver]` class is removed (the round trip uses the default resolver name)
/// and a `Probe.Make()` is appended that sets every member to a non-default value.
pub fn test_input(src: &str, method: &str) -> Option<String> {
    let start = src.find(&format!(" {method}("))?;
    let rest = &src[start..];
    // the method ends where the next member's doc comment or test attribute starts
    let end = ["\n    ///", "\n    [Fact", "\n    [Theory"].iter().filter_map(|m| rest[1..].find(m)).min().map(|e| e + 1).unwrap_or(rest.len());
    let body = &rest[..end];
    let lits: Vec<String> = body.split("\"\"\"").enumerate().filter(|(i, _)| i % 2 == 1).map(|(_, t)| t.trim_matches('\n').to_string())
        .filter(|t| t.contains("MessagePackObject")).collect();
    let app = strip_resolver(lits.last()?);
    let lib = if lits.len() >= 2 { lits[0].clone() } else { String::new() };
    let probe = probe_for(&lib, &app)?;
    Some(format!("// lib\n{lib}\n// app\n{app}\n{probe}"))
}

fn strip_resolver(app: &str) -> String {
    let mut out = Vec::new();
    let mut skip = false;
    for l in app.lines() {
        if l.trim() == "[GeneratedMessagePackResolver]" {
            skip = true;
            continue;
        }
        if skip && l.contains("partial class") {
            skip = false;
            continue;
        }
        skip = false;
        out.push(l);
    }
    out.join("\n")
}

struct TypeDecl {
    ns: String,
    name: String,
    params: Vec<String>,
    members: Vec<(String, String)>,
    object: bool,
}

fn types_of(code: &str) -> (Vec<TypeDecl>, bool) {
    let (mut ns, mut file_scoped, mut out, mut attr) = (String::new(), false, Vec::<TypeDecl>::new(), false);
    for l in code.lines() {
        let t = l.trim();
        if let Some(n) = t.strip_prefix("namespace ") {
            file_scoped = n.trim_end().ends_with(';');
            ns = n.trim_end_matches(';').trim().to_string();
            continue;
        }
        if t.starts_with("[MessagePackObject") {
            attr = true;
            continue;
        }
        let words: Vec<&str> = t.split_whitespace().collect();
        if let Some(i) = words.iter().position(|w| matches!(*w, "class" | "struct")) {
            if let Some(n) = words.get(i + 1) {
                let (name, params) = match n.split_once('<') {
                    Some((a, b)) => (a.to_string(), b.trim_end_matches('>').split(',').map(|x| x.trim().to_string()).collect()),
                    None => (n.trim_end_matches(['{', '(']).to_string(), vec![]),
                };
                out.push(TypeDecl { ns: ns.clone(), name, params, members: vec![], object: attr });
                attr = false;
                continue;
            }
        }
        // public <Type> <Name> { get; set; }  or  public <Type> <Name>;
        let m = t.split("]").last().unwrap_or(t).trim();
        if let Some(r) = m.strip_prefix("public ") {
            let ws: Vec<&str> = r.split_whitespace().collect();
            if ws.len() >= 2 && (r.contains("set;") || r.contains("init;") || ws[1].ends_with(';')) && !r.contains('(') {
                if let Some(td) = out.last_mut() {
                    td.members.push((ws[0].to_string(), ws[1].trim_end_matches(';').to_string()));
                }
            }
        }
    }
    (out, file_scoped)
}

fn value_for(ty: &str, types: &[TypeDecl], depth: usize) -> Option<String> {
    match ty.trim_end_matches('?') {
        "int" | "long" | "short" | "uint" | "ulong" | "double" | "float" | "decimal" => return Some("42".into()),
        "byte" | "sbyte" | "ushort" => return Some("(byte)42".into()),
        "string" => return Some("\"k\"".into()),
        "bool" => return Some("true".into()),
        _ => {}
    }
    if depth > 4 {
        return None;
    }
    let (name, args): (&str, Vec<String>) = match ty.split_once('<') {
        Some((a, b)) => (a, b.trim_end_matches('>').split(',').map(|x| x.trim().to_string()).collect()),
        None => (ty, vec![]),
    };
    let td = types.iter().find(|t| t.name == name && t.params.len() == args.len())?;
    let subst = |m: &str| td.params.iter().zip(&args).find(|(p, _)| p.as_str() == m).map(|(_, a)| a.clone()).unwrap_or_else(|| m.to_string());
    let inits: Vec<String> = td.members.iter().filter_map(|(mt, mn)| Some(format!("{mn} = {}", value_for(&subst(mt), types, depth + 1)?))).collect();
    let full = if td.ns.is_empty() { format!("global::{ty}") } else { format!("global::{}.{ty}", td.ns) };
    Some(format!("new {full} {{ {} }}", inits.join(", ")))
}

fn probe_for(lib: &str, app: &str) -> Option<String> {
    let (mut types, _) = types_of(lib);
    let (app_types, file_scoped) = types_of(app);
    let root = app_types.iter().find(|t| t.object && t.params.is_empty())?.name.clone();
    let root_ns = app_types.iter().find(|t| t.name == root).map(|t| t.ns.clone()).unwrap_or_default();
    types.extend(app_types);
    let v = value_for(&root, &types, 0)?;
    let class = format!("public static class Probe {{ public static object Make() => {v}; }}");
    Some(if file_scoped || root_ns.is_empty() { class } else { format!("namespace WitnessProbe {{ {class} }}") })
}

// ---------------------------------------------------------------- descriptor names in rows

/// `(descriptor field name, diagnostic ID)` from `DiagnosticDescriptor` declarations under `dir`,
/// resolving `id: SomeConst` through `const string SomeConst = "…"`.
pub fn descriptor_ids(dir: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() && !matches!(p.file_name().and_then(|n| n.to_str()), Some("bin" | "obj")) {
                stack.push(p);
            } else if p.extension().map(|x| x == "cs").unwrap_or(false) {
                files.push(p);
            }
        }
    }
    let (mut consts, mut descs) = (Vec::<(String, String)>::new(), Vec::<(String, String)>::new());
    for f in files {
        let Ok(t) = std::fs::read_to_string(&f) else { continue };
        if !t.contains("DiagnosticDescriptor") {
            continue;
        }
        let lines: Vec<&str> = t.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            if let Some(p) = l.find("const string ") {
                let r = &l[p + 13..];
                if let (Some((n, v)), true) = (r.split_once('='), r.contains('"')) {
                    consts.push((n.trim().into(), v.trim().trim_end_matches(';').trim_matches('"').into()));
                }
            }
            if let Some(p) = l.find("DiagnosticDescriptor ") {
                let r = &l[p + 21..];
                if let Some(n) = r.split(|c: char| c == '=' || c.is_whitespace()).next().filter(|n| !n.is_empty() && r.contains('=')) {
                    if let Some(idl) = lines[i..lines.len().min(i + 4)].iter().find_map(|x| x.trim().strip_prefix("id:")) {
                        descs.push((n.into(), idl.trim().trim_end_matches(',').into()));
                    }
                }
            }
        }
    }
    descs.into_iter().filter_map(|(n, v)| {
        let id = if v.starts_with('"') { v.trim_matches('"').to_string() } else { consts.iter().find(|(c, _)| *c == v)?.1.clone() };
        Some((n, id))
    }).collect()
}

/// A difference whose row outcomes name descriptors (`MsgPack00xMessagePackAnalyzer.MemberNeedsKey`)
/// with those names replaced by their diagnostic IDs.
pub fn normalize_row(d: &Value, names: &[(String, String)]) -> Value {
    let mut d = d.clone();
    for k in ["old", "new"] {
        let Some(t) = d[k].as_str() else { continue };
        let words: Vec<String> = t.split_whitespace().map(|w| {
            let core = w.trim_end_matches([',', ';']);
            let last = core.rsplit('.').next().unwrap_or(core);
            match names.iter().find(|(n, _)| n == last) {
                Some((_, id)) if core.contains('.') || core == last => w.replacen(core, id, 1),
                _ => w.to_string(),
            }
        }).collect();
        d[k] = json!(words.join(" "));
    }
    d
}

/// The plugin's line loop: one request per stdin line, one reply per stdout line.
pub fn serve(mut lab: Lab) {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(req) => lab.handle(&req),
            Err(e) => json!({"id": 1, "op": "run", "ok": false, "error": format!("request is not JSON: {e}")}),
        };
        let reply = scrub_value(&reply, &lab.roots());
        if writeln!(out, "{reply}").and_then(|_| out.flush()).is_err() {
            break;
        }
        if s(&reply, "op") == "shutdown" {
            break;
        }
    }
}

// ---------------------------------------------------------------- consumer projects

/// A buildable consumer (library and app project) for `dotnet build` with the SDK: references the
/// generator and runtime of one built tree, adds `Microsoft.Net.Compilers.Toolset` (SDK 8 ships
/// Roslyn 4.11, the generator needs 4.12) and the build properties (e.g. `NoWarn`). Files written to `dir`.
pub fn write_consumer(cfg: &Config, dir: &Path, side: &Value, input: &RunInput) -> Result<(), String> {
    let pkgs: String = cfg.adapter["consumerPackages"].as_object().into_iter().flatten()
        .map(|(k, v)| format!("    <PackageReference Include=\"{k}\" Version=\"{}\" PrivateAssets=\"all\" />\n", v.as_str().unwrap_or(""))).collect();
    let props: String = input.build_properties.as_object().into_iter().flatten()
        .filter(|(_, v)| !v.as_str().unwrap_or("").is_empty())
        .map(|(k, v)| format!("    <{k}>{}</{k}>\n", v.as_str().unwrap_or(""))).collect();
    let rt = s(side, "runtime");
    let proj = |name: &str, extra: &str| format!(
        "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <TargetFramework>net8.0</TargetFramework>\n    <AssemblyName>{name}</AssemblyName>\n    <LangVersion>12</LangVersion>\n{props}  </PropertyGroup>\n  <ItemGroup>\n{pkgs}    <Reference Include=\"MessagePack\"><HintPath>{rt}/MessagePack.dll</HintPath></Reference>\n    <Reference Include=\"MessagePack.Annotations\"><HintPath>{rt}/MessagePack.Annotations.dll</HintPath></Reference>\n{extra}  </ItemGroup>\n</Project>\n");
    let w = |p: PathBuf, t: String| -> Result<(), String> {
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&p, t).map_err(|e| e.to_string())
    };
    w(dir.join("lib/lib.csproj"), proj("Lib", ""))?;
    w(dir.join("lib/Lib.cs"), input.library.clone())?;
    w(dir.join("app/app.csproj"), proj("App", &format!(
        "    <ProjectReference Include=\"../lib/lib.csproj\" />\n    <Analyzer Include=\"{}\" />\n", s(side, "generator"))))?;
    w(dir.join("app/App.cs"), input.app.clone())
}
