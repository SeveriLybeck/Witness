//! Mirror mode: when the solver rows come from `crates/differ` (declarations start with
//! `(declare-sort Obj 0)`), the models are emitted in the differ's own vocabulary
//! (reference values are one sort `Obj` with `w_null`; `w_len`, `w_at`, `w_is_<T>`,
//! opaque names as declared), so each solver literal translates to Lean syntactically and
//! no bridge is needed. Paths collect events: a report, an effect call (unit result), a
//! throw; the outcome tag follows the differ: throw, then reports, then value or effect,
//! else nothing. The differ's library facts become hypotheses of every lemma.

use crate::emit::{lean_str, Emitted, Model, SolverRows, Theorem};
use serde_json::Value;
use std::collections::BTreeMap;

pub const PRELUDE2: &str = "\
namespace W2
inductive R (α : Type) where | thr (e : String) | ok (a : α) (r c : Bool)
abbrev M (α : Type) := Bool → Bool → R α
def ret {α : Type} (a : α) : M α := fun r c => .ok a r c
def bind {α β : Type} (m : M α) (f : α → M β) : M β := fun r c =>
  match m r c with | .thr e => .thr e | .ok a r' c' => f a r' c'
def rep : M Unit := fun _ c => .ok () true c
def eff : M Unit := fun r _ => .ok () r true
def thr {α : Type} (e : String) : M α := fun _ _ => .thr e
def guard {α : Type} (p : Prop) [Decidable p] (e : String) (m : M α) : M α := fun r c => if p then .thr e else m r c
def cond {α : Type} (b : Bool) (t f : M α) : M α := fun r c => if b = true then t r c else f r c
def tag {α : Type} (m : M α) (v : Bool) : W.Tag := match m false false with
  | .thr _ => .throws | .ok _ r c => if r then .reports else if v || c then .value else .nothing
def tagF (m : M Bool) : W.Tag := match m false false with
  | .thr _ => .throws | .ok v r c => if r then .reports else if v || c then .value else .nothing
end W2
";

pub const SIMP2: &str = "U.old_t, U.new_t, W2.tag, W2.tagF, W2.bind, W2.ret, W2.rep, W2.eff, W2.thr, W2.guard, W2.cond";

pub fn is_mirror(rows: &SolverRows) -> bool {
    rows.declarations.iter().any(|d| d.trim() == "(declare-sort Obj 0)")
}

// ---------- s-expressions ----------

#[derive(Clone, Debug, PartialEq)]
pub enum Sx {
    A(String),
    L(Vec<Sx>),
}

pub fn parse_sx(s: &str) -> Result<Sx, String> {
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    let r = sx(&cs, &mut i)?;
    while i < cs.len() && cs[i].is_whitespace() {
        i += 1;
    }
    if i != cs.len() {
        return Err(format!("trailing text in `{s}`"));
    }
    Ok(r)
}

fn sx(cs: &[char], i: &mut usize) -> Result<Sx, String> {
    while *i < cs.len() && cs[*i].is_whitespace() {
        *i += 1;
    }
    if *i >= cs.len() {
        return Err("unexpected end".into());
    }
    match cs[*i] {
        '(' => {
            *i += 1;
            let mut v = vec![];
            loop {
                while *i < cs.len() && cs[*i].is_whitespace() {
                    *i += 1;
                }
                if *i >= cs.len() {
                    return Err("unclosed (".into());
                }
                if cs[*i] == ')' {
                    *i += 1;
                    return Ok(Sx::L(v));
                }
                v.push(sx(cs, i)?);
            }
        }
        ')' => Err("unexpected )".into()),
        '"' => {
            let mut t = String::from("\"");
            *i += 1;
            loop {
                if *i >= cs.len() {
                    return Err("unclosed string".into());
                }
                if cs[*i] == '"' {
                    if *i + 1 < cs.len() && cs[*i + 1] == '"' {
                        t.push_str("\"\"");
                        *i += 2;
                        continue;
                    }
                    *i += 1;
                    t.push('"');
                    return Ok(Sx::A(t));
                }
                t.push(cs[*i]);
                *i += 1;
            }
        }
        '|' => {
            let mut t = String::new();
            *i += 1;
            while *i < cs.len() && cs[*i] != '|' {
                t.push(cs[*i]);
                *i += 1;
            }
            *i += 1;
            Ok(Sx::A(t))
        }
        _ => {
            let mut t = String::new();
            while *i < cs.len() && !cs[*i].is_whitespace() && cs[*i] != '(' && cs[*i] != ')' {
                t.push(cs[*i]);
                *i += 1;
            }
            Ok(Sx::A(t))
        }
    }
}

/// Lean name of an SMT symbol.
pub fn lname(s: &str) -> String {
    crate::emit::ident(s)
}

fn lty(sort: &str) -> &'static str {
    match sort {
        "Bool" => "Bool",
        "Int" => "Int",
        "String" => "String",
        _ => "Obj",
    }
}

pub struct Decls {
    pub order: Vec<String>,
    pub sig: BTreeMap<String, (Vec<String>, String)>,
}

