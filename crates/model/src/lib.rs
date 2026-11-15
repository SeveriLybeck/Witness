//! Witness model interface (D23, D43, D46).
//!
//! - [`Recorder`]: one interface over `claude -p --json-schema`; every call is stored
//!   under `recordings/<sha256 of canonical {purpose,prompt,schema}>.json`. Replay mode
//!   never calls live and returns [`ModelError::NoRecording`].
//! - [`review_prompt`], [`check_review`], [`review`]: the AI review from bundle facts only,
//!   with citations checked against the bundle.
//! - [`snippet_matches`], [`accept_snippet`]: proposed replay snippets kept only when the
//!   real outcomes match the difference's row.

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub use contracts::{canonical_json, recording_key};

pub const PURPOSE_REVIEW: &str = "ai-review";
pub const PURPOSE_SNIPPET: &str = "replay-snippet";
pub const HEADINGS: [&str; 4] = ["matches", "not-in-description", "described-not-found", "left-to-you"];
pub const BANNED: [&str; 5] = ["proved", "safe", "verified", "trusted", "correct"];
pub const TEST_CATEGORIES: [&str; 5] = ["declared", "new-api", "hanging", "withdrawn", "not-run"];

#[derive(Debug)]
pub enum ModelError {
    /// Replay mode and no file for this key.
    NoRecording(String),
    Io(String),
    Cli(String),
    BadResponse(String),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::NoRecording(k) => write!(f, "no recording {k}"),
            ModelError::Io(e) => write!(f, "io: {e}"),
            ModelError::Cli(e) => write!(f, "cli: {e}"),
            ModelError::BadResponse(e) => write!(f, "bad response: {e}"),
        }
    }
}
impl std::error::Error for ModelError {}

#[derive(Debug, Clone)]
pub enum Mode {
    /// Recordings only; a missing one is `NoRecording`.
    Replay,
    /// Use a recording when present, otherwise run the CLI and record.
    Live { cli: PathBuf },
}

#[derive(Debug, Clone)]
pub struct Recorder {
    pub dir: PathBuf,
    pub mode: Mode,
}

impl Recorder {
    pub fn replay(dir: impl Into<PathBuf>) -> Self {
        Recorder { dir: dir.into(), mode: Mode::Replay }
    }
    pub fn live(dir: impl Into<PathBuf>, cli: impl Into<PathBuf>) -> Self {
        Recorder { dir: dir.into(), mode: Mode::Live { cli: cli.into() } }
    }

    pub fn path_for(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.json"))
    }

    /// Returns `(key, response)`.
    pub fn call(&self, purpose: &str, prompt: &str, schema: &Value) -> Result<(String, Value), ModelError> {
        let key = recording_key(purpose, prompt, schema);
        let path = self.path_for(&key);
        if let Ok(text) = fs::read_to_string(&path) {
            let rec: Value = serde_json::from_str(&text).map_err(|e| ModelError::BadResponse(e.to_string()))?;
            let resp = rec.get("response").cloned().ok_or_else(|| ModelError::BadResponse("no response".into()))?;
            return Ok((key, resp));
        }
        let cli = match &self.mode {
            Mode::Replay => return Err(ModelError::NoRecording(key)),
            Mode::Live { cli } => cli,
        };
        let response = run_cli(cli, prompt, schema)?;
        fs::create_dir_all(&self.dir).map_err(|e| ModelError::Io(e.to_string()))?;
        let rec = json!({"provenance": "pipeline", "purpose": purpose, "prompt": prompt, "schema": schema, "response": response});
        fs::write(&path, serde_json::to_string_pretty(&rec).unwrap()).map_err(|e| ModelError::Io(e.to_string()))?;
        Ok((key, response))
    }
}

