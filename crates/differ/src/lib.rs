//! Witness differ (group 6): where do the old and new model of a unit behave differently?
//!
//! `diff_models` enumerates the feasible paths of both versions through one `z3 -in`
//! process, pairs them, and classifies each feasible pair as identical or differing. It adds
//! the unspecified properties of D18 as rows and assigns each differing row a kind (D21).

pub mod bridge;
pub mod exec;
pub mod interp;
pub mod model;
pub mod read;
pub mod smt;

use exec::{differs, Exec, Leaf};
use model::{Loc, Model};
use serde_json::{json, Value};
use smt::{Sat, Z3};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Outcome {
    pub tag: String,
    pub text: String,
}

impl Outcome {
    pub fn new(tag: &str, text: &str) -> Outcome {
        Outcome { tag: tag.into(), text: text.into() }
    }
    pub fn to_json(&self) -> Value {
        json!({"tag": self.tag, "text": self.text})
    }
}

/// Outcome from what a path did: a throw wins, then reports, then a value or effect.
pub fn outcome_of(throw: Option<&str>, reports: &[String], calls: &[String], value: Option<&str>) -> Outcome {
    if let Some(x) = throw {
        // Everything observable on the path: earlier reports in order, then the throw.
        return if reports.is_empty() {
            Outcome::new("throws", x)
        } else {
            Outcome::new("throws", &format!("reports {}, throws {x}", reports.join(", ")))
        };
    }
    if !reports.is_empty() {
        return Outcome::new("reports", &reports.join(", "));
    }
    if let Some(v) = value {
        return Outcome::new("value", v);
    }
    if !calls.is_empty() {
        return Outcome::new("value", &format!("calls {}", calls.join(", ")));
    }
    Outcome::new("nothing", "nothing reported")
}

/// Effect calls of a path as shown, in order (consecutive repeats once), with the call that threw last.
fn effects(l: &Leaf) -> Vec<String> {
    let mut v: Vec<String> = vec![];
    for e in l.path.events.iter().filter(|e| !e.report) {
        if v.last() != Some(&e.shown) {
            v.push(e.shown.clone());
        }
    }
    if let Err(exec::Thr { call: Some(c), .. }) = &l.end {
        v.push(c.clone());
    }
    v
}

/// A long value is cut for display.
fn clip(v: &str) -> String {
    if v.chars().count() > 100 { v.chars().take(97).collect::<String>() + "…" } else { v.to_string() }
}