pub fn parse_decls(ds: &[String]) -> Result<Decls, String> {
    let mut d = Decls { order: vec![], sig: BTreeMap::new() };
    for s in ds {
        let x = parse_sx(s)?;
        let Sx::L(v) = x else { return Err(format!("bad declaration {s}")) };
        let atom = |i: usize| match v.get(i) {
            Some(Sx::A(a)) => Ok(a.clone()),
            _ => Err(format!("bad declaration {s}")),
        };
        match atom(0)?.as_str() {
            "declare-sort" => {}
            "declare-const" => {
                let n = atom(1)?;
                d.sig.insert(n.clone(), (vec![], atom(2)?));
                d.order.push(n);
            }
            "declare-fun" => {
                let n = atom(1)?;
                let args = match v.get(2) {
                    Some(Sx::L(a)) => a
                        .iter()
                        .map(|y| match y {
                            Sx::A(t) => Ok(t.clone()),
                            _ => Err(format!("bad sort in {s}")),
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => return Err(format!("bad declaration {s}")),
                };
                d.sig.insert(n.clone(), (args, atom(3)?));
                d.order.push(n);
            }
            o => return Err(format!("unknown declaration {o}")),
        }
    }
    Ok(d)
}

impl Decls {
    fn is_bool(&self, a: &Sx) -> bool {
        match a {
            Sx::A(n) => n == "true" || n == "false" || self.sig.get(n).is_some_and(|s| s.1 == "Bool"),
            Sx::L(v) => match v.first() {
                Some(Sx::A(h)) => {
                    matches!(h.as_str(), "not" | "and" | "or" | "=>" | "=" | "<=" | "<" | ">=" | ">" | "distinct")
                        || self.sig.get(h).is_some_and(|s| s.1 == "Bool")
                        || (h == "ite" && v.len() == 4 && self.is_bool(&v[2]))
                }
                _ => false,
            },
        }
    }

    pub fn prop(&self, x: &Sx) -> Result<String, String> {
        if let Sx::L(v) = x {
            if let Some(Sx::A(h)) = v.first() {
                let args = &v[1..];
                let ps = || args.iter().map(|a| self.prop(a)).collect::<Result<Vec<_>, _>>();
                match h.as_str() {
                    "not" if args.len() == 1 => return Ok(format!("(¬ {})", self.prop(&args[0])?)),
                    "and" => return Ok(format!("({})", ps()?.join(" ∧ "))),
                    "or" => return Ok(format!("({})", ps()?.join(" ∨ "))),
                    "=>" if args.len() == 2 => return Ok(format!("({} → {})", self.prop(&args[0])?, self.prop(&args[1])?)),
                    "=" if args.len() == 2 => return Ok(format!("({} = {})", self.term(&args[0])?, self.term(&args[1])?)),
                    "distinct" if args.len() == 2 => return Ok(format!("({} ≠ {})", self.term(&args[0])?, self.term(&args[1])?)),
                    o @ ("<=" | "<" | ">=" | ">") if args.len() == 2 => {
                        let s = match o { "<=" => "≤", "<" => "<", ">=" => "≥", _ => ">" };
                        return Ok(format!("({} {s} {})", self.term(&args[0])?, self.term(&args[1])?));
                    }
                    _ => {}
                }
            }
        }
        match x {
            Sx::A(a) if a == "true" => Ok("True".into()),
            Sx::A(a) if a == "false" => Ok("False".into()),
            _ if self.is_bool(x) => Ok(format!("({} = true)", self.term(x)?)),
            _ => Err(format!("not a proposition: {x:?}")),
        }
    }

    pub fn term(&self, x: &Sx) -> Result<String, String> {
        match x {
            Sx::A(a) => {
                if a == "true" || a == "false" {
                    Ok(a.clone())
                } else if a.starts_with('"') {
                    Ok(lean_str(&a[1..a.len() - 1].replace("\"\"", "\"")))
                } else if a.chars().all(|c| c.is_ascii_digit()) {
                    Ok(format!("({a} : Int)"))
                } else if self.sig.contains_key(a) {
                    Ok(lname(a))
                } else {
                    Err(format!("undeclared symbol {a}"))
                }
            }
            Sx::L(v) => {
                let Some(Sx::A(h)) = v.first() else { return Err("bad application".into()) };
                let args = &v[1..];
                let ts = || args.iter().map(|a| self.term(a)).collect::<Result<Vec<_>, _>>();
                match h.as_str() {
                    "not" => Ok(format!("(!{})", ts()?.join(""))),
                    "and" => Ok(format!("({})", ts()?.join(" && "))),
                    "or" => Ok(format!("({})", ts()?.join(" || "))),
                    "=" | "<=" | "<" | ">=" | ">" | "=>" | "distinct" => Ok(format!("(decide {})", self.prop(x)?)),
                    "ite" if args.len() == 3 => {
                        Ok(format!("(if {} then {} else {})", self.prop(&args[0])?, self.term(&args[1])?, self.term(&args[2])?))
                    }
                    "-" if args.len() == 1 => Ok(format!("(-{})", self.term(&args[0])?)),
                    o @ ("+" | "-" | "*") => Ok(format!("({})", ts()?.join(&format!(" {o} ")))),
                    f if self.sig.contains_key(f) => Ok(format!("({} {})", lname(f), ts()?.join(" "))),
                    f => Err(format!("undeclared function {f}")),
                }
            }
        }
    }
}

// ---------- models ----------

#[derive(Clone, Copy, Debug, PartialEq)]
enum So {
    Bool,
    Int,
    Str,
    Obj,
    Unit,
    Fin,
    Never,
}

fn so_of(s: &str) -> So {
    match s {
        "Bool" => So::Bool,
        "Int" => So::Int,
        "String" => So::Str,
        "Void" | "Unit" => So::Unit,
        _ => So::Obj,
    }
}

fn smt(s: So) -> &'static str {
    match s {
        So::Bool => "Bool",
        So::Int => "Int",
        So::Str => "String",
        _ => "Obj",
    }
}

/// The differ's identifier rule for `w_` helper names.
fn dident(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

#[derive(Clone, Debug)]
struct V {
    code: String,
    so: So,
    mon: bool,
}

fn pure(code: impl Into<String>, so: So) -> V {
    V { code: code.into(), so, mon: false }
}

impl V {
    fn m(&self) -> String {
        if self.mon { self.code.clone() } else { format!("(W2.ret {})", self.code) }
    }
}

struct Tr<'a> {
    opaque: &'a BTreeMap<String, crate::emit::Opaque>,
    params: &'a BTreeMap<String, String>,
    scope: Vec<(String, So)>,
    fresh: usize,
    element: Option<String>,
}

fn args_of(e: &Value) -> &[Value] {
    e.get("args").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}
fn op_of(e: &Value) -> &str {
    e.get("op").and_then(Value::as_str).unwrap_or("")
}

fn guard(cond: String, exc: &str, then: V) -> V {
    V { code: format!("(W2.guard ({cond}) {} {})", lean_str(exc), then.m()), so: then.so, mon: true }
}

fn exception_of(c: &str) -> &'static str {
    match c {
        "index-empty" => "IndexOutOfRangeException",
        "failed-cast" => "InvalidCastException",
        "nullable-value-null" => "InvalidOperationException",
        "dictionary-duplicate-add" => "ArgumentException",
        "null-receiver" => "NullReferenceException",
        _ => "Exception",
    }
}

