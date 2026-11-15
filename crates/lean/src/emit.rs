//! Emits one Lean file per unit: prelude, both model versions as defs, one lemma per
//! solver row (`C_i -> old = A_i /\ new = B_i`, closed by `simp_all`) and one coverage
//! lemma over the row literals as Booleans (closed by `decide`). D17.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const PRELUDE: &str = "\
set_option linter.unusedSimpArgs false
set_option linter.unusedVariables false
namespace W
inductive Tag where | value | throws | reports | nothing
  deriving DecidableEq
inductive Out where | value | throws (e : String) | reports (d : String) | nothing
def Out.tag : Out → Tag
  | .value => .value | .throws _ => .throws | .reports _ => .reports | .nothing => .nothing
end W
";

#[derive(Clone, Debug)]
pub struct Opaque {
    pub name: String,
    pub symbol: String,
    pub args: Vec<String>,
    pub result: String,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub unit: String,
    pub version: String,
    pub params: Vec<(String, String)>,
    pub opaque: Vec<Opaque>,
    pub body: Value,
}

#[derive(Clone, Debug)]
pub struct SolverRow {
    pub n: u64,
    pub row: Option<String>,
    pub literals: Vec<String>,
    pub old_tag: String,
    pub new_tag: String,
    pub assumptions: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct SolverRows {
    pub unit: String,
    pub declarations: Vec<String>,
    pub rows: Vec<SolverRow>,
}

/// Atom bridge: solver literal -> Lean propositions over the model's names, plus facts.
/// Not part of the contract (see STATUS.md); not checked by Lean.
#[derive(Clone, Debug, Default)]
pub struct Bridge {
    pub literals: BTreeMap<String, Vec<String>>,
    pub facts: Vec<String>,
}

fn s(v: &Value, k: &str) -> Result<String, String> {
    v.get(k).and_then(Value::as_str).map(str::to_string).ok_or(format!("missing string `{k}`"))
}
fn strs(v: &Value, k: &str) -> Result<Vec<String>, String> {
    v.get(k)
        .and_then(Value::as_array)
        .ok_or(format!("missing array `{k}`"))?
        .iter()
        .map(|x| x.as_str().map(str::to_string).ok_or(format!("`{k}` holds a non-string")))
        .collect()
}
fn arr<'a>(v: &'a Value, k: &str) -> Result<&'a Vec<Value>, String> {
    v.get(k).and_then(Value::as_array).ok_or(format!("missing array `{k}`"))
}

pub fn parse_model(v: &Value) -> Result<Model, String> {
    let params = arr(v, "params")?
        .iter()
        .map(|p| Ok((s(p, "name")?, s(p, "sort")?)))
        .collect::<Result<_, String>>()?;
    let opaque = arr(v, "opaque")?
        .iter()
        .map(|o| {
            Ok(Opaque { name: s(o, "name")?, symbol: s(o, "symbol")?, args: strs(o, "args")?, result: s(o, "result")? })
        })
        .collect::<Result<_, String>>()?;
    Ok(Model {
        unit: s(v, "unit")?,
        version: s(v, "version")?,
        params,
        opaque,
        body: v.get("body").cloned().ok_or("missing `body`")?,
    })
}

