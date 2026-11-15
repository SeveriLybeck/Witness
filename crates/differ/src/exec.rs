//! Symbolic executor: enumerates the feasible paths of one model version as SMT-LIB text.
//!
//! Values are SMT terms. Every reference type is one uninterpreted sort `Obj` with a
//! distinguished `w_null`; Bool, Int and String keep their theory sorts. Paths fork at `if`,
//! at lazy `and`/`or` whose later operands may throw, and at each enabled contract.

use crate::model::{Expr, Loc, Model, Opaque};
use crate::smt::{q, Sat, Z3};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum S {
    Bool,
    Int,
    Str,
    Obj,
    Unit,
}

pub fn sort_of(name: &str) -> S {
    match name {
        "Bool" => S::Bool,
        "Int" => S::Int,
        "String" => S::Str,
        "Void" | "Unit" => S::Unit,
        _ => S::Obj,
    }
}

pub fn smt_sort(s: S) -> &'static str {
    match s {
        S::Bool => "Bool",
        S::Int => "Int",
        S::Str => "String",
        S::Obj | S::Unit => "Obj",
    }
}

#[derive(Clone, Debug)]
pub struct T {
    pub s: String,
    pub sort: S,
}

fn t(s: impl Into<String>, sort: S) -> T {
    T { s: s.into(), sort }
}

fn unit() -> T {
    t("unit", S::Unit)
}

#[derive(Clone, Debug)]
pub struct Event {
    pub report: bool,
    pub name: String,
    /// Name as shown in outcome texts.
    pub shown: String,
    pub args: Vec<T>,
}

#[derive(Clone, Debug, Default)]
pub struct Path {
    pub pc: Vec<String>,
    pub facts: Vec<String>,
    pub events: Vec<Event>,
    /// Contracts whose throwing case this path takes.
    pub assumptions: Vec<String>,
    pub locs: Vec<Loc>,
    pub fork_locs: Vec<Loc>,
    pub per_iteration: bool,
    pub null_receiver: bool,
    /// Keys of the library facts (`FACTS`) asserted on this path.
    pub fact_keys: Vec<&'static str>,
    /// One entry per `pc` literal: the same condition in the source's terms.
    pub readable: Vec<String>,
    /// Values a region hands on when it ends (model `carried`), compared between versions.
    pub carry: Vec<T>,
    /// Source lines of the branch tests taken, in order (model `line` on `if`).
    pub lines: Vec<u64>,
}

#[derive(Clone, Debug)]
pub struct Thr {
    pub exc: String,
    pub loc: Option<Loc>,
    /// The effect call that threw (a contract on a void call such as `Dictionary.Add`).
    pub call: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Leaf {
    pub path: Path,
    pub end: Result<T, Thr>,
}

impl Path {
    pub fn asserts(&self) -> Vec<String> {
        let mut v = self.facts.clone();
        v.extend(self.pc.iter().cloned());
        v
    }
    fn fact(&mut self, key: &'static str, f: String) {
        if !self.facts.contains(&f) {
            self.facts.push(f);
        }
        if !self.fact_keys.contains(&key) {
            self.fact_keys.push(key);
        }
    }
}

pub fn exception_of(contract: &str) -> &'static str {
    match contract {
        "index-empty" => "IndexOutOfRangeException",
        "failed-cast" => "InvalidCastException",
        "nullable-value-null" => "InvalidOperationException",
        "dictionary-duplicate-add" => "ArgumentException",
        "null-receiver" => "NullReferenceException",
        _ => "Exception",
    }
}

/// A library fact: a quantifier-free statement asserted to z3 when a call to `symbol` (a
/// substring of the resolved symbol) is built; both versions share it. `key` is stable.
#[derive(Clone, Copy, Debug)]
pub struct Fact {
    pub key: &'static str,
    pub symbol: &'static str,
    pub text: &'static str,
    /// The SMT-LIB statement asserted, over `{t}` (the call term), `{a}` (its receiver or array; for
    /// located-null-false the location argument) and `{i}` (an index).
    pub smt: &'static str,
}