impl<'a> Tr<'a> {
    fn var(&mut self) -> String {
        self.fresh += 1;
        format!("x{}", self.fresh)
    }

    /// Bind monadic values in order, then build from their pure codes.
    fn bind_all(&mut self, vs: Vec<V>, k: impl FnOnce(&mut Self, Vec<String>) -> Result<V, String>) -> Result<V, String> {
        let mut codes = vec![];
        let mut binds = vec![];
        for v in vs {
            if v.mon {
                let x = self.var();
                binds.push((v.code, x.clone()));
                codes.push(x);
            } else {
                codes.push(v.code);
            }
        }
        let inner = k(self, codes)?;
        if binds.is_empty() {
            return Ok(inner);
        }
        let mut code = inner.m();
        for (m, x) in binds.into_iter().rev() {
            code = format!("(W2.bind {m} (fun {x} => {code}))");
        }
        Ok(V { code, so: inner.so, mon: true })
    }

    fn args(&mut self, e: &Value) -> Result<Vec<V>, String> {
        args_of(e).iter().map(|a| self.ex(a)).collect()
    }

    fn to_fin(&self, v: V) -> V {
        match v.so {
            So::Fin | So::Never => V { so: So::Fin, ..v },
            So::Unit if v.mon => V { code: format!("(W2.bind {} (fun _ => W2.ret false))", v.code), so: So::Fin, mon: true },
            So::Unit => pure("false", So::Fin),
            _ if v.mon => V { code: format!("(W2.bind {} (fun _ => W2.ret true))", v.code), so: So::Fin, mon: true },
            _ => pure("true", So::Fin),
        }
    }