/// `claude -p --output-format json --json-schema <schema>`, prompt on stdin.
fn run_cli(cli: &Path, prompt: &str, schema: &Value) -> Result<Value, ModelError> {
    let mut child = Command::new(cli)
        .args(["-p", "--output-format", "json", "--json-schema", &canonical_json(schema)])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ModelError::Cli(e.to_string()))?;
    child.stdin.take().unwrap().write_all(prompt.as_bytes()).map_err(|e| ModelError::Cli(e.to_string()))?;
    let out = child.wait_with_output().map_err(|e| ModelError::Cli(e.to_string()))?;
    if !out.status.success() {
        return Err(ModelError::Cli(format!("exit {:?}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr))));
    }
    let env: Value = serde_json::from_slice(&out.stdout).map_err(|e| ModelError::BadResponse(e.to_string()))?;
    // The envelope carries `structured_output` under --json-schema; else `result` holds JSON text.
    if let Some(v) = env.get("structured_output").filter(|v| !v.is_null()) {
        return Ok(v.clone());
    }
    if let Some(s) = env.get("result").and_then(Value::as_str) {
        return serde_json::from_str(s).map_err(|e| ModelError::BadResponse(e.to_string()));
    }
    if env.is_object() && env.get("type").is_none() {
        return Ok(env);
    }
    Err(ModelError::BadResponse("no structured output".into()))
}

// ---------------------------------------------------------------- bundle

/// The records the review reads, loaded from one bundle directory.
#[derive(Debug, Clone, Default)]
pub struct Bundle {
    pub bundle: Value,
    pub differences: Value,
    pub tests: Value,
    pub levels: Value,
    pub groups: Value,
    pub scope: Value,
    pub questions: Value,
    pub blocks: Value,
    pub parts: Value,
    pub files: Value,
    pub replays: Value,
    /// The directory it was loaded from; the contract validator runs on a copy of it.
    pub dir: Option<PathBuf>,
}

fn read(dir: &Path, name: &str) -> Value {
    fs::read_to_string(dir.join(format!("{name}.json")))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

fn list<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

impl Bundle {
    pub fn load(dir: &Path) -> Bundle {
        Bundle {
            bundle: read(dir, "bundle"),
            differences: read(dir, "differences"),
            tests: read(dir, "tests"),
            levels: read(dir, "levels"),
            groups: read(dir, "groups"),
            scope: read(dir, "scope"),
            questions: read(dir, "questions"),
            blocks: read(dir, "blocks"),
            parts: read(dir, "parts"),
            files: read(dir, "files"),
            replays: read(dir, "replays"),
            dir: Some(dir.to_path_buf()),
        }
    }
    pub fn differences(&self) -> &[Value] { list(&self.differences, "differences") }
    pub fn tests(&self) -> &[Value] { list(&self.tests, "tests") }
    pub fn questions(&self) -> &[Value] { list(&self.questions, "questions") }

    /// IDs named in the prompt (feeds the prompt only; the key depends on it).
    fn prompt_ids(&self) -> BTreeSet<String> {
        let mut c = BTreeSet::new();
        for (rec, key) in [(&self.differences, "differences"), (&self.blocks, "blocks"), (&self.questions, "questions"), (&self.parts, "parts")] {
            for item in list(rec, key) {
                if let Some(id) = item.get("id").and_then(Value::as_str) {
                    c.insert(id.to_string());
                }
            }
        }
        for t in self.tests() {
            c.insert(s(t, "name").to_string());
            if !s(t, "category").is_empty() {
                c.insert(s(t, "category").to_string());
            }
        }
        for r in list(&self.replays, "replays") {
            if r.get("input").map(|i| !i.is_null()).unwrap_or(false) {
                c.insert(format!("preset:{}", s(r, "difference")));
                if r["input"]["source"] == "issue" {
                    c.insert("reproduction".into());
                }
            }
        }
        c
    }

    /// The same records as the contracts crate's bundle, for `contracts::cite_resolves`.
    pub fn contract_bundle(&self) -> contracts::Bundle {
        if let Some(Ok(b)) = self.dir.as_ref().map(|d| contracts::Bundle::load(d)) {
            return b;
        }
        let mut r = std::collections::BTreeMap::new();
        for (k, v) in [
            ("bundle", &self.bundle), ("differences", &self.differences), ("tests", &self.tests),
            ("levels", &self.levels), ("groups", &self.groups), ("scope", &self.scope),
            ("questions", &self.questions), ("blocks", &self.blocks), ("parts", &self.parts),
            ("files", &self.files), ("replays", &self.replays),
        ] {
            if !v.is_null() {
                r.insert(k.to_string(), v.clone());
            }
        }
        contracts::Bundle::from_records(r)
    }
}

// ---------------------------------------------------------------- review

/// The review is a short check: at most this many statements in all, and per heading.
pub const MAX_STATEMENTS: usize = 10;
pub const MAX_PER_HEADING: [usize; 4] = [2, 4, 2, 3];
/// A statement's `text` and a heading's summary are one line of at most this many words.
pub const LINE_WORDS: usize = 12;

/// Schema the model must answer with: one line per heading, a bounded list of one-line
/// statements, the longer wording of a statement in `detail`.
pub fn review_schema() -> Value {
    let line = json!({"type": "string", "minLength": 1, "maxLength": 110});
    json!({
        "type": "object",
        "required": ["summaries", "statements"],
        "properties": {
            "summaries": {"type": "object", "properties": {
                "matches": line, "not-in-description": line, "described-not-found": line, "left-to-you": line
            }},
            "statements": {"type": "array", "maxItems": MAX_STATEMENTS, "items": {
                "type": "object", "required": ["heading", "text", "cites"],
                "properties": {
                    "heading": {"enum": HEADINGS},
                    "text": line,
                    "detail": {"type": "string"},
                    "cites": {"type": "array", "minItems": 1, "items": {"type": "string"}}
                }}}
        }
    })
}

/// The prompt: bundle facts, description and requirement text only. No source text.
pub fn review_prompt(b: &Bundle) -> String {
    let req = &b.bundle["requirement"];
    let mut p = String::new();
    p.push_str("You are checking a pull request for a programmer who will read your answer in ten seconds. ");
    p.push_str("Compare the evidence below with the pull request's description and the requirement. ");
    p.push_str("The reader asks: does this change do what it says, and what did it do that nobody asked for?\n\n");
    p.push_str("Answer with a short check, not a report:\n");
    p.push_str(&format!(
        "- At most {MAX_STATEMENTS} statements in all: matches at most {}, not-in-description at most {}, described-not-found at most {}, left-to-you at most {}. Fewer is better; a heading with nothing to say gets no statement.\n",
        MAX_PER_HEADING[0], MAX_PER_HEADING[1], MAX_PER_HEADING[2], MAX_PER_HEADING[3]
    ));
    p.push_str(&format!(
        "- `text` is one line of at most {LINE_WORDS} words in a C# developer's terms (unhandled exception, diagnostic no longer reported, public API added). No preamble, no hedging, no restating the description.\n"
    ));
    p.push_str("- Group related evidence: the same behaviour at several sites, or several items of one kind, is ONE statement that cites all of them. Never write one statement per difference when they say the same thing.\n");
    p.push_str("- `detail` is optional: one or two sentences shown only when the reader asks for more. Leave it out when `text` says it all.\n");
    p.push_str("- not-in-description is for behaviour or surface the description and requirement do not ask for. Put the largest first. Do not list that the description is short, or documentation and test files that merely accompany the change.\n");
    p.push_str("- left-to-you names a decision only the reviewer can make, as a question, citing its q<n> items; questions about the same thing share one statement.\n");
    p.push_str(&format!(
        "- `summaries` gives each heading that has statements one line of at most {LINE_WORDS} words that stands for all of them.\n"
    ));
    p.push_str("- State only what the evidence below shows. You have not seen the code: name no method, call, cast, operator or mechanism that does not appear in this prompt, in `text`, `detail` or `summaries`, and do not guess how the change was made.\n");
    p.push_str("Every statement cites item IDs from this prompt (d<n>, b<n>, q<n>, test names, test categories, preset:d<n>, reproduction, File.cs:line). ");
    p.push_str("A statement citing anything else is discarded. Never use the words proved, safe, verified, trusted, correct.\n\n");
    p.push_str(&format!("## Requirement {}: {}\n{}\n\n", s(req, "id"), s(req, "title"), s(req, "text")));
    p.push_str(&format!("## Description\n{}\n\n", s(&b.bundle, "description")));
    p.push_str("## Differences\n");
    for d in b.differences() {
        let loc = &d["location"];
        p.push_str(&format!(
            "{} kind={} block={} when {}: old {} -> new {} at {}:{}\n",
            s(d, "id"), s(d, "kind"), s(d, "block"), s(d, "condition"), s(d, "old"), s(d, "new"),
            s(loc, "file"), loc["line"]
        ));
    }
    p.push_str("\n## Tests\n");
    for t in b.tests() {
        p.push_str(&format!("{} change={} category={}\n", s(t, "name"), s(t, "change"), s(t, "category")));
    }
    p.push_str("\n## Blocks and rules fired\n");
    for l in list(&b.levels, "levels") {
        let rules: Vec<&str> = list(l, "rules").iter().filter_map(Value::as_str).collect();
        let name = list(&b.blocks, "blocks").iter().find(|x| x["id"] == l["block"]).map(|x| s(x, "name")).unwrap_or("");
        p.push_str(&format!("{} {} level={} rules={}\n", s(l, "block"), name, l["level"], rules.join(",")));
    }
    p.push_str("\n## Mechanical groups\n");
    for g in list(&b.groups, "groups") {
        p.push_str(&format!("{} {} {}\n", s(g, "id"), s(g, "rule"), s(g, "name")));
    }
    let sc = &b.scope;
    p.push_str(&format!(
        "\n## Scope\nchanged production lines {}, modelled {}, assumptions {}, not modelled {}\n",
        sc["changedLines"]["production"], sc["modelledLines"], list(sc, "assumptions").len(), list(sc, "notModelled").len()
    ));
    p.push_str("\n## Questions (left to the reviewer)\n");
    for q in b.questions() {
        p.push_str(&format!("{} rule={} block={}\n", s(q, "id"), s(q, "rule"), s(q, "block")));
    }
    let presets: Vec<String> = b.prompt_ids().into_iter().filter(|c| c.starts_with("preset:") || c == "reproduction").collect();
    p.push_str(&format!("\n## Replay inputs\n{}\n", presets.join(" ")));
    p
}

fn banned(text: &str) -> bool {
    let t = text.to_lowercase();
    BANNED.iter().any(|w| t.contains(w))
}

/// Check a model response: keep statements whose cites all resolve and which use no banned
/// word, order them by heading, and make `left-to-you` cover every question.
/// Returns an `ai-review.json` value.
pub fn check_review(response: &Value, b: &Bundle, recording: Option<&str>) -> Value {
    let cb = b.contract_bundle();
    let mut kept: Vec<Value> = Vec::new();
    let mut dropped: Vec<Value> = Vec::new();
    for st in list(response, "statements") {
        let heading = s(st, "heading");
        let text = s(st, "text");
        let cites: Vec<String> = list(st, "cites").iter().filter_map(Value::as_str).map(String::from).collect();
        let detail = s(st, "detail").trim();
        let ok = HEADINGS.contains(&heading)
            && !text.trim().is_empty()
            && !cites.is_empty()
            && cites.iter().all(|c| contracts::cite_resolves(&cb, c))
            && !banned(text)
            && !banned(detail);
        if ok {
            let mut k = json!({"heading": heading, "text": text, "cites": cites});
            if !detail.is_empty() && detail != text.trim() {
                k["detail"] = json!(detail);
            }
            kept.push(k);
        } else {
            dropped.push(json!({"text": text, "cites": cites}));
        }
    }
    // Left to you lists every question (D40).
    for q in b.questions() {
        let id = s(q, "id");
        let covered = kept.iter().any(|k| k["heading"] == "left-to-you" && list(k, "cites").iter().any(|c| c == id));
        if !covered {
            kept.push(json!({"heading": "left-to-you", "text": format!("{} {}", s(q, "rule"), s(q, "block")), "cites": [id]}));
        }
    }
    kept.sort_by_key(|k| HEADINGS.iter().position(|h| *h == s(k, "heading")).unwrap_or(4));
    // One line per heading, kept only for headings that have a statement and when it passes the word check.
    let mut summaries = serde_json::Map::new();
    for h in HEADINGS {
        let line = s(&response["summaries"], h).trim();
        if !line.is_empty() && !banned(line) && kept.iter().any(|k| k["heading"] == h) {
            summaries.insert(h.to_string(), json!(line));
        }
    }
    json!({"provenance": "pipeline", "recording": recording, "summaries": summaries, "statements": kept, "dropped": dropped})
}

/// The review for one bundle directory. With no recording (replay mode) it returns the
/// `no AI review` record: `recording: null`, no statements.
pub fn review(bundle_dir: &Path, rec: &Recorder) -> Result<Value, ModelError> {
    let b = Bundle::load(bundle_dir);
    let prompt = review_prompt(&b);
    match rec.call(PURPOSE_REVIEW, &prompt, &review_schema()) {
        Ok((key, resp)) => Ok(check_review(&resp, &b, Some(&key))),
        Err(ModelError::NoRecording(_)) => Ok(no_review()),
        Err(e) => Err(e),
    }
}

pub fn no_review() -> Value {
    json!({"provenance": "pipeline", "recording": null, "statements": [], "dropped": []})
}

// ---------------------------------------------------------------- replay snippets

/// The class of an outcome string, for comparing a row with a real run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    /// Name of the first `...Exception` mentioned, without namespace.
    pub throws: Option<String>,
    /// Diagnostic IDs mentioned (`MsgPack004`, `CS8785`).
    pub ids: BTreeSet<String>,
}

/// Reads both the thrown exception and the diagnostic IDs; "nothing reported" or
/// "0 warnings" has neither.
pub fn classify(outcome: &str) -> Outcome {
    let words: Vec<&str> = outcome
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        .filter(|w| !w.is_empty())
        .collect();
    let throws = words
        .iter()
        .find(|w| w.ends_with("Exception"))
        .map(|e| e.rsplit('.').next().unwrap().to_string());
    let ids: BTreeSet<String> = words
        .iter()
        .filter(|w| {
            let letters = w.trim_end_matches(|c: char| c.is_ascii_digit());
            letters.len() < w.len() && !letters.is_empty() && w.len() - letters.len() >= 3
                && letters.chars().all(|c| c.is_ascii_alphabetic())
        })
        .map(|w| w.to_string())
        .collect();
    Outcome { throws, ids }
}

/// The row's throw (if any) equals the real one, and the row's IDs are all among the real IDs.
/// A row with neither ("nothing reported") needs a real outcome with neither.
pub fn outcome_matches(row: &str, real: &str) -> bool {
    let (r, x) = (classify(row), classify(real));
    if r.throws.is_none() && r.ids.is_empty() {
        return x.throws.is_none() && x.ids.is_empty();
    }
    (r.throws.is_none() || r.throws == x.throws) && r.ids.is_subset(&x.ids)
}

/// A proposed snippet is kept only when both real outcomes match the difference's row (D23).
pub fn snippet_matches(difference: &Value, real_old: &str, real_new: &str) -> bool {
    outcome_matches(s(difference, "old"), real_old) && outcome_matches(s(difference, "new"), real_new)
}

/// Build a `replays.json` entry: `seen-live` with source `model` when it matches, otherwise
/// the snippet is discarded (`input: null`, `not-replayed`).
pub fn accept_snippet(difference: &Value, snippet: &str, real_old: &str, real_new: &str) -> Value {
    let id = s(difference, "id");
    if snippet_matches(difference, real_old, real_new) {
        json!({"difference": id, "input": {"source": "model", "text": snippet},
               "old": real_old, "new": real_new, "state": "seen-live", "roundTrip": null})
    } else {
        json!({"difference": id, "input": null, "old": null, "new": null, "state": "not-replayed", "roundTrip": null})
    }
}

pub fn snippet_schema() -> Value {
    json!({"type": "object", "required": ["snippet"], "properties": {"snippet": {"type": "string", "minLength": 1}}})
}

/// Prompt for a proposed replay snippet: the difference's row and the requirement text.
pub fn snippet_prompt(b: &Bundle, difference: &Value) -> String {
    let req = &b.bundle["requirement"];
    format!(
        "Write a C# consumer snippet (// lib and // app sections) whose build shows this difference.\n\
         Difference {}: kind {}, when {}: old {} -> new {}.\n\n## Requirement {}\n{}\n",
        s(difference, "id"), s(difference, "kind"), s(difference, "condition"), s(difference, "old"), s(difference, "new"),
        s(req, "id"), s(req, "text")
    )
}

/// Ask the recorder for a snippet for one difference.
pub fn propose_snippet(b: &Bundle, difference: &Value, rec: &Recorder) -> Result<String, ModelError> {
    let (_, resp) = rec.call(PURPOSE_SNIPPET, &snippet_prompt(b, difference), &snippet_schema())?;
    resp.get("snippet").and_then(Value::as_str).map(String::from).ok_or_else(|| ModelError::BadResponse("no snippet".into()))
}