/// The outcome of one path, read against the other version's path of the same row: every observable
/// thing in order (reports, effect calls, an early end of the iteration, the value, the throw), then
/// `without X` for each effect the other path performs or attempts and this one does not.
/// The tag keeps the order throw, reports, value or effect, nothing (Lean mirrors the tags).
fn leaf_outcome(l: &Leaf, other: &Leaf, reader: &read::Reader) -> Outcome {
    let reports: Vec<String> = l.path.events.iter().filter(|e| e.report).map(|e| e.name.clone()).collect();
    let all = effects(l);
    let done: Vec<String> = match &l.end {
        Err(exec::Thr { call: Some(_), .. }) => all[..all.len() - 1].to_vec(),
        _ => all.clone(),
    };
    let ends = |x: &String| x == exec::SKIP || x == exec::LEAVE;
    // repeats of one call in a row are counted
    let mut runs: Vec<(String, usize)> = vec![];
    for e in l.path.events.iter().filter(|e| !e.report && !ends(&e.shown)) {
        match runs.last_mut() {
            Some((n, c)) if *n == e.shown => *c += 1,
            _ => runs.push((e.shown.clone(), 1)),
        }
    }
    let calls: Vec<String> = runs.iter().map(|(n, c)| if *c > 1 { format!("{n} ×{c}") } else { n.clone() }).collect();
    let value = match &l.end {
        Ok(v) if v.sort != exec::S::Unit => Some(clip(&reader.literal(&v.s))),
        _ => None,
    };
    let throw = l.end.as_ref().err().map(|t| t.exc.rsplit('.').next().unwrap_or(&t.exc).to_string());
    let without: Vec<String> = if throw.is_some() {
        vec![]
    } else {
        let mut w: Vec<String> = vec![];
        for x in effects(other) {
            if !ends(&x) && !all.contains(&x) && !w.contains(&x) {
                w.push(x);
            }
        }
        w
    };
    let tag = if throw.is_some() {
        "throws"
    } else if !reports.is_empty() {
        "reports"
    } else if value.is_some() || !done.is_empty() {
        "value"
    } else {
        "nothing"
    };
    let mut parts: Vec<String> = vec![];
    if !reports.is_empty() {
        parts.push(format!("reports {}", reports.join(", ")));
    }
    if !calls.is_empty() {
        parts.push(format!("calls {}", calls.join(", ")));
    }
    parts.extend(done.iter().filter(|x| ends(x)).cloned());
    if let Some(v) = &value {
        parts.push(format!("returns {v}"));
    }
    if let Some(x) = &throw {
        parts.push(format!("throws {x}"));
    }
    if !without.is_empty() {
        parts.push(format!("without {}", without.join(", ")));
    }
    // the plain cases keep their short form: the tag says "throws" or "reports"
    let text = match (parts.len(), tag) {
        (0, _) => "nothing reported".to_string(),
        (1, "throws") => throw.unwrap(),
        (1, "reports") => reports.join(", "),
        _ => parts.join(", "),
    };
    Outcome::new(tag, &text)
}

/// The fixed kind table of D21 over the old and new outcome.
pub fn kind_of(old: &Outcome, new: &Outcome, property: Option<&str>, new_null_receiver: bool, public_api: bool) -> &'static str {
    let (ot, nt) = (old.tag == "throws", new.tag == "throws");
    if ot && !nt {
        "crash-fixed"
    } else if nt && !ot && (new.text.contains("NullReferenceException") || new_null_receiver) {
        "null-access"
    } else if nt {
        "crash"
    } else if old.tag == "reports" && new.tag == "nothing" {
        "silent-failure"
    } else if property == Some("no-new-dependency") {
        "new-dependency"
    } else if public_api {
        "public-api"
    } else {
        "output-changed"
    }
}

#[derive(Clone, Debug)]
pub struct Row {
    pub n: usize,
    /// rows.json id (`r1`…); None when old and new agree.
    pub id: Option<String>,
    pub literals: Vec<String>,
    /// The literals in the source's terms, for display (`rows.json` `condition`).
    pub condition: Vec<String>,
    pub old: Outcome,
    pub new: Outcome,
    pub assumptions: Vec<String>,
    pub kind: Option<&'static str>,
    pub property: Option<&'static str>,
    pub per_iteration: bool,
    pub first_changed: Option<Loc>,
    /// `(get-value …)` over the inputs for a differing row.
    pub example: Option<String>,
    /// The solver answered unknown; the row is kept as differing (playbook D2).
    pub undecided: bool,
    /// Library facts (`exec::FACTS` keys) asserted on the row's old or new path.
    pub facts: Vec<&'static str>,
}

#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Contracts switched off (e.g. `index-empty`) for ablation.
    pub disabled: Vec<String>,
    /// Number of the first row id (`r<first_row>`); default 1.
    pub first_row: usize,
    /// Skip the dependency property (faster).
    pub no_dependency: bool,
    /// The unit's file, used when the models carry no location (`--file`).
    pub file: Option<String>,
    /// Changed lines of the new side in the unit's file (`--changed`); overrides the model's `lines`.
    pub changed: Vec<u64>,
    /// The unit is new code with no old version (D18): `old` is an empty model, and only the paths of `new`
    /// that throw become rows (`property: no-exception`).
    pub new_only: bool,
}