    fn ex(&mut self, e: &Value) -> Result<V, String> {
        let name = e.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        let a = args_of(e);
        match op_of(e) {
            "lit" => Ok(match e.get("value") {
                Some(Value::Bool(b)) => pure(b.to_string(), So::Bool),
                Some(Value::Number(n)) => pure(format!("({} : Int)", n.as_i64().ok_or("non-integer literal")?), So::Int),
                Some(Value::String(t)) => pure(lean_str(t), So::Str),
                _ => pure("w_null", So::Obj),
            }),
            "var" => {
                let so = self
                    .scope
                    .iter()
                    .rev()
                    .find(|(n, _)| *n == name)
                    .map(|x| x.1)
                    .or_else(|| self.params.get(&name).map(|s| so_of(s)))
                    .ok_or(format!("unbound variable {name}"))?;
                Ok(pure(lname(&name), so))
            }
            "none" => Ok(pure("w_null", So::Obj)),
            "nothing" => Ok(pure("()", So::Unit)),
            "throw" => Ok(V { code: format!("(W2.thr {})", lean_str(&name)), so: So::Never, mon: true }),
            "some" => self.ex(a.first().ok_or("some without argument")?),
            "isnull" => {
                let vs = self.args(e)?;
                let obj = vs.first().map(|v| v.so == So::Obj).unwrap_or(false);
                self.bind_all(vs, |_, c| Ok(if obj { pure(format!("(decide ({} = w_null))", c[0]), So::Bool) } else { pure("false", So::Bool) }))
            }
            "not" => {
                let vs = self.args(e)?;
                self.bind_all(vs, |_, c| Ok(pure(format!("(!{})", c[0]), So::Bool)))
            }
            o @ ("and" | "or") => {
                let vs = self.args(e)?;
                if vs.iter().all(|v| !v.mon) {
                    let j = if o == "and" { " && " } else { " || " };
                    return Ok(pure(format!("({})", vs.iter().map(|v| v.code.clone()).collect::<Vec<_>>().join(j)), So::Bool));
                }
                // short-circuit, as the differ does when a later operand may throw
                let mut it = vs.into_iter().rev();
                let mut acc = it.next().ok_or("empty and/or")?.m();
                for v in it {
                    let x = self.var();
                    acc = if o == "and" {
                        format!("(W2.bind {} (fun {x} => W2.cond {x} {acc} (W2.ret false)))", v.m())
                    } else {
                        format!("(W2.bind {} (fun {x} => W2.cond {x} (W2.ret true) {acc}))", v.m())
                    };
                }
                Ok(V { code: acc, so: So::Bool, mon: true })
            }
            "eq" => {
                let vs = self.args(e)?;
                self.bind_all(vs, |_, c| Ok(pure(format!("(decide ({} = {}))", c[0], c[1]), So::Bool)))
            }
            "index" => {
                let el = e.get("sort").and_then(Value::as_str).map(so_of).unwrap_or(So::Obj);
                let lit_i = a.get(1).filter(|x| op_of(x) == "lit").and_then(|x| x.get("value")).and_then(Value::as_i64);
                let vs = self.args(e)?;
                self.bind_all(vs, |_, c| {
                    let f = if el == So::Obj { "w_at".to_string() } else { format!("w_at_{}", smt(el)) };
                    let cond = match lit_i {
                        Some(n) if n >= 0 => format!("(w_len {} ≤ {})", c[0], c[1]),
                        _ => format!("({} < 0 ∨ w_len {} ≤ {})", c[1], c[0], c[1]),
                    };
                    Ok(guard(cond, exception_of("index-empty"), pure(format!("({f} {} {})", c[0], c[1]), el)))
                })
            }
            "cast" => {
                let contract = e.get("contract").and_then(Value::as_str).map(str::to_string);
                let vs = self.args(e)?;
                let so = vs.first().map(|v| v.so).ok_or("cast without argument")?;
                self.bind_all(vs, |_, c| {
                    let v = pure(c[0].clone(), so);
                    Ok(match contract {
                        Some(k) if so == So::Obj => {
                            let n = format!("w_is_{}", dident(&name));
                            guard(format!("({} ≠ w_null ∧ ¬ ({n} {} = true))", c[0], c[0]), exception_of(&k), v)
                        }
                        _ => v,
                    })
                })
            }
            "value" => {
                let target = e.get("sort").and_then(Value::as_str).map(so_of).unwrap_or(So::Obj);
                let contract = e.get("contract").and_then(Value::as_str).map(str::to_string);
                let vs = self.args(e)?;
                let so = vs.first().map(|v| v.so).ok_or("value without argument")?;
                self.bind_all(vs, |_, c| {
                    if so != So::Obj {
                        return Ok(pure(c[0].clone(), so));
                    }
                    let r = if target == So::Obj { pure(c[0].clone(), So::Obj) } else { pure(format!("(w_val_{} {})", smt(target), c[0]), target) };
                    Ok(match contract {
                        Some(k) => guard(format!("({} = w_null)", c[0]), exception_of(&k), r),
                        None => r,
                    })
                })
            }
            "call" => {
                let o = self.opaque.get(&name).cloned().ok_or(format!("call to undeclared opaque {name}"))?;
                let contract = e.get("contract").and_then(Value::as_str).map(str::to_string);
                let vs = self.args(e)?;
                let sorts: Vec<So> = vs.iter().map(|v| if v.so == So::Unit { So::Obj } else { v.so }).collect();
                let res = so_of(&o.result);
                self.bind_all(vs, |_, c| {
                    let a = |i: usize| c.get(i).cloned().unwrap_or_else(|| "w_null".into());
                    let wc = || format!("w_contains_{}_{}", smt(sorts[0]), smt(sorts[1]));
                    let body = if o.symbol.starts_with("T:") && c.len() == 1 && res == So::Bool {
                        let ty = dident(o.symbol.rsplit('.').next().unwrap_or(&o.symbol).trim_start_matches("T:"));
                        pure(format!("(w_is_{ty} {})", a(0)), So::Bool)
                    } else if o.symbol.contains(".ContainsKey(") && c.len() == 2 {
                        pure(format!("({} {} {})", wc(), a(0), a(1)), So::Bool)
                    } else if res == So::Unit {
                        V { code: "W2.eff".into(), so: So::Unit, mon: true }
                    } else if c.is_empty() {
                        pure(lname(&name), res)
                    } else {
                        pure(format!("({} {})", lname(&name), c.join(" ")), res)
                    };
                    Ok(match contract.as_deref() {
                        Some("null-receiver") => guard(format!("({} = w_null)", a(0)), exception_of("null-receiver"), body),
                        Some("dictionary-duplicate-add") if c.len() >= 2 => {
                            guard(format!("({} {} {} = true)", wc(), a(0), a(1)), exception_of("dictionary-duplicate-add"), body)
                        }
                        _ => body,
                    })
                })
            }
            "report" => {
                let vs = self.args(e)?;
                self.bind_all(vs, |_, _| Ok(V { code: "W2.rep".into(), so: So::Unit, mon: true }))
            }
            "if" => {
                let c = self.ex(a.first().ok_or("if without condition")?)?;
                let mut t = match a.get(1) { Some(x) => self.ex(x)?, None => pure("()", So::Unit) };
                let mut f = match a.get(2) { Some(x) => self.ex(x)?, None => pure("()", So::Unit) };
                if t.so == So::Never {
                    t.so = f.so;
                } else if f.so == So::Never {
                    f.so = t.so;
                }
                if t.so != f.so {
                    t = self.to_fin(t);
                    f = self.to_fin(f);
                }
                let so = t.so;
                if !c.mon && !t.mon && !f.mon {
                    return Ok(pure(format!("(if {} = true then {} else {})", c.code, t.code, f.code), so));
                }
                let x = self.var();
                Ok(V { code: format!("(W2.bind {} (fun {x} => W2.cond {x} {} {}))", c.m(), t.m(), f.m()), so, mon: true })
            }
            "let" => {
                let v = self.ex(a.first().ok_or("let without value")?)?;
                self.scope.push((name.clone(), v.so));
                let b = a.get(1).ok_or("let without body".to_string()).and_then(|x| self.ex(x));
                self.scope.pop();
                let b = b?;
                let n = lname(&name);
                if !v.mon && !b.mon {
                    return Ok(pure(format!("(let {n} := {}; {})", v.code, b.code), b.so));
                }
                Ok(V { code: format!("(W2.bind {} (fun {n} => {}))", v.m(), b.m()), so: b.so, mon: true })
            }
            "seq" => {
                let vs = self.args(e)?;
                let mut it = vs.into_iter().rev();
                let mut acc = match it.next() { Some(v) => v, None => return Ok(pure("()", So::Unit)) };
                for v in it {
                    if v.mon {
                        acc = V { code: format!("(W2.bind {} (fun _ => {}))", v.code, acc.m()), so: acc.so, mon: true };
                    }
                }
                Ok(acc)
            }
            "foreach" => {
                // D19: one arbitrary iteration. The element is a fresh value (a lemma
                // variable, as in the differ); params stand for any state before it.
                if self.element.is_some() {
                    return Err("nested `foreach`".into());
                }
                let coll = self.ex(a.first().ok_or("foreach without collection")?)?;
                let el = e.get("sort").and_then(Value::as_str).map(so_of).unwrap_or(So::Obj);
                self.element = Some(name.clone());
                self.scope.push((name.clone(), el));
                let body = a.get(1).ok_or("foreach without body".to_string()).and_then(|x| self.ex(x));
                self.scope.pop();
                let body = body?;
                let code = format!("(W2.bind {} (fun _ => W2.bind {} (fun _ => W2.ret ())))", coll.m(), body.m());
                Ok(V { code, so: So::Unit, mon: true })
            }
            o => Err(format!("unsupported op `{o}` in mirror mode")),
        }
    }
}

