//! Witness Lean certifier (group 7). See STATUS.md.

pub mod check;
pub mod emit;
pub mod fold;
pub mod mirror;
pub mod proofs;

use check::{run_lean, Limits};
use emit::{render, Bridge, Model, SolverRows, Theorem};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Row and coverage states, as in `lean.schema.json`.
pub const CHECKED: &str = "checked";
pub const MODEL_CHECKED: &str = "model-checked";
pub const NOT_CHECKED: &str = "not-checked";

/// Wording per state (D2, D17).
pub fn wording(state: &str) -> &'static str {
    match state {
        CHECKED => "checked by Lean",
        MODEL_CHECKED => "proof found by a model, checked by Lean",
        "checking" => "checking",
        _ => "found by the solver, not checked by Lean",
    }
}

#[derive(Default)]
pub struct Options<'a> {
    pub limits: Limits,
    /// test hooks: replace a theorem's proof, add declarations, drop rows from coverage
    pub proof_override: BTreeMap<String, String>,
    pub extra_decls: Vec<String>,
    pub drop_from_coverage: BTreeSet<u64>,
    pub proofs: Option<&'a dyn proofs::ProofSource>,
}

#[derive(Clone, Debug)]
pub struct RowResult {
    pub n: u64,
    pub row: Option<String>,
    pub state: &'static str,
    pub reason: String,
    pub seconds: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct UnitResult {
    pub unit: String,
    pub file: String,
    pub rows: Vec<RowResult>,
    pub coverage: &'static str,
    pub coverage_reason: String,
    pub seconds: Option<f64>,
    /// library facts every lemma of the unit takes as hypotheses: (key, text, Lean form)
    pub facts: Vec<(String, String, String)>,
    /// per-iteration unit: `no-exception` of the new body per iteration and lifted to any length
    pub per_iteration: Option<&'static str>,
    pub lifted: Option<&'static str>,
    pub lift_reason: String,
    /// per-iteration unit: loop-carried locals (`carriedNames` of either model); Lean does not compare them
    pub carried: Vec<String>,
}

/// What `lean.json` says about carried loop state, so a reader does not take the Lean lemmas for more.
pub const CARRIED_NOTE: &str = "Lean checks each row's outcome (reports, events, throws, value) for one iteration and the no-exception lift over any number of iterations; the values of carried loop state after the iteration are compared by the solver only";

fn carried_names(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if k == "carriedNames" {
                    for n in x.as_array().into_iter().flatten().filter_map(Value::as_str) {
                        if !out.iter().any(|y| y == n) {
                            out.push(n.to_string());
                        }
                    }
                } else {
                    carried_names(x, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| carried_names(x, out)),
        _ => {}
    }
}

impl UnitResult {
    /// `lean.json` (rows with a row ID only; identical rows have none).
    pub fn to_json(&self) -> Value {
        let rows: Vec<Value> = self
            .rows
            .iter()
            .filter_map(|r| {
                r.row.as_ref().map(|id| json!({"row": id, "state": r.state, "file": self.file, "seconds": r.seconds}))
            })
            .collect();
        let ids: Vec<&String> = self.rows.iter().filter_map(|r| r.row.as_ref()).collect();
        let facts: Vec<Value> = self
            .facts
            .iter()
            .map(|(k, t, l)| json!({"key": k, "text": t, "lean": l, "rows": ids}))
            .collect();
        let mut j = json!({"rows": rows, "coverage": [{"unit": self.unit, "state": self.coverage, "file": self.file, "seconds": self.seconds, "facts": facts}]});
        if let Some(p) = self.per_iteration {
            j["loops"] = json!([{"unit": self.unit, "property": "no-exception", "version": "new", "perIteration": p, "lifted": self.lifted.unwrap_or(NOT_CHECKED), "reason": self.lift_reason, "file": self.file, "carried": {"names": self.carried, "lean": NOT_CHECKED, "note": CARRIED_NOTE}}]);
        }
        j
    }
    pub fn row(&self, n: u64) -> &RowResult {
        self.rows.iter().find(|r| r.n == n).expect("row")
    }
}

pub fn file_name(unit: &str) -> String {
    let u: String = unit.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' { c } else { '_' }).collect();
    format!("{u}.lean")
}

/// Emit, check and (for rows the fixed script left open) try model proofs.
/// Writes `<out>/lean/<unit>.lean`; the `file` field is relative to `out`.
pub fn certify(old: &Model, new: &Model, rows: &SolverRows, bridge: &Bridge, out: &Path, opts: &Options) -> Result<UnitResult, String> {
    let dir = out.join("lean");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = file_name(&rows.unit);
    let path: PathBuf = dir.join(&name);
    let mut em = if mirror::is_mirror(rows) {
        mirror::emit(old, new, rows, &opts.drop_from_coverage)
    } else {
        emit::emit(old, new, rows, bridge, &opts.drop_from_coverage)
    };
    for t in em.theorems.iter_mut() {
        if let Some(p) = opts.proof_override.get(&t.name) {
            t.proof = p.clone();
        }
    }
    let (text, spans) = render(&em.header, &opts.extra_decls, &em.theorems);
    std::fs::write(&path, &text).map_err(|e| e.to_string())?;
    let run = run_lean(&path, &spans, opts.limits);
    let secs = Some(run.seconds);

    let mut results = vec![];
    for r in &rows.rows {
        let tname = format!("row_{}", r.n);
        let (mut state, mut reason) = match (em.unbound.get(&r.n), run.verdicts.get(&tname)) {
            (Some(why), _) => (NOT_CHECKED, why.clone()),
            (None, Some(v)) if v.accepted => (CHECKED, String::new()),
            (None, Some(v)) => (NOT_CHECKED, v.reason.clone()),
            (None, None) => (NOT_CHECKED, "no lemma".into()),
        };
        let mut seconds = secs;
        if state == NOT_CHECKED && em.unbound.get(&r.n).is_none() {
            if let (Some(src), Some(t)) = (opts.proofs, em.theorems.iter().find(|t| t.name == tname)) {
                if let Some((s, why)) = model_proof(&em.header, t, &reason, src, &dir, opts.limits) {
                    state = MODEL_CHECKED;
                    seconds = Some(s);
                    reason = why;
                }
            }
        }
        results.push(RowResult { n: r.n, row: r.row.clone(), state, reason, seconds });
    }
    let cv = run.verdicts.get("coverage");
    let (coverage, coverage_reason) = match cv {
        Some(v) if v.accepted => (CHECKED, String::new()),
        Some(v) => (NOT_CHECKED, em.coverage_note.clone().map(|n| format!("{n}; {}", v.reason)).unwrap_or(v.reason.clone())),
        None => (NOT_CHECKED, "no lemma".into()),
    };
    let st = |n: &str| match run.verdicts.get(n) {
        Some(v) if v.accepted => CHECKED,
        _ => NOT_CHECKED,
    };
    let (per_iteration, lifted) = if em.element.is_some() { (Some(st("body_nothrow")), Some(st("loop_nothrow"))) } else { (None, None) };
    let lift_reason = em.lift_note.clone().unwrap_or_else(|| run.verdicts.get("loop_nothrow").map(|v| v.reason.clone()).unwrap_or_default());
    let mut carried = vec![];
    if em.element.is_some() {
        carried_names(&old.body, &mut carried);
        carried_names(&new.body, &mut carried);
    }
    Ok(UnitResult {
        unit: rows.unit.clone(),
        file: format!("lean/{name}"),
        rows: results,
        coverage,
        coverage_reason,
        seconds: secs,
        facts: em.facts.clone(),
        per_iteration,
        lifted,
        lift_reason,
        carried,
    })
}

fn model_proof(header: &str, t: &Theorem, first_error: &str, src: &dyn proofs::ProofSource, dir: &Path, limits: Limits) -> Option<(f64, String)> {
    let mut last = first_error.to_string();
    for attempt in 1..=proofs::MAX_ATTEMPTS {
        let p = src.propose(&t.statement, &last, attempt)?;
        if let Some(w) = proofs::forbidden(&p) {
            last = format!("forbidden token {w}");
            continue;
        }
        let mut t2 = t.clone();
        t2.proof = p;
        let (text, spans) = render(header, &[], &[t2]);
        let path = dir.join(format!("{}.model{attempt}.lean", t.name));
        std::fs::write(&path, text).ok()?;
        let run = run_lean(&path, &spans, limits);
        match run.verdicts.get(&t.name) {
            Some(v) if v.accepted => return Some((run.seconds, format!("attempt {attempt}"))),
            Some(v) => last = v.reason.clone(),
            None => last = "no verdict".into(),
        }
    }
    None
}