/// The unit's site: its real file, and the `#<line>` of the unit id when that is a changed
/// line, else its first changed line. An error names the unit when either is unknown.
pub fn unit_site(old: &Model, new: &Model, opts: &Options) -> Result<Loc, String> {
    let file = [new, old]
        .iter()
        .find_map(|m| m.body.first_loc().map(|l| l.file.clone()).or_else(|| m.not_modelled.first().map(|(l, _)| l.file.clone())))
        .or_else(|| opts.file.clone())
        .ok_or_else(|| format!("unit {}: no source file in the models (pass --file)", new.unit))?;
    let c = changed_lines(new, opts);
    let site = new.unit.rsplit_once('#').and_then(|(_, s)| s.parse::<u64>().ok()).filter(|s| c.contains(s));
    let line = site.or(c.first().copied()).ok_or_else(|| format!("unit {}: the new model names no changed line", new.unit))?;
    Ok(Loc { file, line, end_line: line })
}

#[derive(Clone, Debug)]
pub struct UnitResult {
    pub unit: String,
    pub declarations: Vec<String>,
    pub rows: Vec<Row>,
    /// Inputs only the new model has; not checked for new dependencies (see STATUS.md).
    pub new_inputs: Vec<String>,
    pub queries: usize,
    pub old_paths: usize,
    pub new_paths: usize,
}

impl UnitResult {
    pub fn differing(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter().filter(|r| r.id.is_some())
    }
}

fn union_params(old: &Model, new: &Model) -> Result<Vec<(String, String)>, String> {
    let mut v = old.params.clone();
    for (n, s) in &new.params {
        match v.iter().find(|(m, _)| m == n) {
            Some((_, s2)) if exec::smt_sort(exec::sort_of(s2)) != exec::smt_sort(exec::sort_of(s)) => {
                return Err(format!("input {n} has sort {s2} in old and {s} in new"))
            }
            Some(_) => {}
            None => v.push((n.clone(), s.clone())),
        }
    }
    Ok(v)
}

fn dedup(v: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for x in v {
        if !out.contains(&x) {
            out.push(x);
        }
    }
    out
}

/// Changed lines of the new side: the new model's modelled and not-modelled lines (the
/// translator fills `lines` from the change's parts).
fn changed_lines(new: &Model, opts: &Options) -> Vec<u64> {
    if !opts.changed.is_empty() {
        let mut c = opts.changed.clone();
        c.sort();
        c.dedup();
        return c;
    }
    let mut c = new.modelled.clone();
    for (l, _) in &new.not_modelled {
        c.extend(l.line..=l.end_line);
    }
    c.sort();
    c.dedup();
    c
}

/// Always a changed line when the model names any: the old throw site if changed, else the
/// last located decision on the new path that is changed, else a changed line reached on
/// the new path, else the unit's site line (`Type.M#<line>`), else the first changed line.
fn first_changed(o: &Leaf, n: &Leaf, old: &Model, new: &Model, c: &[u64]) -> Option<Loc> {
    if c.is_empty() {
        return None;
    }
    let file = new.body.first_loc().or(old.body.first_loc()).map(|l| l.file.clone());
    let ok = |l: &&Loc| c.contains(&l.line) && Some(&l.file) == file.as_ref();
    let mut cands: Vec<&Loc> = vec![];
    if let (Err(th), Ok(_)) = (&o.end, &n.end) {
        cands.extend(th.loc.iter());
    }
    cands.extend(n.path.fork_locs.iter().rev());
    cands.extend(n.path.locs.iter());
    // then the first changed branch test the new path takes (the first changed statement on its path)
    let by_test = || n.path.lines.iter().find(|l| c.contains(l)).and_then(|l| file.clone().map(|f| Loc { file: f, line: *l, end_line: *l }));
    // None: the caller falls back to the unit site.
    cands.into_iter().find(ok).map(|l| Loc { end_line: l.line, ..l.clone() }).or_else(by_test)
}