/// The differ's library facts (keys of `differ::exec::FACTS`) this mode reproduces:
/// as hypotheses (`opaque_facts`) or, for naming facts, in the call translation.
pub const COVERED_FACTS: &[&str] = &[
    "length-non-negative",
    "first-or-default",
    "located-null-false",
    "isinsource-null-false",
    "dsr-elements-non-null",
    "getsyntax-non-null",
    ".ContainsKey(",
    "T:",
];

/// Plain text of each fact, word for word as `differ::exec::FACTS` (pinned by a test);
/// `w_len` is the differ's inline `(>= (w_len a) 0)`.
pub const FACT_TEXT: &[(&str, &str)] = &[
    ("length-non-negative", "an array length is not negative"),
    ("first-or-default", "FirstOrDefault(a) is a[0] when a is not empty, else null"),
    ("located-null-false", "a null location is never in the compilation"),
    ("isinsource-null-false", "IsInSource is false for a null location (Location.None)"),
    ("dsr-elements-non-null", "elements of DeclaringSyntaxReferences are not null"),
    ("getsyntax-non-null", "GetSyntax of a non-null reference is not null"),
];

/// The differ's SMT statement of each fact (`differ::exec::FACTS[].smt`) that the Lean form in
/// `opaque_facts` renders, generalised over `{a}`, `{t}`, `{i}`. Pinned by a test: a changed
/// template means `opaque_facts` must be reviewed.
pub const FACT_SMT: &[(&str, &str)] = &[
    ("length-non-negative", "(>= (w_len {a}) 0)"),
    ("first-or-default", "(= {t} (ite (< 0 (w_len {a})) (w_at {a} 0) w_null))"),
    ("located-null-false", "(=> (= {a} w_null) (not {t}))"),
    ("isinsource-null-false", "(=> (= {a} w_null) (not {t}))"),
    ("dsr-elements-non-null", "(=> (and (<= 0 {i}) (< {i} (w_len {a}))) (not (= (w_at {a} {i}) w_null)))"),
    ("getsyntax-non-null", "(=> (not (= {a} w_null)) (not (= {t} w_null)))"),
];