pub fn parse_rows(v: &Value) -> Result<SolverRows, String> {
    let rows = arr(v, "rows")?
        .iter()
        .map(|r| {
            Ok(SolverRow {
                n: r.get("n").and_then(Value::as_u64).ok_or("missing `n`")?,
                row: r.get("row").and_then(Value::as_str).map(str::to_string),
                literals: strs(r, "literals")?,
                old_tag: s(r.get("old").ok_or("missing `old`")?, "tag")?,
                new_tag: s(r.get("new").ok_or("missing `new`")?, "tag")?,
                assumptions: strs(r, "assumptions").unwrap_or_default(),
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(SolverRows { unit: s(v, "unit")?, declarations: strs(v, "declarations").unwrap_or_default(), rows })
}

pub fn parse_bridge(v: &Value) -> Result<Bridge, String> {
    let mut b = Bridge::default();
    if let Some(m) = v.get("literals").and_then(Value::as_object) {
        for (k, x) in m {
            let props = x
                .as_array()
                .ok_or("bridge literal must map to an array")?
                .iter()
                .map(|p| p.as_str().map(str::to_string).ok_or("bridge prop must be a string".to_string()))
                .collect::<Result<_, _>>()?;
            b.literals.insert(k.clone(), props);
        }
    }
    if v.get("facts").is_some() {
        b.facts = strs(v, "facts")?;
    }
    Ok(b)
}

const KEYWORDS: &[&str] = &[
    "at", "fun", "let", "in", "if", "then", "else", "have", "show", "from", "do", "by", "match", "with", "end",
    "open", "section", "namespace", "theorem", "def", "where", "deriving", "variable", "instance", "structure",
    "inductive", "axiom", "example", "Type", "Prop", "Sort", "import", "true", "false", "none", "some", "W", "U",
];

pub fn ident(name: &str) -> String {
    let mut o: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if o.is_empty() || o.starts_with(|c: char| c.is_ascii_digit()) || KEYWORDS.contains(&o.as_str()) {
        o = format!("x_{o}");
    }
    o
}

pub fn lean_str(t: &str) -> String {
    format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
}

fn exception_of(contract: &str) -> &'static str {
    match contract {
        "index-empty" => "IndexOutOfRangeException",
        "failed-cast" => "InvalidCastException",
        "nullable-value-null" => "InvalidOperationException",
        "dictionary-duplicate-add" => "ArgumentException",
        "null-receiver" => "NullReferenceException",
        _ => "Exception",
    }
}

fn opt_inner(sort: &str) -> Option<&str> {
    sort.strip_prefix("Opt").filter(|r| r.starts_with(|c: char| c.is_ascii_uppercase()))
}

/// Shared translation context for both versions of one unit.
#[derive(Default)]
struct Ctx {
    sorts: BTreeSet<String>,
    dec_sorts: BTreeSet<String>,
    fns: BTreeMap<String, String>,
    unwraps: BTreeSet<String>,
    opaque: BTreeMap<String, Opaque>,
    alias: BTreeMap<String, String>,
    params: BTreeMap<String, String>,
    has_foreach: bool,
}

impl Ctx {
    fn ty(&mut self, sort: &str) -> String {
        match sort {
            "Bool" | "Int" | "String" | "Nat" => sort.to_string(),
            _ => {
                if let Some(x) = opt_inner(sort) {
                    format!("(Option {})", self.ty(x))
                } else {
                    let t = ident(sort);
                    self.sorts.insert(t.clone());
                    t
                }
            }
        }
    }
    fn fun(&mut self, name: &str, args: &[&str], res: &str) -> String {
        let mut t: Vec<String> = args.iter().map(|a| self.ty(a)).collect();
        t.push(self.ty(res));
        self.fns.insert(name.to_string(), t.join(" → "));
        name.to_string()
    }
    fn coerce(&mut self, code: String, actual: &str, expected: &str) -> Result<String, String> {
        if actual == expected {
            return Ok(code);
        }
        if opt_inner(actual) == Some(expected) {
            let f = format!("unwrap_{}", ident(expected));
            let oe = format!("Opt{expected}");
            self.fun(&f, &[&oe], expected);
            self.unwraps.insert(expected.to_string());
            return Ok(format!("({f} {code})"));
        }
        Err(format!("sort mismatch: {actual} where {expected} expected"))
    }
}

type Guards = Vec<(String, String)>;

struct Tr<'a> {
    cx: &'a mut Ctx,
    scope: Vec<(String, String)>,
}

fn args_of(e: &Value) -> &[Value] {
    e.get("args").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}
fn op_of(e: &Value) -> &str {
    e.get("op").and_then(Value::as_str).unwrap_or("")
}
fn arg(e: &Value, i: usize) -> Result<&Value, String> {
    args_of(e).get(i).ok_or(format!("`{}` needs argument {i}", op_of(e)))
}

impl<'a> Tr<'a> {
    fn term(&mut self, e: &Value, exp: Option<&str>, g: &mut Guards) -> Result<(String, String), String> {
        let name = e.get("name").and_then(Value::as_str).unwrap_or("");
        let sort = e.get("sort").and_then(Value::as_str);
        match op_of(e) {
            "lit" => match e.get("value") {
                Some(Value::Bool(b)) => Ok((b.to_string(), "Bool".into())),
                Some(Value::Number(n)) => {
                    let so = sort.unwrap_or("Int").to_string();
                    let t = self.cx.ty(&so);
                    Ok((format!("({n} : {t})"), so))
                }
                Some(Value::String(t)) => Ok((lean_str(t), "String".into())),
                _ => Err("unsupported literal".into()),
            },
            "var" => {
                let so = self
                    .scope
                    .iter()
                    .rev()
                    .find(|(n, _)| n == name)
                    .map(|(_, s)| s.clone())
                    .or_else(|| self.cx.params.get(name).cloned())
                    .or(sort.map(str::to_string))
                    .ok_or(format!("unknown variable {name}"))?;
                self.cx.ty(&so);
                Ok((ident(name), so))
            }
            "call" => {
                let op = self.cx.opaque.get(name).cloned().ok_or(format!("unknown call {name}"))?;
                let f = self.cx.alias.get(name).cloned().unwrap_or_else(|| ident(name));
                let a: Vec<&str> = op.args.iter().map(String::as_str).collect();
                self.cx.fun(&f, &a, &op.result);
                if args_of(e).len() != op.args.len() {
                    return Err(format!("call {name}: arity"));
                }
                let mut code = f;
                for (x, want) in args_of(e).iter().zip(op.args.iter()) {
                    let (c, so) = self.term(x, Some(want), g)?;
                    code = format!("{code} {}", self.cx.coerce(c, &so, want)?);
                }
                Ok((format!("({code})"), op.result.clone()))
            }
            "not" => {
                let (c, so) = self.term(arg(e, 0)?, Some("Bool"), g)?;
                Ok((format!("(!{})", self.cx.coerce(c, &so, "Bool")?), "Bool".into()))
            }
            o @ ("and" | "or") => {
                let mut parts = vec![];
                for (i, x) in args_of(e).iter().enumerate() {
                    let before = g.len();
                    let (c, so) = self.term(x, Some("Bool"), g)?;
                    if i > 0 && g.len() != before {
                        return Err("throwing operand after short-circuit".into());
                    }
                    parts.push(self.cx.coerce(c, &so, "Bool")?);
                }
                let j = if o == "and" { " && " } else { " || " };
                Ok((format!("({})", parts.join(j)), "Bool".into()))
            }
            "eq" => {
                let (a, sa) = self.term(arg(e, 0)?, None, g)?;
                let (b, sb) = self.term(arg(e, 1)?, Some(&sa), g)?;
                let b = self.cx.coerce(b, &sb, &sa)?;
                let t = self.cx.ty(&sa);
                self.cx.dec_sorts.insert(t);
                Ok((format!("(decide ({a} = {b}))"), "Bool".into()))
            }
            "isnull" => {
                let (c, so) = self.term(arg(e, 0)?, None, g)?;
                if opt_inner(&so).is_some() {
                    Ok((format!("({c}).isNone"), "Bool".into()))
                } else {
                    let f = format!("isnull_{}", ident(&so));
                    self.cx.fun(&f, &[&so], "Bool");
                    Ok((format!("({f} {c})"), "Bool".into()))
                }
            }
            "some" => {
                let inner = exp.and_then(opt_inner);
                let (c, so) = self.term(arg(e, 0)?, inner, g)?;
                Ok((format!("(some {c})"), format!("Opt{so}")))
            }
            "none" => {
                let so = sort.or(exp).filter(|x| opt_inner(x).is_some()).ok_or("`none` without an option sort")?;
                let t = self.cx.ty(so);
                Ok((format!("(none : {t})"), so.to_string()))
            }
            "index" => {
                let (a, sa) = self.term(arg(e, 0)?, None, g)?;
                let (i, si) = self.term(arg(e, 1)?, Some("Int"), g)?;
                let i = self.cx.coerce(i, &si, "Int")?;
                let elt = match exp {
                    Some(x) if opt_inner(x).is_none() => x.to_string(),
                    _ => sa.strip_suffix("Array").filter(|x| !x.is_empty()).ok_or("index element sort unknown")?.to_string(),
                };
                let at = format!("at_{}", ident(&sa));
                self.cx.fun(&at, &[&sa, "Int"], &elt);
                if let Some(k) = e.get("contract").and_then(Value::as_str) {
                    if k != "index-empty" {
                        return Err(format!("contract {k} on index"));
                    }
                    let em = format!("empty_{}", ident(&sa));
                    self.cx.fun(&em, &[&sa], "Bool");
                    g.push((format!("({em} {a})"), exception_of(k).into()));
                }
                Ok((format!("({at} {a} {i})"), elt))
            }
            "cast" => {
                let (c, so) = self.term(arg(e, 0)?, exp, g)?;
                if let Some(k) = e.get("contract").and_then(Value::as_str) {
                    let f = format!("is_{}", ident(name));
                    self.cx.fun(&f, &[&so], "Bool");
                    g.push((format!("(!({f} {c}))"), exception_of(k).into()));
                }
                Ok((c, so))
            }
            "if" => {
                let (c, sc) = self.term(arg(e, 0)?, Some("Bool"), g)?;
                let c = self.cx.coerce(c, &sc, "Bool")?;
                let mut inner = Guards::new();
                let (t, e2, so) = if op_of(arg(e, 1)?) == "none" && exp.is_none() {
                    let (b, sb) = self.term(arg(e, 2)?, None, &mut inner)?;
                    let (a, sa) = self.term(arg(e, 1)?, Some(&sb), &mut inner)?;
                    (self.cx.coerce(a, &sa, &sb)?, b, sb)
                } else {
                    let (a, sa) = self.term(arg(e, 1)?, exp, &mut inner)?;
                    let (b, sb) = self.term(arg(e, 2)?, Some(&sa), &mut inner)?;
                    let b = self.cx.coerce(b, &sb, &sa)?;
                    (a, b, sa)
                };
                if !inner.is_empty() {
                    return Err("throwing subterm inside a conditional value".into());
                }
                Ok((format!("(if {c} then {t} else {e2})"), so))
            }
            o => Err(format!("unsupported term `{o}`")),
        }
    }

    fn stmt(&mut self, e: &Value) -> Result<String, String> {
        let name = e.get("name").and_then(Value::as_str).unwrap_or("");
        let mut g = Guards::new();
        let core = match op_of(e) {
            "nothing" => "W.Out.nothing".to_string(),
            "throw" => format!("(W.Out.throws {})", lean_str(if name.is_empty() { "Exception" } else { name })),
            "report" => {
                for x in args_of(e) {
                    self.term(x, None, &mut g)?;
                }
                format!("(W.Out.reports {})", lean_str(name))
            }
            "value" => {
                for x in args_of(e) {
                    self.term(x, None, &mut g)?;
                }
                "W.Out.value".to_string()
            }
            "if" => {
                let (c, sc) = self.term(arg(e, 0)?, Some("Bool"), &mut g)?;
                let c = self.cx.coerce(c, &sc, "Bool")?;
                let t = self.stmt(arg(e, 1)?)?;
                let f = self.stmt(arg(e, 2)?)?;
                format!("(if {c} then {t} else {f})")
            }
            "let" => {
                let (v, so) = self.term(arg(e, 0)?, None, &mut g)?;
                let t = self.cx.ty(&so);
                self.scope.push((name.to_string(), so));
                let b = arg(e, 1).and_then(|x| self.stmt(x));
                self.scope.pop();
                format!("(let {} : {t} := {v}; {})", ident(name), b?)
            }
            "foreach" => {
                self.cx.has_foreach = true;
                return Err("`foreach` is compared per iteration by the fold lemmas, not by a row lemma".into());
            }
            o => return Err(format!("unsupported statement `{o}`")),
        };
        Ok(g.into_iter().rev().fold(core, |acc, (c, x)| format!("(if {c} then W.Out.throws {} else {acc})", lean_str(&x))))
    }
}

/// One theorem of an emitted file.
#[derive(Clone, Debug)]
pub struct Theorem {
    pub name: String,
    /// solver row number for row lemmas, None for the coverage lemma
    pub n: Option<u64>,
    pub statement: String,
    pub proof: String,
}

#[derive(Clone, Debug)]
pub struct Emitted {
    pub header: String,
    pub theorems: Vec<Theorem>,
    /// row number -> reason it has no lemma
    pub unbound: BTreeMap<u64, String>,
    /// error that stops the whole unit (L1)
    pub error: Option<String>,
    pub has_foreach: bool,
    pub simp_set: String,
    /// why coverage fell back to free Booleans (e.g. the missing bridge fact)
    pub coverage_note: Option<String>,
    /// library facts every lemma of the unit takes as hypotheses: (key, plain text, Lean form)
    pub facts: Vec<(String, String, String)>,
    /// per-iteration unit: loop element name, and why the loop lift was not emitted
    pub element: Option<String>,
    pub lift_note: Option<String>,
}

/// Literal atoms: `(not X)` -> (X, false), else (lit, true).
pub fn atom(lit: &str) -> (String, bool) {
    let t = lit.trim();
    if let Some(r) = t.strip_prefix("(not ").and_then(|r| r.strip_suffix(')')) {
        let (a, p) = atom(r);
        return (a, !p);
    }
    (t.to_string(), true)
}

fn bridge_props(b: &Bridge, lit: &str) -> Option<Vec<String>> {
    if let Some(p) = b.literals.get(lit.trim()) {
        return Some(p.clone());
    }
    let (a, pos) = atom(lit);
    let key = if pos { a.clone() } else { a };
    let alt = if pos { format!("(not {key})") } else { key.clone() };
    let p = b.literals.get(&alt)?;
    if p.len() != 1 {
        return None;
    }
    let x = &p[0];
    x.strip_suffix(" = true")
        .map(|h| format!("{h} = false"))
        .or_else(|| x.strip_suffix(" = false").map(|h| format!("{h} = true")))
        .map(|y| vec![y])
}

fn tag_claim(term: &str, tag: &str) -> Result<String, String> {
    match tag {
        "value" | "throws" | "reports" | "nothing" => Ok(format!("W.Out.tag ({term}) = W.Tag.{tag}")),
        t => Err(format!("unknown outcome tag {t}")),
    }
}

/// Bridged propositions for one side of an atom; a missing side of a one-proposition
/// atom is its negation.
fn side(b: &Bridge, a: &str, pos: bool) -> Option<Vec<String>> {
    let lit = if pos { a.to_string() } else { format!("(not {a})") };
    if let Some(p) = bridge_props(b, &lit) {
        return Some(p);
    }
    let other = bridge_props(b, &if pos { format!("(not {a})") } else { a.to_string() })?;
    (other.len() == 1).then(|| vec![format!("¬ ({})", other[0])])
}

fn conj(ps: &[String]) -> String {
    if ps.is_empty() { "True".into() } else { ps.iter().map(|p| format!("({p})")).collect::<Vec<_>>().join(" ∧ ") }
}

/// The fact the bridge must supply for an atom with several propositions per side.
pub fn dichotomy(pos: &[String], neg: &[String]) -> String {
    format!("({}) ∨ ({})", conj(pos), conj(neg))
}

/// Coverage over the bridged propositions, with the bridge facts as hypotheses: every
/// input of the model satisfies some row. Split on each atom's dichotomy, then `simp_all`.
fn model_coverage(
    atoms: &[String],
    rows: &SolverRows,
    drop: &BTreeSet<u64>,
    bridge: &Bridge,
    bind: &str,
    hyps: &str,
) -> Result<Theorem, String> {
    let mut proof = String::from("by\n");
    let mut split = vec![];
    for (i, a) in atoms.iter().enumerate() {
        let p = side(bridge, a, true).ok_or(format!("atom `{a}` has no model term"))?;
        let n = side(bridge, a, false).ok_or(format!("atom `{a}` has no model term"))?;
        let d = dichotomy(&p, &n);
        if p.len() == 1 && n.len() == 1 {
            proof.push_str(&format!("  have s{i} : {d} := by\n    by_cases hc : ({}) <;> simp_all\n", p[0]));
        } else {
            let want = d.replace(' ', "");
            let j = bridge
                .facts
                .iter()
                .position(|f| f.replace(' ', "") == want)
                .ok_or(format!("bridge fact missing: {d}"))?;
            proof.push_str(&format!("  have s{i} : {d} := k{j}\n"));
        }
        let pat = |k: usize, t: &str| {
            if k == 1 { format!("{t}") } else { format!("⟨{}⟩", (0..k).map(|x| format!("{t}_{x}")).collect::<Vec<_>>().join(", ")) }
        };
        split.push(format!("rcases s{i} with {} | {}", pat(p.len(), &format!("p{i}")), pat(n.len(), &format!("n{i}"))));
    }
    let mut disj = vec![];
    for r in rows.rows.iter().filter(|r| !drop.contains(&r.n)) {
        let mut ps = vec![];
        for l in &r.literals {
            let (a, pos) = atom(l);
            ps.extend(side(bridge, &a, pos).ok_or(format!("literal `{l}` has no model term"))?);
        }
        disj.push(format!("({})", conj(&ps)));
    }
    let body = if disj.is_empty() { "False".to_string() } else { disj.join(" ∨\n    ") };
    split.push("simp_all".into());
    proof.push_str(&format!("  {}", split.join(" <;> ")));
    Ok(Theorem {
        name: "coverage".into(),
        n: None,
        statement: format!("-- coverage over the bridged propositions\ntheorem coverage {bind}{hyps} :\n    {body}"),
        proof,
    })
}

pub const FIXED_PROOF: &str = "by\n  simp_all [U.old_f, U.new_f, W.Out.tag]";

/// Emit the unit. `drop_from_coverage` removes rows from the coverage lemma only (test hook).
pub fn emit(old: &Model, new: &Model, rows: &SolverRows, bridge: &Bridge, drop_from_coverage: &BTreeSet<u64>) -> Emitted {
    let mut out = Emitted {
        header: String::new(),
        theorems: vec![],
        unbound: BTreeMap::new(),
        error: None,
        has_foreach: false,
        simp_set: "U.old_f, U.new_f, W.Out.tag".into(),
        coverage_note: None,
        facts: vec![],
        element: None,
        lift_note: None,
    };
    // coverage lemma first: it needs no model
    let mut atoms: Vec<String> = vec![];
    for r in &rows.rows {
        for l in &r.literals {
            let (a, _) = atom(l);
            if !atoms.contains(&a) {
                atoms.push(a);
            }
        }
    }
    let disj: Vec<String> = rows
        .rows
        .iter()
        .filter(|r| !drop_from_coverage.contains(&r.n))
        .map(|r| {
            if r.literals.is_empty() {
                return "true".to_string();
            }
            let c: Vec<String> = r
                .literals
                .iter()
                .map(|l| {
                    let (a, p) = atom(l);
                    let i = atoms.iter().position(|x| *x == a).unwrap();
                    if p { format!("a{i}") } else { format!("!a{i}") }
                })
                .collect();
            format!("({})", c.join(" && "))
        })
        .collect();
    let body = if disj.is_empty() { "false".to_string() } else { disj.join(" || ") };
    let binders = if atoms.is_empty() {
        String::new()
    } else {
        format!("∀ ({} : Bool), ", (0..atoms.len()).map(|i| format!("a{i}")).collect::<Vec<_>>().join(" "))
    };
    let names: Vec<String> = atoms.iter().enumerate().map(|(i, a)| format!("-- a{i}: {a}")).collect();
    let mut coverage = Theorem {
        name: "coverage".into(),
        n: None,
        statement: format!("{}\ntheorem coverage : {binders}({body}) = true", names.join("\n")),
        proof: "by decide".into(),
    };

    let mut cx = Ctx::default();
    for m in [old, new] {
        for o in &m.opaque {
            if let Some(prev) = cx.opaque.get(&o.name) {
                if prev.args != o.args || prev.result != o.result {
                    out.error = Some(format!("opaque {} differs between versions", o.name));
                }
            }
            if let Some(t) = o.symbol.strip_prefix("T:") {
                let last = t.rsplit('.').next().unwrap_or(t);
                cx.alias.insert(o.name.clone(), format!("is_{}", ident(last)));
            }
            cx.opaque.insert(o.name.clone(), o.clone());
        }
        for (p, so) in &m.params {
            if let Some(prev) = cx.params.get(p) {
                if prev != so {
                    out.error = Some(format!("parameter {p} has two sorts"));
                }
            }
            cx.params.insert(p.clone(), so.clone());
        }
    }
    let mut bodies = vec![];
    for m in [old, new] {
        let mut tr = Tr { cx: &mut cx, scope: vec![] };
        match tr.stmt(&m.body) {
            Ok(b) => bodies.push(b),
            Err(e) => {
                out.error.get_or_insert(format!("{} model: {e}", m.version));
            }
        }
    }
    out.has_foreach = cx.has_foreach;
    let params: Vec<(String, String)> = cx.params.clone().into_iter().collect();
    let ptys: Vec<(String, String)> = params.iter().map(|(p, so)| (ident(p), cx.ty(so))).collect();
    let sorts: Vec<String> = cx.sorts.iter().cloned().collect();
    let mut bind = String::new();
    if !sorts.is_empty() {
        bind.push_str(&format!("{{{} : Type}}", sorts.join(" ")));
    }
    for d in &cx.dec_sorts {
        if cx.sorts.contains(d) {
            bind.push_str(&format!(" [DecidableEq {d}]"));
        }
    }
    for (f, t) in &cx.fns {
        bind.push_str(&format!(" ({f} : {t})"));
    }
    for (p, t) in &ptys {
        bind.push_str(&format!(" ({p} : {t})"));
    }
    let args: Vec<String> = cx.fns.keys().cloned().chain(ptys.iter().map(|(p, _)| p.clone())).collect();
    let app = args.join(" ");

    let mut header = String::from(PRELUDE);
    if cx.has_foreach {
        header.push_str(crate::fold::FOLD_LEMMAS);
    }
    header.push_str(&format!("-- unit: {}\nnamespace U\n", rows.unit));
    if out.error.is_none() {
        header.push_str(&format!("def old_f {bind} : W.Out :=\n  {}\n", bodies[0]));
        header.push_str(&format!("def new_f {bind} : W.Out :=\n  {}\n", bodies[1]));
    }
    out.header = header;

    if out.error.is_none() {
        let mut hyps = String::new();
        for u in &cx.unwraps {
            let f = format!("unwrap_{}", ident(u));
            hyps.push_str(&format!(" (hu_{f} : ∀ x, {f} (some x) = x)"));
        }
        for (i, f) in bridge.facts.iter().enumerate() {
            hyps.push_str(&format!(" (k{i} : {f})"));
        }
        for r in &rows.rows {
            let mut hs = String::new();
            let mut j = 0;
            let mut missing = None;
            for l in &r.literals {
                match bridge_props(bridge, l) {
                    Some(ps) => {
                        for p in ps {
                            hs.push_str(&format!(" (h{j} : {p})"));
                            j += 1;
                        }
                    }
                    None => {
                        missing = Some(l.clone());
                        break;
                    }
                }
            }
            if let Some(l) = missing {
                out.unbound.insert(r.n, format!("literal `{l}` has no model term"));
                continue;
            }
            let claim = tag_claim(&format!("U.old_f {app}"), &r.old_tag)
                .and_then(|a| tag_claim(&format!("U.new_f {app}"), &r.new_tag).map(|b| format!("{a} ∧ {b}")));
            match claim {
                Ok(c) => out.theorems.push(Theorem {
                    name: format!("row_{}", r.n),
                    n: Some(r.n),
                    statement: format!(
                        "-- row {} {} assumptions: [{}]\ntheorem row_{} {bind}{hyps}{hs} :\n    {c}",
                        r.n,
                        r.row.as_deref().unwrap_or("identical"),
                        r.assumptions.join(", "),
                        r.n
                    ),
                    proof: FIXED_PROOF.into(),
                }),
                Err(e) => {
                    out.unbound.insert(r.n, e);
                }
            }
        }
        match model_coverage(&atoms, rows, drop_from_coverage, bridge, &bind, &hyps) {
            Ok(t) => coverage = t,
            Err(e) => out.coverage_note = Some(e),
        }
    } else {
        for r in &rows.rows {
            out.unbound.insert(r.n, out.error.clone().unwrap());
        }
    }
    out.theorems.push(coverage);
    out
}

/// Line span (1-based, inclusive) of each theorem in a rendered file.
#[derive(Clone, Debug)]
pub struct Span {
    pub name: String,
    pub start: usize,
    pub end: usize,
}

/// Render header, extra declarations and theorems (each followed by `#print axioms`).
pub fn render(header: &str, extra: &[String], theorems: &[Theorem]) -> (String, Vec<Span>) {
    let mut text = String::from(header);
    for x in extra {
        text.push_str(x);
        text.push('\n');
    }
    let mut spans = vec![];
    for t in theorems {
        let start = text.lines().count() + 1;
        text.push_str(&format!("{} := {}\n#print axioms U.{}\n", t.statement, t.proof, t.name));
        spans.push(Span { name: t.name.clone(), start, end: text.lines().count() });
    }
    text.push_str("end U\n");
    (text, spans)
}