/// Is some observation of `m` changed by giving input `p` a second value?
fn depends(z: &mut Z3, m: &Model, params: &[(String, String)], base: &[Leaf], p: &str, disabled: &[String]) -> Result<bool, String> {
    if !m.params.iter().any(|(n, _)| n == p) {
        return Ok(false);
    }
    let rename = HashMap::from([(p.to_string(), format!("{p}__2"))]);
    let copy = Exec::new(z, m, disabled, rename).paths(m, params)?;
    for a in base {
        for b in &copy {
            let d = differs(a, b);
            if d == "false" {
                continue;
            }
            let mut asserts = a.path.asserts();
            asserts.extend(b.path.asserts());
            asserts.push(d);
            if z.check(&asserts, &[])?.0 != Sat::Unsat {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Does the disjunction of the path conditions cover every input?
fn uncovered(z: &mut Z3, leaves: &[Leaf]) -> Result<bool, String> {
    let mut asserts: Vec<String> = dedup(leaves.iter().flat_map(|l| l.path.facts.clone()));
    let conj = |pc: &[String]| if pc.is_empty() { "true".to_string() } else { format!("(and {})", pc.join(" ")) };
    asserts.push(format!("(not (or false {}))", leaves.iter().map(|l| conj(&l.path.pc)).collect::<Vec<_>>().join(" ")));
    Ok(z.check(&asserts, &[])?.0 == Sat::Sat)
}

/// Compare two versions of one unit.
pub fn diff_models(old: &Model, new: &Model, opts: &Options) -> Result<UnitResult, String> {
    let mut z = Z3::start()?;
    let params = union_params(old, new)?;
    let changed = changed_lines(new, opts);
    let po = Exec::new(&mut z, old, &opts.disabled, HashMap::new()).paths(old, &params)?;
    let pn = Exec::new(&mut z, new, &opts.disabled, HashMap::new()).paths(new, &params)?;
    let get: Vec<String> = params.iter().map(|(n, _)| smt::q(n)).collect();
    let opaque: HashMap<String, model::Opaque> = old.opaque.iter().chain(&new.opaque).map(|o| (o.name.clone(), o.clone())).collect();
    let reader = read::Reader { opaque: &opaque };
    let mut rows: Vec<Row> = vec![];
    let push =|rows: &mut Vec<Row>, mut r: Row| {
        r.n = rows.len();
        rows.push(r);
    };
    for o in &po {
        for n in &pn {
            let mut asserts = o.path.asserts();
            asserts.extend(n.path.asserts());
            if z.check(&asserts, &[])?.0 == Sat::Unsat {
                continue;
            }
            let lits = dedup(o.path.pc.iter().chain(&n.path.pc).cloned());
            let (mut oo, no) = (leaf_outcome(o, n, &reader), leaf_outcome(n, o, &reader));
            if opts.new_only {
                oo = Outcome::new("nothing", "not present");
            }
            let readable = dedup(o.path.readable.iter().chain(&n.path.readable).filter(|c| *c != "null is null").cloned());
            let assumptions = dedup(o.path.assumptions.iter().chain(&n.path.assumptions).cloned());
            let per_iteration = o.path.per_iteration || n.path.per_iteration;
            let new_throws = n.end.is_err();
            let base = Row {
                n: 0,
                id: None,
                literals: lits.clone(),
                condition: readable,
                old: oo.clone(),
                new: no.clone(),
                assumptions,
                kind: None,
                property: None,
                per_iteration,
                first_changed: first_changed(o, n, old, new, &changed),
                example: None,
                undecided: false,
                facts: { let mut f = o.path.fact_keys.clone(); for k in &n.path.fact_keys { if !f.contains(k) { f.push(k) } } f },
            };
            let d = differs(o, n);
            let diff_row = |extra: Option<String>, sat: (Sat, Option<String>)| {
                let mut r = base.clone();
                if let Some(x) = extra {
                    r.condition.push(reader.literal(&x));
                    r.literals.push(x);
                }
                r.property = if new_throws { Some("no-exception") } else { None };
                r.kind = Some(kind_of(&oo, &no, r.property, n.path.null_receiver, false));
                if r.old == r.new {
                    // same events, other arguments or another value
                    r.new = Outcome::new(&r.new.tag, &format!("{}, with other values", r.new.text));
                }
                r.example = sat.1;
                r.undecided = sat.0 == Sat::Unknown;
                r
            };
            let identical = |extra: Option<String>| {
                let mut r = base.clone();
                if let Some(x) = extra {
                    r.condition.push(reader.literal(&x));
                    r.literals.push(x);
                }
                r
            };
            if opts.new_only {
                // new code (D18): only a path that throws is a row of its own
                if new_throws {
                    push(&mut rows, diff_row(None, (Sat::Sat, None)));
                } else {
                    push(&mut rows, identical(None));
                }
                continue;
            }
            if d == "false" {
                push(&mut rows, identical(None));
                continue;
            }
            let mut with_d = asserts.clone();
            with_d.push(d.clone());
            let sat_d = z.check(&with_d, &get)?;
            if sat_d.0 == Sat::Unsat {
                push(&mut rows, identical(None));
                continue;
            }
            if d == "true" {
                push(&mut rows, diff_row(None, sat_d));
                continue;
            }
            let mut with_nd = asserts.clone();
            with_nd.push(format!("(not {d})"));
            if z.check(&with_nd, &[])?.0 == Sat::Unsat {
                push(&mut rows, diff_row(None, sat_d));
            } else {
                push(&mut rows, diff_row(Some(d.clone()), sat_d));
                push(&mut rows, identical(Some(format!("(not {d})"))));
            }
        }
    }
    // D18: every input is handled.
    for (leaves, version) in [(&po, "old"), (&pn, "new")] {
        if uncovered(&mut z, leaves)? {
            let o = Outcome::new("nothing", "unhandled input");
            push(&mut rows, Row {
                n: 0,
                id: None,
                literals: vec![],
                condition: vec![],
                old: if version == "old" { o.clone() } else { Outcome::new("nothing", "handled") },
                new: if version == "new" { o.clone() } else { Outcome::new("nothing", "handled") },
                assumptions: vec![],
                kind: Some("output-changed"),
                property: Some("all-inputs"),
                per_iteration: false,
                first_changed: new.body.first_loc().cloned(),
                example: None,
                undecided: false,
                facts: vec![],
            });
        }
    }
    // D18: no output newly depends on an input the old version ignored.
    let mut new_inputs = vec![];
    if !opts.no_dependency && !opts.new_only {
        for (p, _) in &new.params {
            if !old.params.iter().any(|(n, _)| n == p) {
                new_inputs.push(p.clone());
                continue;
            }
            if depends(&mut z, new, &params, &pn, p, &opts.disabled)? && !depends(&mut z, old, &params, &po, p, &opts.disabled)? {
                push(&mut rows, Row {
                    n: 0,
                    id: None,
                    literals: vec![],
                    condition: vec![],
                    old: Outcome::new("value", &format!("independent of {p}")),
                    new: Outcome::new("value", &format!("depends on {p}")),
                    assumptions: vec![],
                    kind: Some("new-dependency"),
                    property: Some("no-new-dependency"),
                    per_iteration: pn.iter().any(|l| l.path.per_iteration),
                    first_changed: new.body.first_loc().cloned(),
                    example: None,
                    undecided: false,
                facts: vec![],
                });
            }
        }
    }
    // Every row's location is a changed line of the unit, or the unit's site.
    let site = unit_site(old, new, opts);
    for r in rows.iter_mut() {
        let bad = match &r.first_changed {
            None => true,
            Some(l) => !changed.contains(&l.line),
        };
        if bad {
            match &site {
                Ok(s) => r.first_changed = Some(s.clone()),
                Err(e) if r.kind.is_some() => return Err(e.clone()),
                Err(_) => r.first_changed = None,
            }
        }
    }
    let mut next = opts.first_row.max(1);
    for r in rows.iter_mut() {
        if r.kind.is_some() {
            r.id = Some(format!("r{next}"));
            next += 1;
        }
    }
    Ok(UnitResult {
        unit: new.unit.clone(),
        declarations: z.decls.clone(),
        rows,
        new_inputs,
        queries: z.queries,
        old_paths: po.len(),
        new_paths: pn.len(),
    })
}

/// `solver-rows.json` (contracts/messages/solver-rows.schema.json).
pub fn solver_rows_json(r: &UnitResult) -> Value {
    json!({
        "provenance": "pipeline",
        "unit": r.unit,
        "declarations": r.declarations.iter().filter(|d| !d.contains("__2 ")).collect::<Vec<_>>(),
        "rows": r.rows.iter().map(|x| json!({
            "n": x.n, "row": x.id, "literals": x.literals, "old": x.old.to_json(), "new": x.new.to_json(),
            "assumptions": x.assumptions})).collect::<Vec<_>>(),
    })
}

/// Side record for the bundle builder: what solver-rows.json does not carry.
pub fn detail_json(r: &UnitResult) -> Value {
    json!({
        "unit": r.unit,
        "checker": format!("solver ({})", smt::version().unwrap_or_default()),
        "newInputs": r.new_inputs,
        "paths": {"old": r.old_paths, "new": r.new_paths},
        "queries": r.queries,
        "rows": r.differing().map(|x| json!({
            "row": x.id, "kind": x.kind, "solverKind": x.kind, "property": x.property, "perIteration": x.per_iteration,
            "firstChanged": x.first_changed.as_ref().map(Loc::to_json), "condition": x.condition.join(" and "),
            "example": x.example, "undecided": x.undecided, "facts": x.facts})).collect::<Vec<_>>(),
        "libraryFacts": library_facts(r).iter().map(|(f, rows)| json!({"key": f.key, "symbol": f.symbol, "text": f.text, "rows": rows})).collect::<Vec<_>>(),
        "groups": group(r.differing()).iter().map(|g| json!({
            "kind": g.kind, "old": g.old_tag, "new": g.new_tag,
            "location": g.location.as_ref().map(Loc::to_json), "rows": g.rows})).collect::<Vec<_>>(),
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub kind: &'static str,
    pub old_tag: String,
    pub old_text: String,
    pub new_text: String,
    pub new_tag: String,
    pub location: Option<Loc>,
    pub rows: Vec<String>,
}

/// Group differing rows into differences: same kind, same old and new tag, same first
/// changed location. Order follows the first row of each group.
pub fn group<'a>(rows: impl IntoIterator<Item = &'a Row>) -> Vec<Group> {
    let mut gs: Vec<Group> = vec![];
    for r in rows {
        let (Some(id), Some(kind)) = (&r.id, r.kind) else { continue };
        match gs.iter_mut().find(|g| g.kind == kind && g.old_text == r.old.text && g.new_text == r.new.text && g.old_tag == r.old.tag && g.new_tag == r.new.tag && g.location == r.first_changed) {
            Some(g) => g.rows.push(id.clone()),
            None => gs.push(Group {
                kind,
                old_tag: r.old.tag.clone(),
                old_text: r.old.text.clone(),
                new_text: r.new.text.clone(),
                new_tag: r.new.tag.clone(),
                location: r.first_changed.clone(),
                rows: vec![id.clone()],
            }),
        }
    }
    gs
}

/// `rows.json` (contracts/schemas/rows.schema.json) for the differing rows of one or more
/// units, with `kind` carried as an extra field. Differences are numbered from
/// `first_difference` in `group` order, per unit.
pub fn rows_record(units: &[&UnitResult], first_difference: usize) -> Result<Value, String> {
    let mut rows = vec![];
    let mut d = first_difference.max(1);
    for u in units {
        let gs = group(u.differing());
        for r in u.differing() {
            let id = r.id.clone().unwrap();
            let k = gs.iter().position(|g| g.rows.contains(&id)).unwrap();
            let loc = r.first_changed.clone().ok_or_else(|| format!("unit {}: row {id} has no changed location", u.unit))?;
            rows.push(json!({
                "id": id, "difference": format!("d{}", d + k), "unit": u.unit,
                "condition": r.condition.join(" and "), "literals": r.literals,
                "old": r.old.to_json(), "new": r.new.to_json(), "property": r.property,
                "perIteration": r.per_iteration, "assumptions": r.assumptions,
                "firstChanged": loc.to_json(), "kind": r.kind}));
        }
        d += gs.len();
    }
    Ok(json!({"provenance": "pipeline", "rows": rows}))
}

/// Fill `scope.json`'s `assumptions[].rows` with the rows resting on each contract.
pub fn fill_scope(scope: &mut Value, units: &[&UnitResult]) {
    let Some(asm) = scope.get_mut("assumptions").and_then(Value::as_array_mut) else { return };
    // Library facts: one `library-fact` entry per fact a row rests on, merged by key.
    for u in units {
        for (f, rows) in library_facts(u) {
            match asm.iter_mut().find(|a| a["kind"] == "library-fact" && a["key"] == f.key) {
                Some(a) => {
                    let mut have: Vec<Value> = a["rows"].as_array().cloned().unwrap_or_default();
                    for r in rows {
                        if !have.contains(&json!(r)) {
                            have.push(json!(r));
                        }
                    }
                    a["rows"] = json!(have);
                }
                None => asm.push(json!({"kind": "library-fact", "key": f.key, "text": f.text, "rows": rows})),
            }
        }
    }
    for a in asm.iter_mut() {
        if a["kind"] == "library-fact" {
            continue;
        }
        let c = a["contract"].as_str().unwrap_or_default().to_string();
        let mut rows: Vec<String> =
            a["rows"].as_array().map(|v| v.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        for u in units {
            for r in u.differing() {
                if r.assumptions.contains(&c) && !rows.contains(r.id.as_ref().unwrap()) {
                    rows.push(r.id.clone().unwrap());
                }
            }
        }
        a["rows"] = json!(rows);
    }
}

/// Library facts used by the differing rows of a unit, each with the rows resting on it.
/// They are assumptions like the contracts, but `scope.json`'s `assumptions[].contract` is a
/// closed enum (common.schema.json `contract`), so they are reported here and in `detail.json`.
pub fn library_facts(r: &UnitResult) -> Vec<(&'static exec::Fact, Vec<String>)> {
    let mut out: Vec<(&'static exec::Fact, Vec<String>)> = vec![];
    for f in exec::FACTS {
        let rows: Vec<String> = r.differing().filter(|x| x.facts.contains(&f.key)).filter_map(|x| x.id.clone()).collect();
        if !rows.is_empty() {
            out.push((f, rows));
        }
    }
    out
}

/// One-version property `no-exception` (D18, D19): the feasible paths of `m` that throw,
/// as (literals, exception, contracts used). A `foreach` body is checked for one arbitrary
/// iteration, so an empty result holds for a loop of any length.
pub fn throwing_paths(m: &Model, disabled: &[String]) -> Result<Vec<(Vec<String>, String, Vec<String>)>, String> {
    let mut z = Z3::start()?;
    let leaves = Exec::new(&mut z, m, disabled, HashMap::new()).paths(m, &m.params)?;
    Ok(leaves
        .into_iter()
        .filter_map(|l| match l.end {
            Err(th) => Some((l.path.pc, th.exc, l.path.assumptions)),
            Ok(_) => None,
        })
        .collect())
}