fn text_of(k: &str) -> String {
    FACT_TEXT.iter().find(|(x, _)| *x == k).map(|x| x.1.to_string()).unwrap_or_default()
}

/// Library facts as (key, plain text, Lean form).
pub fn opaque_facts(ops: &BTreeMap<String, crate::emit::Opaque>, d: &Decls) -> Vec<(String, String, String)> {
    let mut f: Vec<(String, String, String)> = vec![];
    let mut add = |k: &str, l: String| f.push((k.to_string(), text_of(k), l));
    let has = |n: &str| d.sig.contains_key(n);
    if has("w_len") {
        add("length-non-negative", "∀ a, 0 ≤ w_len a".to_string());
    }
    for (n, o) in ops {
        if !has(n) {
            continue;
        }
        let ln = lname(n);
        let k = o.args.len();
        let xs: Vec<String> = (0..k).map(|i| format!("y{i}")).collect();
        let bind = if k == 0 { String::new() } else { format!("∀ {}, ", xs.join(" ")) };
        let app = if k == 0 { ln.clone() } else { format!("({ln} {})", xs.join(" ")) };
        let res = so_of(&o.result);
        if o.symbol.contains("FirstOrDefault") && k == 1 && has("w_at") {
            add("first-or-default", format!("∀ a, {ln} a = (if 0 < w_len a then w_at a 0 else w_null)"));
        }
        if o.symbol.contains("DeclaringSyntaxReferences") && has("w_at") && res == So::Obj {
            add("dsr-elements-non-null", format!("{bind}∀ i, 0 ≤ i → i < w_len {app} → w_at {app} i ≠ w_null"));
        }
        if o.symbol.contains("SyntaxReference.GetSyntax") && k >= 1 && res == So::Obj {
            add("getsyntax-non-null", format!("{bind}y0 ≠ w_null → {app} ≠ w_null"));
        }
        if o.symbol.contains("ReportIfLocatedInCompilation") && k >= 1 && res == So::Bool {
            add("located-null-false", format!("{bind}y{} = w_null → {app} = false", k - 1));
        }
        if o.symbol.contains("Location.IsInSource") && k == 1 && res == So::Bool {
            add("isinsource-null-false", format!("{bind}y0 = w_null → {app} = false"));
        }
    }
    drop(add);
    f
}