pub const FACTS: &[Fact] = &[
    Fact { key: "length-non-negative", symbol: "ImmutableArray", text: "an array length is not negative", smt: "(>= (w_len {a}) 0)" },
    Fact { key: "first-or-default", symbol: "FirstOrDefault", text: "FirstOrDefault(a) is a[0] when a is not empty, else null", smt: "(= {t} (ite (< 0 (w_len {a})) (w_at {a} 0) w_null))" },
    Fact { key: "located-null-false", symbol: "ReportIfLocatedInCompilation", text: "a null location is never in the compilation", smt: "(=> (= {a} w_null) (not {t}))" },
    Fact { key: "isinsource-null-false", symbol: "Location.IsInSource", text: "IsInSource is false for a null location (Location.None)", smt: "(=> (= {a} w_null) (not {t}))" },
    Fact { key: "dsr-elements-non-null", symbol: "DeclaringSyntaxReferences", text: "elements of DeclaringSyntaxReferences are not null", smt: "(=> (and (<= 0 {i}) (< {i} (w_len {a}))) (not (= (w_at {a} {i}) w_null)))" },
    Fact { key: "getsyntax-non-null", symbol: "SyntaxReference.GetSyntax", text: "GetSyntax of a non-null reference is not null", smt: "(=> (not (= {a} w_null)) (not (= {t} w_null)))" },
];

/// Mappings, not assertions: how two opaque kinds are read (no z3 fact of their own).
pub const READINGS: &[(&str, &str)] = &[
    (".ContainsKey(", "ContainsKey(d, k) is the predicate behind dictionary-duplicate-add"),
    ("T:", "an opaque Bool over one value with a type symbol is the type test used by casts"),
];

pub fn fact(key: &str) -> Option<&'static Fact> {
    FACTS.iter().find(|f| f.key == key)
}

type Env = HashMap<String, T>;
type Out = Vec<(Path, Result<T, Thr>)>;

pub struct Exec<'a> {
    pub z: &'a mut Z3,
    opaque: HashMap<String, Opaque>,
    enabled: HashSet<String>,
    pub rename: HashMap<String, String>,
}

pub fn declare_base(z: &mut Z3) -> Result<(), String> {
    z.declare("Obj", "(declare-sort Obj 0)".into())?;
    z.declare("w_null", "(declare-const w_null Obj)".into())?;
    z.declare("w_len", "(declare-fun w_len (Obj) Int)".into())?;
    Ok(())
}

