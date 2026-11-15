//! Witness contracts: a small JSON Schema validator (the subset the contract schemas use),
//! the bundle validator with the coverage invariants of `specs/evidence-bundle`, and the
//! recording key. Every check stops at the first failure and names its path.
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// A validation failure: `path` is `<file>` plus a JSON pointer, or `<file>:<line>` for line invariants.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub path: String,
    pub message: String,
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}
impl std::error::Error for Failure {}
fn fail<T>(path: impl Into<String>, message: impl Into<String>) -> Result<T, Failure> {
    Err(Failure { path: path.into(), message: message.into() })
}

/// The contract directory (this crate), overridable with `WITNESS_CONTRACTS`.
pub fn contracts_dir() -> PathBuf {
    std::env::var_os("WITNESS_CONTRACTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

/// Records of a bundle directory, in the order the spec lists them.
pub const BUNDLE_RECORDS: [&str; 17] = [
    "bundle", "files", "parts", "blocks", "rows", "differences", "scope", "lean", "tests", "links", "dependencies",
    "groups", "levels", "questions", "mode", "replays", "ai-review",
];

/// A set of schemas loaded from one or more directories, addressed by file name.
pub struct Schemas {
    docs: HashMap<String, Value>,
}

impl Schemas {
    pub fn load(dirs: &[PathBuf]) -> Result<Schemas, Failure> {
        let mut docs = HashMap::new();
        for d in dirs {
            let rd = fs::read_dir(d).map_err(|e| Failure { path: d.display().to_string(), message: e.to_string() })?;
            for e in rd.flatten() {
                let p = e.path();
                let name = p.file_name().unwrap().to_string_lossy().to_string();
                if name.ends_with(".schema.json") {
                    docs.insert(name, read_json(&p)?);
                }
            }
        }
        Ok(Schemas { docs })
    }
    /// Record schemas (`contracts/schemas`) and message schemas (`contracts/messages`).
    pub fn contract() -> Result<Schemas, Failure> {
        let c = contracts_dir();
        Schemas::load(&[c.join("schemas"), c.join("messages")])
    }
    pub fn has(&self, name: &str) -> bool {
        self.docs.contains_key(&format!("{name}.schema.json"))
    }
    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.docs.keys().map(|k| k.trim_end_matches(".schema.json").to_string()).collect();
        v.sort();
        v
    }
    pub fn schema(&self, name: &str) -> Option<&Value> {
        self.docs.get(&format!("{name}.schema.json"))
    }
    /// Validate `v` against schema `<name>.schema.json`; `prefix` starts the failure path.
    pub fn validate(&self, name: &str, v: &Value, prefix: &str) -> Result<(), Failure> {
        let doc = format!("{name}.schema.json");
        let s = self.docs.get(&doc).ok_or_else(|| Failure { path: prefix.into(), message: format!("no schema {doc}") })?;
        self.check(&doc, s, v, prefix)
    }

    /// Validate `v` against `<name>.schema.json#/$defs/<def>`.
    pub fn validate_def(&self, name: &str, def: &str, v: &Value, prefix: &str) -> Result<(), Failure> {
        let doc = format!("{name}.schema.json");
        let (d, s) = self.resolve(&doc, &format!("#/$defs/{def}"))?;
        self.check(&d, s, v, prefix)
    }

    fn resolve<'a>(&'a self, doc: &str, r: &str) -> Result<(String, &'a Value), Failure> {
        let (file, frag) = match r.split_once('#') {
            Some(("", f)) => (doc.to_string(), f),
            Some((d, f)) => (d.to_string(), f),
            None => (r.to_string(), ""),
        };
        let mut cur = self.docs.get(&file).ok_or_else(|| Failure { path: doc.into(), message: format!("unresolved $ref {r}") })?;
        for seg in frag.split('/').filter(|s| !s.is_empty()) {
            cur = cur.get(seg).ok_or_else(|| Failure { path: doc.into(), message: format!("unresolved $ref {r}") })?;
        }
        Ok((file, cur))
    }

    fn check(&self, doc: &str, s: &Value, v: &Value, path: &str) -> Result<(), Failure> {
        let Some(s) = s.as_object() else { return Ok(()) };
        if let Some(r) = s.get("$ref").and_then(Value::as_str) {
            let (d, t) = self.resolve(doc, r)?;
            self.check(&d, t, v, path)?;
        }
        if let Some(t) = s.get("type") {
            let ok = match t {
                Value::String(t) => type_ok(t, v),
                Value::Array(ts) => ts.iter().any(|t| t.as_str().map(|t| type_ok(t, v)).unwrap_or(false)),
                _ => true,
            };
            if !ok {
                return fail(path, format!("expected type {}, found {}", t, type_name(v)));
            }
        }
        if let Some(e) = s.get("enum").and_then(Value::as_array) {
            if !e.contains(v) {
                return fail(path, format!("{} is not one of {}", v, Value::Array(e.clone())));
            }
        }
        if let Some(c) = s.get("const") {
            if c != v {
                return fail(path, format!("expected {c}"));
            }
        }
        if let Some(alts) = s.get("anyOf").or_else(|| s.get("oneOf")).and_then(Value::as_array) {
            let mut best: Option<Failure> = None;
            let mut ok = false;
            for a in alts {
                match self.check(doc, a, v, path) {
                    Ok(()) => { ok = true; break; }
                    // report the alternative that got furthest into the value
                    Err(f) => {
                        if best.as_ref().map(|b| f.path.len() > b.path.len()).unwrap_or(true) {
                            best = Some(f);
                        }
                    }
                }
            }
            if !ok {
                return Err(best.unwrap_or(Failure { path: path.into(), message: "matches no alternative".into() }));
            }
        }
        match v {
            Value::Object(o) => {
                if let Some(req) = s.get("required").and_then(Value::as_array) {
                    for r in req.iter().filter_map(Value::as_str) {
                        if !o.contains_key(r) {
                            return fail(path, format!("missing required field `{r}`"));
                        }
                    }
                }
                if let Some(m) = s.get("minProperties").and_then(Value::as_u64) {
                    if (o.len() as u64) < m {
                        return fail(path, format!("expected at least {m} fields"));
                    }
                }
                let props = s.get("properties").and_then(Value::as_object);
                for (k, val) in o {
                    let p = format!("{path}/{k}");
                    if let Some(ps) = props.and_then(|ps| ps.get(k)) {
                        self.check(doc, ps, val, &p)?;
                    } else if let Some(ap) = s.get("additionalProperties") {
                        if ap == &Value::Bool(false) {
                            return fail(p, format!("field `{k}` is not allowed"));
                        }
                        self.check(doc, ap, val, &p)?;
                    }
                    if let Some(pn) = s.get("propertyNames") {
                        self.check(doc, pn, &Value::String(k.clone()), &p)?;
                    }
                }
            }
            Value::Array(a) => {
                if let Some(m) = s.get("minItems").and_then(Value::as_u64) {
                    if (a.len() as u64) < m {
                        return fail(path, format!("expected at least {m} items"));
                    }
                }
                if let Some(it) = s.get("items") {
                    for (i, x) in a.iter().enumerate() {
                        self.check(doc, it, x, &format!("{path}/{i}"))?;
                    }
                }
            }
            Value::Number(n) => {
                if let (Some(m), Some(x)) = (s.get("minimum").and_then(Value::as_f64), n.as_f64()) {
                    if x < m {
                        return fail(path, format!("{x} is below the minimum {m}"));
                    }
                }
                if let (Some(m), Some(x)) = (s.get("maximum").and_then(Value::as_f64), n.as_f64()) {
                    if x > m {
                        return fail(path, format!("{x} is above the maximum {m}"));
                    }
                }
            }
            Value::String(st) => {
                if let Some(m) = s.get("minLength").and_then(Value::as_u64) {
                    if (st.chars().count() as u64) < m {
                        return fail(path, format!("shorter than {m}"));
                    }
                }
                if let Some(p) = s.get("pattern").and_then(Value::as_str) {
                    let re = regex::Regex::new(p).map_err(|e| Failure { path: doc.into(), message: e.to_string() })?;
                    if !re.is_match(st) {
                        return fail(path, format!("\"{st}\" does not match {p}"));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn type_ok(t: &str, v: &Value) -> bool {
    match t {
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "integer" => v.is_i64() || v.is_u64(),
        "number" => v.is_number(),
        "boolean" => v.is_boolean(),
        "null" => v.is_null(),
        _ => true,
    }
}
fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub fn read_json(p: &Path) -> Result<Value, Failure> {
    let t = fs::read_to_string(p).map_err(|e| Failure { path: p.display().to_string(), message: e.to_string() })?;
    serde_json::from_str(&t).map_err(|e| Failure { path: p.display().to_string(), message: format!("not JSON: {e}") })
}

/// Lowercase hex SHA-256 of the canonical JSON (sorted keys, no whitespace) of `{purpose, prompt, schema}`.
pub fn recording_key(purpose: &str, prompt: &str, schema: &Value) -> String {
    use sha2::{Digest, Sha256};
    let v = serde_json::json!({"purpose": purpose, "prompt": prompt, "schema": schema});
    let canon = canonical_json(&v);
    let h = Sha256::digest(canon.as_bytes());
    h.iter().map(|b| format!("{b:02x}")).collect()
}

/// Canonical JSON: keys sorted by code point, no whitespace.
pub fn canonical_json(v: &Value) -> String {
    match v {
        Value::Object(o) => {
            let m: BTreeMap<&String, &Value> = o.iter().collect();
            let inner: Vec<String> = m.iter().map(|(k, v)| format!("{}:{}", Value::String((*k).clone()), canonical_json(v))).collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(a) => format!("[{}]", a.iter().map(canonical_json).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

/// Which records each section writes.
pub fn section_records(section: &str) -> &'static [&'static str] {
    match section {
        "map" => &["files", "parts", "blocks", "groups", "dependencies"],
        "proof" => &["rows", "differences", "scope", "lean"],
        "playground" => &["replays"],
        "tests" => &["tests", "links"],
        "rules" => &["levels", "questions", "mode"],
        "review" => &["ai-review"],
        _ => &[],
    }
}

/// Options for [`validate_bundle`].
#[derive(Default, Clone)]
pub struct Options {
    /// A clone of the repository: locations are then checked against the files at their commit.
    pub repo: Option<PathBuf>,
}

type Records = BTreeMap<String, Value>;

fn arr<'a>(r: &'a Records, rec: &str, key: &str) -> Vec<&'a Value> {
    r.get(rec).and_then(|v| v.get(key)).and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default()
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}
fn strs(v: &Value, k: &str) -> Vec<String> {
    v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}
fn lines(v: &Value, k: &str) -> Vec<u64> {
    v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default()
}

/// Optional records in subdirectories of a bundle: (path, schema).
pub const EXTRA_RECORDS: [(&str, &str); 4] = [
    ("map-extra/facts.json", "facts"),
    ("tests-extra/snapshots.json", "snapshots"),
    ("tests-extra/suggested-tests.json", "suggested-tests"),
    ("tests-extra/reverted.json", "reverted"),
];

/// Difference kinds as stored (`differences[].kind`).
pub const DIFFERENCE_KINDS: [&str; 8] =
    ["crash-fixed", "crash", "silent-failure", "null-access", "output-changed", "new-dependency", "public-api", "data-loss"];
/// Kinds the differ assigns (`differences[].solverKind`): all but `data-loss`.
pub const SOLVER_KINDS: [&str; 7] =
    ["crash-fixed", "crash", "silent-failure", "null-access", "output-changed", "new-dependency", "public-api"];
/// Test categories as stored.
pub const TEST_CATEGORIES: [&str; 5] = ["declared", "new-api", "hanging", "withdrawn", "not-run"];

/// A full commit SHA: 40 lowercase hex digits.
pub fn is_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The records of one bundle, keyed by record name (`differences`, `tests`, ...), unvalidated.
#[derive(Default, Clone)]
pub struct Bundle {
    pub records: BTreeMap<String, Value>,
}

impl Bundle {
    /// Read every known record present in `dir`; nothing is validated.
    pub fn load(dir: &Path) -> Result<Bundle, Failure> {
        let mut records = BTreeMap::new();
        for rec in BUNDLE_RECORDS {
            let p = dir.join(format!("{rec}.json"));
            if p.exists() {
                records.insert(rec.to_string(), read_json(&p)?);
            }
        }
        Ok(Bundle { records })
    }
    pub fn from_records(records: BTreeMap<String, Value>) -> Bundle {
        Bundle { records }
    }
}

/// Whether one AI-review cite names an existing item of `b`: an ID `d<n>`, `b<n>`, `q<n>`, `p<n>`;
/// a test name; a test category present in `tests.json`; `preset:d<n>` (that difference has a replay
/// with an input); `reproduction` (a replay input has source `issue`); or `File.cs:<line>` for a
/// changed file (full path or file name). Same rules as `web/src/sections/review/cites.ts`.
pub fn cite_resolves(b: &Bundle, cite: &str) -> bool {
    let r = &b.records;
    let has = |rec: &str, key: &str, field: &str, v: &str| arr(r, rec, key).iter().any(|x| s(x, field) == v);
    let id_like = |c: &str, p: char| c.len() > 1 && c.starts_with(p) && c[1..].bytes().all(|x| x.is_ascii_digit()) && !c[1..].starts_with('0');
    if id_like(cite, 'd') { return has("differences", "differences", "id", cite); }
    if id_like(cite, 'b') { return has("blocks", "blocks", "id", cite); }
    if id_like(cite, 'q') { return has("questions", "questions", "id", cite); }
    if id_like(cite, 'p') { return has("parts", "parts", "id", cite); }
    let replays = arr(r, "replays", "replays");
    if let Some(d) = cite.strip_prefix("preset:") {
        return id_like(d, 'd') && has("differences", "differences", "id", d)
            && replays.iter().any(|x| s(x, "difference") == d && x.get("input").map(|i| !i.is_null()).unwrap_or(false));
    }
    if cite == "reproduction" {
        return replays.iter().any(|x| s(&x["input"], "source") == "issue");
    }
    let tests = arr(r, "tests", "tests");
    if tests.iter().any(|t| s(t, "name") == cite || s(t, "category") == cite) {
        return true;
    }
    if let Some((file, line)) = cite.rsplit_once(':') {
        if line.parse::<u64>().map(|n| n > 0).unwrap_or(false) {
            return arr(r, "files", "files").iter().any(|f| {
                let p = s(f, "file");
                p == file || p.rsplit('/').next() == Some(file)
            });
        }
    }
    false
}

/// Validate one bundle directory, or a session directory, or a recordings directory.
/// Returns the first failure.
pub fn validate_dir(dir: &Path, opts: &Options) -> Result<(), Failure> {
    let schemas = Schemas::contract()?;
    if dir.join("bundle.json").exists() {
        return validate_bundle_with(&schemas, dir, opts);
    }
    let mut any = false;
    let mut entries: Vec<PathBuf> = fs::read_dir(dir).map_err(|e| Failure { path: dir.display().to_string(), message: e.to_string() })?
        .flatten().map(|e| e.path()).filter(|p| p.extension().map(|e| e == "json").unwrap_or(false)).collect();
    entries.sort();
    for p in entries {
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        let name = if schemas.has(&stem) { stem.clone() } else if stem.len() == 64 { "recording".into() } else { continue };
        any = true;
        let v = read_json(&p)?;
        schemas.validate(&name, &v, &format!("{stem}.json"))?;
        if name == "recording" {
            let key = recording_key(s(&v, "purpose"), s(&v, "prompt"), v.get("schema").unwrap_or(&Value::Null));
            if key != stem {
                return fail(format!("{stem}.json"), format!("file name is not the recording key {key}"));
            }
        }
    }
    if !any {
        return fail(dir.display().to_string(), "no bundle.json and no known record");
    }
    Ok(())
}

pub fn validate_bundle(dir: &Path, opts: &Options) -> Result<(), Failure> {
    validate_bundle_with(&Schemas::contract()?, dir, opts)
}

fn validate_bundle_with(schemas: &Schemas, dir: &Path, opts: &Options) -> Result<(), Failure> {
    let mut r: Records = BTreeMap::new();
    for rec in BUNDLE_RECORDS {
        let p = dir.join(format!("{rec}.json"));
        if p.exists() {
            let v = read_json(&p)?;
            schemas.validate(rec, &v, &format!("{rec}.json"))?;
            r.insert(rec.to_string(), v);
        }
    }
    for (path, schema) in EXTRA_RECORDS {
        let p = dir.join(path);
        if p.exists() {
            schemas.validate(schema, &read_json(&p)?, path)?;
        }
    }
    let b = r.get("bundle").ok_or(Failure { path: "bundle.json".into(), message: "missing".into() })?.clone();
    // directory name
    let dn = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if let Some((o, n)) = dn.split_once('-') {
        if o.len() == 40 && n.len() == 40 && (o != s(&b, "oldCommit") || n != s(&b, "newCommit")) {
            return fail("bundle.json/oldCommit", format!("commits do not match directory {dn}"));
        }
    }
    // section states and record presence
    let st = |sec: &str| b["sections"][sec]["state"].as_str().unwrap_or("").to_string();
    for sec in ["map", "proof", "playground", "tests", "rules", "review"] {
        let state = st(sec);
        for rec in section_records(sec) {
            let optional = matches!(*rec, "groups" | "dependencies" | "lean");
            if state == "ready" && !optional && !r.contains_key(*rec) {
                return fail(format!("{rec}.json"), format!("missing while section `{sec}` is ready"));
            }
        }
    }
    if st("tests") != "ready" {
        if st("rules") != "running" {
            return fail("bundle.json/sections/rules/state", "must be `running` while tests are not ready");
        }
        for rec in ["links", "levels", "questions", "mode"] {
            if r.contains_key(rec) {
                return fail(format!("{rec}.json"), "must be absent while tests are not ready");
            }
        }
    }
    // ID sequences
    for (rec, key, pre) in [("parts", "parts", "p"), ("blocks", "blocks", "b"), ("differences", "differences", "d"),
        ("rows", "rows", "r"), ("questions", "questions", "q"), ("groups", "groups", "g")] {
        for (i, x) in arr(&r, rec, key).iter().enumerate() {
            if s(x, "id") != format!("{pre}{}", i + 1) {
                return fail(format!("{rec}.json/{key}/{i}/id"), format!("expected {pre}{}", i + 1));
            }
        }
    }
    let parts: BTreeMap<String, &Value> = arr(&r, "parts", "parts").into_iter().map(|p| (s(p, "id").to_string(), p)).collect();
    let blocks: BTreeMap<String, &Value> = arr(&r, "blocks", "blocks").into_iter().map(|p| (s(p, "id").to_string(), p)).collect();
    let diffs: BTreeMap<String, &Value> = arr(&r, "differences", "differences").into_iter().map(|p| (s(p, "id").to_string(), p)).collect();
    let rows: BTreeMap<String, &Value> = arr(&r, "rows", "rows").into_iter().map(|p| (s(p, "id").to_string(), p)).collect();
    let tests: BTreeMap<String, &Value> = arr(&r, "tests", "tests").into_iter().map(|p| (s(p, "name").to_string(), p)).collect();
    let questions: BTreeSet<String> = arr(&r, "questions", "questions").iter().map(|q| s(q, "id").to_string()).collect();
    let need = |set: &dyn Fn(&str) -> bool, id: &str, path: String| -> Result<(), Failure> {
        if set(id) { Ok(()) } else { fail(path, format!("`{id}` does not exist")) }
    };
    let is_part = |id: &str| parts.contains_key(id);
    let is_block = |id: &str| blocks.contains_key(id);
    let is_diff = |id: &str| diffs.contains_key(id);
    let is_row = |id: &str| rows.contains_key(id);
    let is_test = |id: &str| tests.contains_key(id);
    // files and line coverage
    let files: BTreeMap<String, &Value> = arr(&r, "files", "files").into_iter().map(|f| (s(f, "file").to_string(), f)).collect();
    let mut seen: BTreeMap<(String, char, u64), String> = BTreeMap::new();
    let mut per_file: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for (pid, p) in &parts {
        let file = p["new"].get("file").or(p["old"].get("file")).and_then(Value::as_str).unwrap_or("").to_string();
        let f = files.get(&file).ok_or_else(|| Failure { path: format!("parts.json/{pid}"), message: format!("file `{file}` not in files.json") })?;
        if s(f, "category") != s(p, "category") {
            return fail(format!("parts.json/{pid}/category"), format!("differs from files.json for {file}"));
        }
        for (side, k) in [('+', "addedLines"), ('-', "deletedLines")] {
            for l in lines(p, k) {
                if let Some(other) = seen.insert((file.clone(), side, l), pid.clone()) {
                    let which = if side == '+' { "new" } else { "old" };
                    return fail(format!("{file}:{l}"), format!("{which} line in {other} and {pid}"));
                }
            }
        }
        let e = per_file.entry(file).or_default();
        e.0 += lines(p, "addedLines").len() as u64;
        e.1 += lines(p, "deletedLines").len() as u64;
    }
    for (file, f) in &files {
        let (a, d) = per_file.get(file).copied().unwrap_or_default();
        if a != f["added"].as_u64().unwrap_or(0) || d != f["deleted"].as_u64().unwrap_or(0) {
            return fail(format!("{file}"), format!("parts cover +{a} -{d}, files.json says +{} -{}", f["added"], f["deleted"]));
        }
    }
    // scope per category
    if let Some(sc) = r.get("scope") {
        for cat in ["production", "test", "docs", "snapshot", "generated"] {
            let sum: u64 = parts.values().filter(|p| s(p, "category") == cat)
                .map(|p| (lines(p, "addedLines").len() + lines(p, "deletedLines").len()) as u64).sum();
            if sc["changedLines"][cat].as_u64() != Some(sum) {
                return fail(format!("scope.json/changedLines/{cat}"), format!("parts sum to {sum}"));
            }
        }
        for (i, a) in sc["assumptions"].as_array().into_iter().flatten().enumerate() {
            for rid in strs(a, "rows") { need(&is_row, &rid, format!("scope.json/assumptions/{i}/rows"))?; }
        }
    }
    // blocks
    let mut part_block: BTreeMap<String, String> = BTreeMap::new();
    for (bid, bl) in &blocks {
        let mut bfiles = BTreeSet::new();
        for pid in strs(bl, "parts") {
            need(&is_part, &pid, format!("blocks.json/{bid}/parts"))?;
            if let Some(o) = part_block.insert(pid.clone(), bid.clone()) {
                return fail(format!("blocks.json/{bid}/parts"), format!("`{pid}` is also in {o}"));
            }
            let p = parts[&pid];
            if s(p, "category") != s(bl, "category") {
                return fail(format!("blocks.json/{bid}/category"), format!("differs from {pid}"));
            }
            bfiles.insert(p["new"].get("file").or(p["old"].get("file")).and_then(Value::as_str).unwrap_or("").to_string());
        }
        if matches!(s(bl, "category"), "test" | "docs") && bfiles.len() != 1 {
            return fail(format!("blocks.json/{bid}"), "a test or docs block holds exactly one file");
        }
    }
    for (pid, p) in &parts {
        let cat = s(p, "category");
        if matches!(cat, "production" | "test" | "docs") && !part_block.contains_key(pid) {
            return fail(format!("parts.json/{pid}"), format!("{cat} part in no block"));
        }
    }
    let mut file_block: BTreeMap<String, String> = BTreeMap::new();
    for (pid, bid) in &part_block {
        let p = parts[pid];
        if matches!(s(p, "category"), "test" | "docs") {
            let file = p["new"].get("file").or(p["old"].get("file")).and_then(Value::as_str).unwrap_or("").to_string();
            if let Some(o) = file_block.insert(file.clone(), bid.clone()) {
                if &o != bid {
                    return fail(file, format!("in blocks {o} and {bid}"));
                }
            }
        }
    }
    // differences and rows
    let mut row_diff: BTreeMap<String, String> = BTreeMap::new();
    for (i, (did, d)) in diffs.iter().enumerate() {
        let _ = i;
        let n: u64 = did[1..].parse().unwrap_or(0);
        if d["number"].as_u64() != Some(n) {
            return fail(format!("differences.json/{did}/number"), format!("must equal {n}"));
        }
        let bid = s(d, "block");
        need(&is_block, bid, format!("differences.json/{did}/block"))?;
        if s(blocks[bid], "category") != "production" {
            return fail(format!("differences.json/{did}/block"), "must be a production block");
        }
        let drows = strs(d, "rows");
        if s(d, "checker") == "solver" && drows.is_empty() {
            return fail(format!("differences.json/{did}/rows"), "a solver difference has at least one row");
        }
        if s(d, "checker") != "solver" && !drows.is_empty() {
            return fail(format!("differences.json/{did}/rows"), "only solver differences have rows");
        }
        let kind = s(d, "kind");
        if kind != "data-loss" && kind != s(d, "solverKind") {
            return fail(format!("differences.json/{did}/kind"), "differs from solverKind and is not data-loss");
        }
        let mut tags: Option<(String, String)> = None;
        for rid in &drows {
            need(&is_row, rid, format!("differences.json/{did}/rows"))?;
            if let Some(o) = row_diff.insert(rid.clone(), did.clone()) {
                return fail(format!("differences.json/{did}/rows"), format!("`{rid}` is also in {o}"));
            }
            let row = rows[rid];
            if s(row, "difference") != did {
                return fail(format!("rows.json/{rid}/difference"), format!("listed in {did}"));
            }
            let t = (s(&row["old"], "tag").to_string(), s(&row["new"], "tag").to_string());
            if let Some(prev) = &tags {
                if prev != &t {
                    return fail(format!("rows.json/{rid}"), format!("outcome differs from the other rows of {did}"));
                }
            }
            tags = Some(t);
        }
    }
    for rid in rows.keys() {
        if !row_diff.contains_key(rid) {
            return fail(format!("rows.json/{rid}"), "in no difference");
        }
    }
    // lean
    if let Some(l) = r.get("lean") {
        for (i, x) in l["rows"].as_array().into_iter().flatten().enumerate() {
            need(&is_row, s(x, "row"), format!("lean.json/rows/{i}/row"))?;
        }
    }
    // links
    if r.contains_key("links") {
        let mut linked = BTreeSet::new();
        for (i, l) in arr(&r, "links", "links").iter().enumerate() {
            let bid = s(l, "block");
            need(&is_block, bid, format!("links.json/links/{i}/block"))?;
            if !linked.insert(bid.to_string()) {
                return fail(format!("links.json/links/{i}/block"), format!("`{bid}` linked twice"));
            }
            let ts = strs(l, "tests");
            for t in &ts { need(&is_test, t, format!("links.json/links/{i}/tests"))?; }
            if s(l, "state") == "declared" && !ts.iter().any(|t| matches!(s(tests[t.as_str()], "category"), "declared" | "new-api")) {
                return fail(format!("links.json/links/{i}/state"), "declared needs a declared or new-api test");
            }
        }
        for (bid, bl) in &blocks {
            if s(bl, "category") == "production" && !linked.contains(bid) {
                return fail("links.json/links", format!("production block `{bid}` has no entry"));
            }
        }
    }
    for (i, d) in arr(&r, "dependencies", "dependencies").iter().enumerate() {
        need(&is_block, s(d, "from"), format!("dependencies.json/dependencies/{i}/from"))?;
        need(&is_block, s(d, "to"), format!("dependencies.json/dependencies/{i}/to"))?;
    }
    for (i, g) in arr(&r, "groups", "groups").iter().enumerate() {
        for p in strs(g, "parts") { need(&is_part, &p, format!("groups.json/groups/{i}/parts"))?; }
    }
    for (i, l) in arr(&r, "levels", "levels").iter().enumerate() {
        need(&is_block, s(l, "block"), format!("levels.json/levels/{i}/block"))?;
    }
    for (i, q) in arr(&r, "questions", "questions").iter().enumerate() {
        if let Some(bid) = q["block"].as_str() { need(&is_block, bid, format!("questions.json/questions/{i}/block"))?; }
        for d in strs(q, "differences") { need(&is_diff, &d, format!("questions.json/questions/{i}/differences"))?; }
        for t in strs(q, "tests") { need(&is_test, &t, format!("questions.json/questions/{i}/tests"))?; }
    }
    for (i, x) in arr(&r, "replays", "replays").iter().enumerate() {
        need(&is_diff, s(x, "difference"), format!("replays.json/replays/{i}/difference"))?;
    }
    let _ = &questions;
    if let Some(a) = r.get("ai-review") {
        let bundle = Bundle { records: r.clone() };
        let cite_ok = |c: &str| cite_resolves(&bundle, c);
        for (i, st) in a["statements"].as_array().into_iter().flatten().enumerate() {
            for c in strs(st, "cites") { need(&cite_ok, &c, format!("ai-review.json/statements/{i}/cites"))?; }
        }
    }
    // library facts: a `checked` row resting on a library fact needs that fact listed by its unit
    if let (Some(sc), Some(l)) = (r.get("scope"), r.get("lean")) {
        for (i, a) in sc["assumptions"].as_array().into_iter().flatten().enumerate() {
            if s(a, "kind") != "library-fact" { continue; }
            let key = s(a, "key");
            for rid in strs(a, "rows") {
                let checked = l["rows"].as_array().into_iter().flatten().any(|x| s(x, "row") == rid && s(x, "state") == "checked");
                if !checked { continue; }
                let unit = rows.get(&rid).map(|x| s(x, "unit")).unwrap_or("");
                let listed = l["coverage"].as_array().into_iter().flatten()
                    .filter(|c| { let u = s(c, "unit"); u == unit || u.starts_with(&format!("{unit}#")) || unit.starts_with(&format!("{u}#")) })
                    .any(|c| c["facts"].as_array().into_iter().flatten().any(|f| s(f, "key") == key));
                if !listed {
                    return fail(format!("scope.json/assumptions/{i}"), format!("`{rid}` is checked but unit `{unit}` lists no fact `{key}` in lean.json"));
                }
            }
        }
    }
    // locations
    let mut locs: Vec<(String, &Value, bool)> = Vec::new();
    for (pid, p) in &parts {
        if !p["old"].is_null() { locs.push((format!("parts.json/{pid}/old"), &p["old"], true)); }
        if !p["new"].is_null() { locs.push((format!("parts.json/{pid}/new"), &p["new"], false)); }
    }
    for (did, d) in &diffs { locs.push((format!("differences.json/{did}/location"), &d["location"], false)); }
    for (rid, x) in &rows { locs.push((format!("rows.json/{rid}/firstChanged"), &x["firstChanged"], false)); }
    for (t, x) in &tests { locs.push((format!("tests.json/{t}/location"), &x["location"], s(x, "change") == "removed")); }
    for (i, x) in arr(&r, "dependencies", "dependencies").iter().enumerate() {
        for (j, site) in x["sites"].as_array().into_iter().flatten().enumerate() {
            locs.push((format!("dependencies.json/dependencies/{i}/sites/{j}"), site, s(x, "version") == "old"));
        }
    }
    if let Some(sc) = r.get("scope") {
        for (i, n) in sc["notModelled"].as_array().into_iter().flatten().enumerate() {
            locs.push((format!("scope.json/notModelled/{i}/location"), &n["location"], false));
        }
    }
    let mut counts: HashMap<(String, String), Option<u64>> = HashMap::new();
    for (path, l, old) in locs {
        let (line, end) = (l["line"].as_u64().unwrap_or(0), l["endLine"].as_u64().unwrap_or(0));
        if line > end {
            return fail(path, format!("line {line} is after endLine {end}"));
        }
        if let Some(repo) = &opts.repo {
            let commit = if old { s(&b, "oldCommit") } else { s(&b, "newCommit") }.to_string();
            let file = s(l, "file").to_string();
            let n = *counts.entry((commit.clone(), file.clone())).or_insert_with(|| line_count(repo, &commit, &file));
            match n {
                None => return fail(path, format!("`{file}` does not exist at {commit}")),
                Some(n) if end > n => return fail(path, format!("endLine {end} beyond {n} lines of {file}")),
                _ => {}
            }
        }
    }
    Ok(())
}

/// Line count of `file` at `commit`, read with `git cat-file` (no filters, no hooks).
fn line_count(repo: &Path, commit: &str, file: &str) -> Option<u64> {
    let out = std::process::Command::new("git")
        .arg("-C").arg(repo).args(["-c", "core.hooksPath=/dev/null", "cat-file", "blob"])
        .arg(format!("{commit}:{file}")).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let n = out.stdout.iter().filter(|b| **b == b'\n').count() as u64;
    Some(if out.stdout.last().map(|b| *b != b'\n').unwrap_or(false) { n + 1 } else { n })
}