/// Emit a unit in mirror mode. Facts from `extra_facts` (the bridge's, if they parse as
/// Lean over these names) are not used: the differ's own facts are re-derived from symbols.
pub fn emit(old: &Model, new: &Model, rows: &SolverRows, drop: &std::collections::BTreeSet<u64>) -> Emitted {
    let mut out = Emitted {
        header: String::new(),
        theorems: vec![],
        unbound: BTreeMap::new(),
        error: None,
        has_foreach: false,
        simp_set: SIMP2.into(),
        coverage_note: None,
        facts: vec![],
        element: None,
        lift_note: None,
    };
    let fail = |out: &mut Emitted, e: String| {
        for r in &rows.rows {
            out.unbound.insert(r.n, e.clone());
        }
        out.error = Some(e);
    };
    let d = match parse_decls(&rows.declarations) {
        Ok(d) => d,
        Err(e) => {
            fail(&mut out, e);
            return out;
        }
    };
    let mut ops = BTreeMap::new();
    let mut params = BTreeMap::new();
    for m in [old, new] {
        for o in &m.opaque {
            ops.insert(o.name.clone(), o.clone());
        }
        for (p, s) in &m.params {
            params.insert(p.clone(), s.clone());
        }
    }
    let mut bodies = vec![];
    for m in [old, new] {
        let mut tr = Tr { opaque: &ops, params: &params, scope: vec![], fresh: 0, element: None };
        let res = tr.ex(&m.body);
        if let Some(e) = tr.element.take() {
            out.element = Some(e);
        }
        match res {
            Ok(v) => bodies.push(match v.so {
                So::Fin => format!("W2.tagF {}", v.m()),
                so => format!("W2.tag {} {}", v.m(), !matches!(so, So::Unit | So::Never)),
            }),
            Err(e) => {
                if m.body.to_string().contains("\"foreach\"") {
                    out.has_foreach = true;
                }
                fail(&mut out, format!("{} model: {e}", m.version));
                return out;
            }
        }
    }
    let mut bind = String::from("{Obj : Type} [DecidableEq Obj]");
    for n in &d.order {
        let (a, r) = &d.sig[n];
        let mut t: Vec<&str> = a.iter().map(|s| lty(s)).collect();
        t.push(lty(r));
        bind.push_str(&format!(" ({} : {})", lname(n), t.join(" → ")));
    }
    let app: String = d.order.iter().map(|n| lname(n)).collect::<Vec<_>>().join(" ");
    let facts = opaque_facts(&ops, &d);
    let hyps: String = facts.iter().enumerate().map(|(i, f)| format!(" (k{i} : {})", f.2)).collect();
    out.facts = facts.clone();

    let mut header = String::from(crate::emit::PRELUDE);
    header.push_str(PRELUDE2);
    if out.element.is_some() {
        header.push_str(crate::fold::FOLD_LEMMAS);
    }
    header.push_str(&format!("-- unit: {} (differ vocabulary)\nnamespace U\n", rows.unit));
    header.push_str(&format!("def old_t {bind} : W.Tag :=\n  {}\n", bodies[0]));
    header.push_str(&format!("def new_t {bind} : W.Tag :=\n  {}\n", bodies[1]));
    out.header = header;

    let tail = format!("(try simp_all [{SIMP2}, -Int.not_le, -Int.not_lt]) <;> (try (repeat' split)) <;> (try simp_all [-Int.not_le, -Int.not_lt]) <;> (try omega)");
    let mut atoms: Vec<String> = vec![];
    let mut disj = vec![];
    let mut paths: Vec<Vec<(usize, bool)>> = vec![];
    let mut covered: Vec<(u64, usize, String)> = vec![];
    for r in &rows.rows {
        let mut path = vec![];
        let mut hs = String::new();
        let mut haves = String::new();
        let mut cs = vec![];
        let mut bad = None;
        for (j, l) in r.literals.iter().enumerate() {
            match parse_sx(l).and_then(|x| d.prop(&x)) {
                Ok(p) => {
                    hs.push_str(&format!(" (h{j} : {p})"));
                    if let Some(f) = parse_sx(l).ok().and_then(|x| d.flip(&x)) {
                        haves.push_str(&format!("  have d{j} : {f} := by omega\n"));
                    }
                    cs.push(p);
                    let (at, pol) = crate::emit::atom(l);
                    if let Ok(ap) = parse_sx(&at).and_then(|x| d.prop(&x)) {
                        if !atoms.contains(&ap) {
                            atoms.push(ap.clone());
                        }
                        path.push((atoms.iter().position(|a| *a == ap).unwrap(), pol));
                    }
                }
                Err(e) => {
                    bad = Some(format!("literal `{l}`: {e}"));
                    break;
                }
            }
        }
        if let Some(b) = bad {
            out.unbound.insert(r.n, b.clone());
            out.coverage_note.get_or_insert(b);
            continue;
        }
        if !drop.contains(&r.n) {
            disj.push(if cs.is_empty() { "True".to_string() } else { format!("({})", cs.join(" ∧ ")) });
            paths.push(path);
            covered.push((r.n, r.literals.len(), r.new_tag.clone()));
        }
        out.theorems.push(Theorem {
            name: format!("row_{}", r.n),
            n: Some(r.n),
            statement: format!(
                "-- row {} {} assumptions: [{}]\ntheorem row_{} {bind}{hyps}{hs} :\n    U.old_t {app} = W.Tag.{} ∧ U.new_t {app} = W.Tag.{}",
                r.n,
                r.row.as_deref().unwrap_or("identical"),
                r.assumptions.join(", "),
                r.n,
                r.old_tag,
                r.new_tag
            ),
            proof: format!("by\n{haves}  {tail}"),
        });
    }
    if out.coverage_note.is_none() {
        let body = if disj.is_empty() { "False".to_string() } else { disj.join(" ∨\n    ") };
        let mut p = String::from("by\n");
        cover_tree(&atoms, &paths, &mut BTreeMap::new(), 1, &mut p);
        out.theorems.push(Theorem {
            name: "coverage".into(),
            n: None,
            statement: format!("-- coverage over the solver literals, differ facts as hypotheses\nset_option maxHeartbeats 4000000 in\ntheorem coverage {bind}{hyps} :\n    {body}"),
            proof: p,
        });
    }
    if let Some(el) = out.element.clone() {
        let thrower = covered.iter().find(|c| c.2 == "throws").map(|c| c.0);
        if out.coverage_note.is_some() || !drop.is_empty() || covered.len() + out.unbound.len() != rows.rows.len() {
            out.lift_note = Some("coverage incomplete".into());
        } else if let Some(n) = thrower {
            out.lift_note = Some(format!("new body throws on row {n}"));
        } else {
            let ks: String = (0..facts.len()).map(|i| format!(" k{i}")).collect();
            // one-version property per iteration: new never throws, from coverage and rows
            let pats: Vec<String> = covered
                .iter()
                .map(|(_, m, _)| match m {
                    0 => "_".to_string(),
                    1 => "h0".to_string(),
                    m => format!("⟨{}⟩", (0..*m).map(|j| format!("h{j}")).collect::<Vec<_>>().join(", ")),
                })
                .collect();
            let mut p = format!("by\n  rcases U.coverage {app}{ks} with {}\n", pats.join(" | "));
            for (n, m, _) in &covered {
                let hs: String = (0..*m).map(|j| format!(" h{j}")).collect();
                p.push_str(&format!("  · rw [(U.row_{n} {app}{ks}{hs}).2]; decide\n"));
            }
            out.theorems.push(Theorem {
                name: "body_nothrow".into(),
                n: None,
                statement: format!("-- per iteration: the new body never throws\ntheorem body_nothrow {bind}{hyps} :\n    U.new_t {app} ≠ W.Tag.throws"),
                proof: p.trim_end().to_string(),
            });
            // lifted to any loop length by the fold lemma (D19): params are the state
            let pnames: Vec<String> = params.keys().map(|p| lname(p)).collect();
            let eln = lname(&el);
            let elty = d.sig.get(&el).map(|s| lty(&s.1)).unwrap_or("Obj");
            let mut lb = String::from("{Obj : Type} [DecidableEq Obj]");
            let mut prs = String::new();
            let mut app2 = vec![];
            for n in &d.order {
                let ln = lname(n);
                let (a, r) = &d.sig[n];
                let mut t: Vec<&str> = a.iter().map(|s| lty(s)).collect();
                t.push(lty(r));
                if pnames.contains(&ln) {
                    prs.push_str(&format!(" (pr_{ln} : σ → {})", lty(r)));
                    app2.push(format!("(pr_{ln} s)"));
                } else if ln == eln {
                    app2.push("i".into());
                } else {
                    lb.push_str(&format!(" ({ln} : {})", t.join(" → ")));
                    app2.push(ln);
                }
            }
            let app2 = app2.join(" ");
            out.theorems.push(Theorem {
                name: "loop_nothrow".into(),
                n: None,
                statement: format!(
                    "-- any number of iterations, any state before each (fold lemma W.run_nothrow)\ntheorem loop_nothrow {lb} {{σ : Type}} (next : σ → {elty} → σ){prs}{hyps} :\n    ∀ (l : List {elty}) (s : σ), W.throws (W.run (fun (s : σ) (i : {elty}) =>\n      if U.new_t {app2} = W.Tag.throws then (W.Step.threw () s : W.Step Unit σ) else W.Step.ok (next s i)) l s) = false"
                ),
                proof: format!("\n  W.run_nothrow _ (fun s i => by simp only [if_neg (U.body_nothrow {app2}{ks}), W.throws])"),
            });
        }
    }
    out
}