fn ident(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

fn may_throw(e: &Expr, enabled: &HashSet<String>) -> bool {
    let own = match e.op.as_str() {
        "throw" => true,
        "index" | "cast" | "value" | "call" => e.contract.as_ref().is_some_and(|c| enabled.contains(c)),
        _ => false,
    };
    own || e.args.iter().any(|a| may_throw(a, enabled))
}

impl<'a> Exec<'a> {
    pub fn new(z: &'a mut Z3, m: &Model, disabled: &[String], rename: HashMap<String, String>) -> Exec<'a> {
        let enabled = m.contracts.iter().map(|(c, _)| c.clone()).filter(|c| !disabled.contains(c)).collect();
        let opaque = m.opaque.iter().map(|o| (o.name.clone(), o.clone())).collect();
        Exec { z, opaque, enabled, rename }
    }

    /// Elements of `DeclaringSyntaxReferences` are never null (Roslyn).
    fn elements_non_null(&mut self, p: &mut Path, arr: &str, i: &str) {
        let from_dsr = self.opaque.values().any(|o| {
            o.symbol.contains("DeclaringSyntaxReferences") && (arr.starts_with(&format!("({} ", q(&o.name))) || arr == q(&o.name))
        });
        if from_dsr {
            let _ = self.z.declare("w_at", "(declare-fun w_at (Obj Int) Obj)".into());
            p.fact("dsr-elements-non-null", format!("(=> (and (<= 0 {i}) (< {i} (w_len {arr}))) (not (= (w_at {arr} {i}) w_null)))"));
        }
    }

    pub fn param(&mut self, name: &str, sort: &str) -> Result<T, String> {
        let n = q(self.rename.get(name).map(String::as_str).unwrap_or(name));
        let s = sort_of(sort);
        self.z.declare(&n, format!("(declare-const {n} {})", smt_sort(s)))?;
        Ok(t(n, s))
    }

    pub fn paths(&mut self, m: &Model, params: &[(String, String)]) -> Result<Vec<Leaf>, String> {
        declare_base(self.z)?;
        let mut env = Env::new();
        for (n, s) in params {
            let v = self.param(n, s)?;
            env.insert(n.clone(), v);
        }
        let out = self.eval(&m.body, &env, Path::default(), &None)?;
        Ok(out.into_iter().map(|(path, end)| Leaf { path, end }).collect())
    }

    fn feasible(&mut self, p: &Path) -> Result<bool, String> {
        Ok(self.z.check(&p.asserts(), &[])?.0 != Sat::Unsat)
    }

    fn reader(&self) -> crate::read::Reader<'_> {
        crate::read::Reader { opaque: &self.opaque }
    }

    /// `label`: the condition in the source's terms, (when true, when false); else printed from the literal.
    fn branch(&mut self, p: Path, cond: &str, loc: &Option<Loc>, label: Option<(String, String)>) -> Result<Vec<(Path, bool)>, String> {
        match cond {
            "true" => return Ok(vec![(p, true)]),
            "false" => return Ok(vec![(p, false)]),
            _ => {}
        }
        let mut v = vec![];
        for b in [true, false] {
            let mut np = p.clone();
            let lit = if b { cond.to_string() } else { format!("(not {cond})") };
            np.readable.push(match &label {
                Some((t, f)) => if b { t.clone() } else { f.clone() },
                None => self.reader().literal(&lit),
            });
            np.pc.push(lit);
            if let Some(l) = loc {
                np.fork_locs.push(l.clone());
            }
            if self.feasible(&np)? {
                v.push((np, b));
            }
        }
        Ok(v)
    }

    /// Fork on a contract's throwing condition; returns the non-throwing paths and pushes throws to `out`.
    fn contract(
        &mut self,
        p: Path,
        contract: &Option<String>,
        cond: impl FnOnce() -> String,
        loc: &Option<Loc>,
        out: &mut Out,
        syntax: &Option<String>,
        call: Option<String>,
    ) -> Result<Vec<Path>, String> {
        let c = match contract {
            Some(c) if self.enabled.contains(c) => c.clone(),
            _ => return Ok(vec![p]),
        };
        let label = syntax.as_ref().map(|s| match c.as_str() {
            "index-empty" => (format!("{s} is out of range"), format!("{s} is in range")),
            "failed-cast" => (format!("the cast {s} fails"), format!("the cast {s} succeeds")),
            "nullable-value-null" => (format!("{s} has no value"), format!("{s} has a value")),
            "dictionary-duplicate-add" => (format!("the key is already present at {s}"), format!("the key is not yet present at {s}")),
            _ => (format!("the receiver is null at {s}"), format!("the receiver is not null at {s}")),
        });
        let mut ok = vec![];
        for (mut np, thrown) in self.branch(p, &cond(), loc, label)? {
            if thrown {
                np.assumptions.push(c.clone());
                if c == "null-receiver" {
                    np.null_receiver = true;
                }
                out.push((np, Err(Thr { exc: exception_of(&c).into(), loc: loc.clone(), call: call.clone() })));
            } else {
                ok.push(np);
            }
        }
        Ok(ok)
    }

    fn eval_list(&mut self, args: &[Expr], env: &Env, p: Path, loc: &Option<Loc>) -> Result<Vec<(Path, Result<Vec<T>, Thr>)>, String> {
        let mut cur: Vec<(Path, Result<Vec<T>, Thr>)> = vec![(p, Ok(vec![]))];
        for a in args {
            let mut next = vec![];
            for (p, r) in cur {
                match r {
                    Err(th) => next.push((p, Err(th))),
                    Ok(vs) => {
                        for (p2, r2) in self.eval(a, env, p, loc)? {
                            match r2 {
                                Ok(v) => {
                                    let mut vs2 = vs.clone();
                                    vs2.push(v);
                                    next.push((p2, Ok(vs2)));
                                }
                                Err(th) => next.push((p2, Err(th))),
                            }
                        }
                    }
                }
            }
            cur = next;
        }
        Ok(cur)
    }

    /// Evaluate `args`, then map each successful value list through `f`.
    fn with_args<F>(&mut self, e: &Expr, env: &Env, p: Path, loc: &Option<Loc>, mut f: F) -> Result<Out, String>
    where
        F: FnMut(&mut Self, Path, Vec<T>, &mut Out) -> Result<(), String>,
    {
        let mut out = vec![];
        for (p, r) in self.eval_list(&e.args, env, p, loc)? {
            match r {
                Err(th) => out.push((p, Err(th))),
                Ok(vs) => f(self, p, vs, &mut out)?,
            }
        }
        Ok(out)
    }

    fn lit(&self, e: &Expr) -> Result<T, String> {
        let v = e.value.clone().unwrap_or(serde_json::Value::Null);
        Ok(match v {
            serde_json::Value::Bool(b) => t(b.to_string(), S::Bool),
            serde_json::Value::Number(n) => {
                let i = n.as_i64().ok_or("non-integer literal")?;
                t(if i < 0 { format!("(- {})", -i) } else { i.to_string() }, S::Int)
            }
            serde_json::Value::String(s) => t(format!("\"{}\"", s.replace('"', "\"\"")), S::Str),
            serde_json::Value::Null => t("w_null", S::Obj),
            _ => return Err("unsupported literal".into()),
        })
    }

    fn lazy(&mut self, is_and: bool, args: &[Expr], env: &Env, p: Path, loc: &Option<Loc>) -> Result<Out, String> {
        if args.is_empty() {
            return Ok(vec![(p, Ok(t(is_and.to_string(), S::Bool)))]);
        }
        let mut out = vec![];
        for (p, r) in self.eval(&args[0], env, p, loc)? {
            let c = match r {
                Ok(c) => c,
                Err(th) => {
                    out.push((p, Err(th)));
                    continue;
                }
            };
            for (np, b) in self.branch(p, &c.s, loc, None)? {
                if b == is_and {
                    out.extend(self.lazy(is_and, &args[1..], env, np, loc)?);
                } else {
                    out.push((np, Ok(t((!is_and).to_string(), S::Bool))));
                }
            }
        }
        Ok(out)
    }

    fn call(&mut self, e: &Expr, mut p: Path, ts: Vec<T>, loc: &Option<Loc>, out: &mut Out) -> Result<(), String> {
        let name = e.name.clone().ok_or("call without name")?;
        let o = self.opaque.get(&name).cloned().ok_or_else(|| format!("call to undeclared opaque {name}"))?;
        let a = |i: usize| ts.get(i).map(|x| x.s.clone()).unwrap_or_else(|| "w_null".into());
        let sorts: Vec<S> = ts.iter().map(|x| if x.sort == S::Unit { S::Obj } else { x.sort }).collect();
        let res = sort_of(&o.result);
        let contains_name = |sorts: &[S]| format!("w_contains_{}_{}", smt_sort(sorts[0]), smt_sort(sorts[1]));
        // Contracts on the call happen before its effect.
        let conds: Box<dyn FnOnce() -> String> = match e.contract.as_deref() {
            Some("null-receiver") => Box::new(|| format!("(= {} w_null)", a(0))),
            Some("dictionary-duplicate-add") if ts.len() >= 2 => {
                let n = contains_name(&sorts);
                self.z.declare(&n, format!("(declare-fun {n} ({} {}) Bool)", smt_sort(sorts[0]), smt_sort(sorts[1])))?;
                let (x, y) = (a(0), a(1));
                Box::new(move || format!("({n} {x} {y})"))
            }
            _ => Box::new(|| "false".into()),
        };
        let oks = self.contract(p.clone(), &e.contract, conds, loc, out, &e.syntax, (res == S::Unit).then(|| shown(&o, &ts)))?;
        p = match oks.into_iter().next() {
            Some(np) => np,
            None => return Ok(()),
        };
        let term = if o.symbol.starts_with("T:") && ts.len() == 1 && res == S::Bool {
            let ty = ident(o.symbol.rsplit('.').next().unwrap_or(&o.symbol).trim_start_matches("T:"));
            let n = format!("w_is_{ty}");
            self.z.declare(&n, format!("(declare-fun {n} (Obj) Bool)"))?;
            t(format!("({n} {})", a(0)), S::Bool)
        } else if o.symbol.contains(".ContainsKey(") && ts.len() == 2 {
            let n = contains_name(&sorts);
            self.z.declare(&n, format!("(declare-fun {n} ({} {}) Bool)", smt_sort(sorts[0]), smt_sort(sorts[1])))?;
            t(format!("({n} {} {})", a(0), a(1)), S::Bool)
        } else if res == S::Unit {
            p.events.push(Event { report: false, name: display(&o), shown: shown(&o, &ts), args: ts.clone() });
            unit()
        } else {
            let n = q(&name);
            let arg_sorts: Vec<&str> = o.args.iter().map(|s| smt_sort(sort_of(s))).collect();
            self.z.declare(&n, format!("(declare-fun {n} ({}) {})", arg_sorts.join(" "), smt_sort(res)))?;
            if ts.is_empty() {
                t(n, res)
            } else {
                t(format!("({n} {})", ts.iter().map(|x| x.s.as_str()).collect::<Vec<_>>().join(" ")), res)
            }
        };
        if o.symbol.contains("SyntaxReference.GetSyntax") && !ts.is_empty() {
            p.fact("getsyntax-non-null", format!("(=> (not (= {} w_null)) (not (= {} w_null)))", a(0), term.s));
        }
        if o.symbol.contains("FirstOrDefault") && ts.len() == 1 {
            self.elements_non_null(&mut p, &a(0), "0");
            self.z.declare("w_at", "(declare-fun w_at (Obj Int) Obj)".into())?;
            p.fact("length-non-negative", format!("(>= (w_len {}) 0)", a(0)));
            p.fact("first-or-default", format!("(= {} (ite (< 0 (w_len {})) (w_at {} 0) w_null))", term.s, a(0), a(0)));
        }
        if o.symbol.contains("ReportIfLocatedInCompilation") && res == S::Bool && !ts.is_empty() {
            p.fact("located-null-false", format!("(=> (= {} w_null) (not {}))", a(ts.len() - 1), term.s));
        }
        if o.symbol.contains("Location.IsInSource") && res == S::Bool && ts.len() == 1 {
            p.fact("isinsource-null-false", format!("(=> (= {} w_null) (not {}))", a(0), term.s));
        }
        out.push((p, Ok(term)));
        Ok(())
    }

    pub fn eval(&mut self, e: &Expr, env: &Env, mut p: Path, loc: &Option<Loc>) -> Result<Out, String> {
        let loc = &e.location.clone().or_else(|| loc.clone());
        if let Some(l) = &e.location {
            p.locs.push(l.clone());
        }
        let name = || e.name.clone().unwrap_or_default();
        Ok(match e.op.as_str() {
            "lit" => vec![(p, Ok(self.lit(e)?))],
            "var" => {
                let v = env.get(&name()).cloned().ok_or_else(|| format!("unbound variable {}", name()))?;
                vec![(p, Ok(v))]
            }
            "none" => vec![(p, Ok(t("w_null", S::Obj)))],
            "nothing" if e.carried.is_empty() => vec![(p, Ok(unit()))],
            "nothing" => {
                let mut out = vec![];
                for (mut p, r) in self.eval_list(&e.carried, env, p, loc)? {
                    match r {
                        Ok(vs) => {
                            p.carry = vs;
                            out.push((p, Ok(unit())));
                        }
                        Err(th) => out.push((p, Err(th))),
                    }
                }
                out
            }
            "throw" => vec![(p, Err(Thr { exc: name(), loc: loc.clone(), call: None }))],
            "let" => {
                if e.args.len() != 2 {
                    return Err("let needs 2 args".into());
                }
                let mut out = vec![];
                for (p, r) in self.eval(&e.args[0], env, p, loc)? {
                    match r {
                        Ok(v) => {
                            let mut env2 = env.clone();
                            env2.insert(name(), v);
                            out.extend(self.eval(&e.args[1], &env2, p, loc)?);
                        }
                        Err(th) => out.push((p, Err(th))),
                    }
                }
                out
            }
            "if" => {
                let label = e.syntax.as_ref().map(|s| if e.null_test {
                    (format!("{s} is null"), format!("{s} is not null"))
                } else {
                    (s.clone(), crate::read::negate_source(s))
                });
                let mut out = vec![];
                for (p, r) in self.eval(e.args.first().ok_or("if without condition")?, env, p, loc)? {
                    match r {
                        Ok(c) => {
                            let cl = e.args[0].location.clone().or_else(|| loc.clone());
                            for (mut np, b) in self.branch(p, &c.s, &cl, label.clone())? {
                                np.lines.extend(e.line);
                                match e.args.get(if b { 1 } else { 2 }) {
                                    Some(arm) => out.extend(self.eval(arm, env, np, loc)?),
                                    None => out.push((np, Ok(unit()))),
                                }
                            }
                        }
                        Err(th) => out.push((p, Err(th))),
                    }
                }
                out
            }
            "and" | "or" if e.args.iter().skip(1).any(|a| may_throw(a, &self.enabled)) => {
                self.lazy(e.op == "and", &e.args, env, p, loc)?
            }
            "and" | "or" => {
                let op = e.op.clone();
                self.with_args(e, env, p, loc, |_, p, vs, out| {
                    let s = if vs.len() == 1 {
                        vs[0].s.clone()
                    } else {
                        format!("({op} {})", vs.iter().map(|v| v.s.as_str()).collect::<Vec<_>>().join(" "))
                    };
                    out.push((p, Ok(t(s, S::Bool))));
                    Ok(())
                })?
            }
            "not" => self.with_args(e, env, p, loc, |_, p, vs, out| {
                out.push((p, Ok(t(format!("(not {})", vs[0].s), S::Bool))));
                Ok(())
            })?,
            "eq" => self.with_args(e, env, p, loc, |_, p, vs, out| {
                out.push((p, Ok(t(format!("(= {} {})", vs[0].s, vs[1].s), S::Bool))));
                Ok(())
            })?,
            "isnull" => self.with_args(e, env, p, loc, |_, p, vs, out| {
                let s = if vs[0].sort == S::Obj { format!("(= {} w_null)", vs[0].s) } else { "false".into() };
                out.push((p, Ok(t(s, S::Bool))));
                Ok(())
            })?,
            "some" => self.with_args(e, env, p, loc, |_, p, mut vs, out| {
                out.push((p, Ok(vs.remove(0))));
                Ok(())
            })?,
            "index" => {
                let el = e.sort.as_deref().map(sort_of).unwrap_or(S::Obj);
                let c = e.contract.clone().or(Some("index-empty".into()));
                self.with_args(e, env, p, loc, |me, mut p, vs, out| {
                    let (arr, i) = (vs[0].s.clone(), vs[1].s.clone());
                    let f = if el == S::Obj { "w_at".to_string() } else { format!("w_at_{}", smt_sort(el)) };
                    me.z.declare(&f, format!("(declare-fun {f} (Obj Int) {})", smt_sort(el)))?;
                    p.fact("length-non-negative", format!("(>= (w_len {arr}) 0)"));
                    me.elements_non_null(&mut p, &arr, &i);
                    let cond = || match i.parse::<i64>() {
                        Ok(n) if n >= 0 => format!("(<= (w_len {arr}) {i})"),
                        _ => format!("(or (< {i} 0) (<= (w_len {arr}) {i}))"),
                    };
                    for np in me.contract(p, &c, cond, loc, out, &e.syntax, None)? {
                        out.push((np, Ok(t(format!("({f} {arr} {i})"), el))));
                    }
                    Ok(())
                })?
            }
            "cast" => {
                let ty = ident(&name());
                self.with_args(e, env, p, loc, |me, p, vs, out| {
                    let a = vs[0].clone();
                    let n = format!("w_is_{ty}");
                    me.z.declare(&n, format!("(declare-fun {n} (Obj) Bool)"))?;
                    let cond = || format!("(and (not (= {} w_null)) (not ({n} {})))", a.s, a.s);
                    let c = if a.sort == S::Obj { e.contract.clone() } else { None };
                    for np in me.contract(p, &c, cond, loc, out, &e.syntax, None)? {
                        out.push((np, Ok(a.clone())));
                    }
                    Ok(())
                })?
            }
            "value" => {
                let target = e.sort.as_deref().map(sort_of).unwrap_or(S::Obj);
                self.with_args(e, env, p, loc, |me, p, vs, out| {
                    let a = vs[0].clone();
                    if a.sort != S::Obj {
                        out.push((p, Ok(a)));
                        return Ok(());
                    }
                    let cond = || format!("(= {} w_null)", a.s);
                    let c = e.contract.clone();
                    let res = if target == S::Obj {
                        a.clone()
                    } else {
                        let f = format!("w_val_{}", smt_sort(target));
                        me.z.declare(&f, format!("(declare-fun {f} (Obj) {})", smt_sort(target)))?;
                        t(format!("({f} {})", a.s), target)
                    };
                    for np in me.contract(p, &c, cond, loc, out, &e.syntax, None)? {
                        out.push((np, Ok(res.clone())));
                    }
                    Ok(())
                })?
            }
            "call" => self.with_args(e, env, p, loc, |me, p, vs, out| me.call(e, p, vs, loc, out))?,
            "report" => {
                let n = name();
                self.with_args(e, env, p, loc, |_, mut p, vs, out| {
                    p.events.push(Event { report: true, name: n.clone(), shown: n.clone(), args: vs });
                    out.push((p, Ok(unit())));
                    Ok(())
                })?
            }
            "seq" => {
                let mut cur: Out = vec![(p, Ok(unit()))];
                for a in &e.args {
                    let mut next = vec![];
                    for (p, r) in cur {
                        match r {
                            Ok(_) => next.extend(self.eval(a, env, p, loc)?),
                            Err(th) => next.push((p, Err(th))),
                        }
                    }
                    cur = next;
                }
                cur
            }
            "foreach" => {
                // One arbitrary iteration (D19): the element is a fresh unconstrained value.
                let el_sort = e.sort.as_deref().map(sort_of).unwrap_or(S::Obj);
                let el = q(&name());
                self.z.declare(&el, format!("(declare-const {el} {})", smt_sort(el_sort)))?;
                let mut out = vec![];
                let coll = e.args.first().ok_or("foreach without collection")?;
                let body = e.args.get(1).ok_or("foreach without body")?;
                for (mut p, r) in self.eval(coll, env, p, loc)? {
                    match r {
                        Ok(_) => {
                            p.per_iteration = true;
                            let mut env2 = env.clone();
                            env2.insert(name(), t(el.clone(), el_sort));
                            for (p2, r2) in self.eval(body, &env2, p, loc)? {
                                out.push((p2, r2.map(|_| unit())));
                            }
                        }
                        Err(th) => out.push((p, Err(th))),
                    }
                }
                out
            }
            other => return Err(format!("unsupported op {other}")),
        })
    }
}

/// Readable name of an opaque call for outcome texts: `Type.Method` from a documentation-ID
/// symbol (`M:System.Collections.Generic.List`1.Add(`0)` → `List.Add`), else the symbol.
/// Never the model's generated identifier.
pub fn display(o: &Opaque) -> String {
    let sym = o.symbol.as_str();
    match sym {
        "region:skip" => return SKIP.into(),
        "region:leave" => return LEAVE.into(),
        _ => {}
    }
    let body = match sym.split_once(':') {
        Some((k, rest)) if k.len() == 1 => rest,
        _ if !sym.is_empty() => return sym.to_string(),
        _ => return o.name.rsplit_once('_').map(|x| x.0).unwrap_or(&o.name).to_string(),
    };
    let head = body.split('(').next().unwrap_or(body);
    let parts: Vec<&str> = head.split('.').map(|p| p.split('`').next().unwrap_or(p)).filter(|p| !p.is_empty()).collect();
    parts[parts.len().saturating_sub(2)..].join(".")
}

/// Effects of a region's early ends (translator: `region:skip`, `region:leave`), as outcome text.
pub const SKIP: &str = "skips the rest of the iteration";
pub const LEAVE: &str = "leaves the loop";

/// An effect call as shown in an outcome: `intMembers.Add` when the receiver is a plain input, else `Dictionary.Add`.
pub fn shown(o: &Opaque, args: &[T]) -> String {
    let d = display(o);
    let instance = o.symbol.starts_with("M:") && !o.symbol.contains(".#ctor");
    match (args.first(), d.rsplit_once('.')) {
        (Some(r), Some((_, m))) if instance && r.sort == S::Obj && r.s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && r.s != "w_null" => format!("{}.{m}", r.s),
        _ => d,
    }
}

/// Shape of an observation: events (kind, name, arity) and terminator.
fn shape(l: &Leaf) -> (Vec<(bool, String, String)>, String) {
    let ev = l.path.events.iter().map(|e| (e.report, e.name.clone(), e.args.iter().map(|a| smt_sort(a.sort)).collect::<Vec<_>>().join(","))).collect();
    let end = match &l.end {
        Err(th) => format!("throw:{}", th.exc),
        Ok(v) if v.sort == S::Unit => "unit".into(),
        Ok(v) => format!("value:{:?}", v.sort),
    };
    (ev, end)
}

/// SMT formula: the two observations differ.
pub fn differs(a: &Leaf, b: &Leaf) -> String {
    if shape(a) != shape(b) {
        return "true".into();
    }
    let mut ds = vec![];
    for (x, y) in a.path.events.iter().zip(&b.path.events) {
        for (p, q) in x.args.iter().zip(&y.args) {
            if p.s != q.s && p.sort != S::Unit {
                ds.push(format!("(not (= {} {}))", p.s, q.s));
            }
        }
    }
    if let (Ok(p), Ok(q)) = (&a.end, &b.end) {
        if p.sort != S::Unit && p.s != q.s {
            ds.push(format!("(not (= {} {}))", p.s, q.s));
        }
        // what a region hands on to the code after it
        if a.path.carry.len() != b.path.carry.len() || a.path.carry.iter().zip(&b.path.carry).any(|(x, y)| x.sort != y.sort) {
            return "true".into();
        }
        for (x, y) in a.path.carry.iter().zip(&b.path.carry) {
            if x.s != y.s {
                ds.push(format!("(not (= {} {}))", x.s, y.s));
            }
        }
    }
    match ds.len() {
        0 => "false".into(),
        1 => ds.remove(0),
        _ => format!("(or {})", ds.join(" ")),
    }
}