const LOGIC: &str = "true_and, and_true, false_and, and_false, true_or, or_true, false_or, or_false, \
                     not_true_eq_false, not_false_eq_true, Bool.false_eq_true, Bool.true_eq_false, ne_eq";

/// Coverage proof that follows the rows' decision tree: split on the next undecided atom
/// of a consistent row; a leaf where some row is fully decided closes by the atom
/// hypotheses alone; a leaf no row fits (dependent literals) closes by the facts.
fn cover_tree(atoms: &[String], paths: &[Vec<(usize, bool)>], dec: &mut BTreeMap<usize, bool>, depth: usize, out: &mut String) {
    let ind = "  ".repeat(depth);
    let live: Vec<&Vec<(usize, bool)>> =
        paths.iter().filter(|p| p.iter().all(|(a, v)| dec.get(a).is_none_or(|d| d == v))).collect();
    let hyps: Vec<String> = dec.keys().map(|a| format!("c{a}")).collect();
    if live.iter().any(|p| p.iter().all(|(a, _)| dec.contains_key(a))) {
        let hs = if hyps.is_empty() { String::new() } else { format!("{}, ", hyps.join(", ")) };
        out.push_str(&format!("{ind}first | (simp only [{hs}{LOGIC}]; done) | simp_all\n"));
        return;
    }
    if live.is_empty() {
        out.push_str(&format!("{ind}exfalso\n{ind}(try simp_all) <;> (try omega)\n"));
        return;
    }
    let next = live[0].iter().find(|(a, _)| !dec.contains_key(a)).unwrap().0;
    out.push_str(&format!("{ind}by_cases c{next} : {}\n", atoms[next]));
    for v in [true, false] {
        dec.insert(next, v);
        out.push_str(&format!("{ind}· "));
        let mut sub = String::new();
        cover_tree(atoms, paths, dec, depth + 1, &mut sub);
        out.push_str(sub.trim_start());
        dec.remove(&next);
    }
}

impl Decls {
    /// The same comparison in the other direction, so `simp_all` finds it whichever way
    /// a guard or fact states it.
    pub fn flip(&self, x: &Sx) -> Option<String> {
        let cmp = |v: &[Sx], neg: bool| -> Option<String> {
            let (Sx::A(h), a, b) = (v.first()?, v.get(1)?, v.get(2)?) else { return None };
            let (a, b) = (self.term(a).ok()?, self.term(b).ok()?);
            match (h.as_str(), neg) {
                ("<=", false) => Some(format!("(¬ ({b} < {a}))")),
                ("<", false) => Some(format!("(¬ ({b} ≤ {a}))")),
                ("<=", true) => Some(format!("({b} < {a})")),
                ("<", true) => Some(format!("({b} ≤ {a})")),
                _ => None,
            }
        };
        match x {
            Sx::L(v) if v.len() == 2 && v[0] == Sx::A("not".into()) => match &v[1] {
                Sx::L(w) if w.len() == 3 => cmp(w, true),
                _ => None,
            },
            Sx::L(v) if v.len() == 3 => cmp(v, false),
            _ => None,
        }
    }
}
