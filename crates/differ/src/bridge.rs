//! `bridge.json` for `lean-certify --bridge`: each SMT literal of a row as Lean propositions
//! over the names the lean crate emits for the same models. A literal with no faithful
//! translation (e.g. a null of a sort Lean models as never null, or a disjunction) is left
//! out, so its rows stay `not-checked`.

use crate::model::Model;
use crate::UnitResult;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, PartialEq)]
enum Sx {
    A(String),
    L(Vec<Sx>),
}

fn parse(s: &str) -> Option<Sx> {
    let toks: Vec<String> = s.replace('(', " ( ").replace(')', " ) ").split_whitespace().map(str::to_string).collect();
    let mut i = 0;
    let r = go(&toks, &mut i)?;
    (i == toks.len()).then_some(r)
}

fn go(t: &[String], i: &mut usize) -> Option<Sx> {
    let x = t.get(*i)?.clone();
    *i += 1;
    if x == "(" {
        let mut v = vec![];
        while t.get(*i)? != ")" {
            v.push(go(t, i)?);
        }
        *i += 1;
        Some(Sx::L(v))
    } else if x == ")" {
        None
    } else {
        Some(Sx::A(x))
    }
}

fn ident(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect()
}

fn opt_inner(s: &str) -> Option<&str> {
    s.strip_prefix("Opt").filter(|r| r.starts_with(|c: char| c.is_ascii_uppercase()))
}

struct Env {
    params: HashMap<String, String>,
    /// name → (arg sorts, result sort, symbol)
    fns: HashMap<String, (Vec<String>, String, String)>,
    /// opaque name standing for FirstOrDefault, with its result sort
    fod: Option<(String, String)>,
}

impl Env {
    fn new(old: &Model, new: &Model) -> Env {
        let mut e = Env { params: HashMap::new(), fns: HashMap::new(), fod: None };
        for m in [old, new] {
            for (n, s) in &m.params {
                e.params.insert(n.clone(), s.clone());
            }
            for o in &m.opaque {
                e.fns.insert(o.name.clone(), (o.args.clone(), o.result.clone(), o.symbol.clone()));
                if o.symbol.contains("FirstOrDefault") {
                    e.fod = Some((o.name.clone(), o.result.clone()));
                }
            }
        }
        e
    }

    fn coerce(&self, t: String, actual: &str, expected: Option<&str>) -> Option<(String, String)> {
        let Some(x) = expected else { return Some((t, actual.into())) };
        if actual == x {
            return Some((t, actual.into()));
        }
        if opt_inner(actual) == Some(x) {
            return Some((format!("(unwrap_{} {t})", ident(x)), x.into()));
        }
        if opt_inner(x) == Some(actual) {
            return Some((format!("(some {t})"), x.into()));
        }
        None
    }

    /// Lean term and its model sort.
    fn term(&self, s: &Sx, exp: Option<&str>) -> Option<(String, String)> {
        match s {
            Sx::A(a) if a == "w_null" => exp.filter(|x| opt_inner(x).is_some()).map(|x| ("none".into(), x.into())),
            Sx::A(a) if a.parse::<i64>().is_ok() => Some((format!("({a} : Int)"), "Int".into())),
            Sx::A(a) if a == "true" || a == "false" => Some((a.clone(), "Bool".into())),
            Sx::A(a) => {
                let s = self.params.get(a)?;
                self.coerce(ident(a), s, exp)
            }
            Sx::L(v) => {
                let Sx::A(f) = v.first()? else { return None };
                let args = &v[1..];
                if f == "w_at" && args.len() == 2 {
                    let (a, sa) = self.term(&args[0], None)?;
                    let (i, _) = self.term(&args[1], Some("Int"))?;
                    let elt = match exp {
                        Some(x) if opt_inner(x).is_none() => x.to_string(),
                        _ => sa.strip_suffix("Array").filter(|x| !x.is_empty())?.to_string(),
                    };
                    return self.coerce(format!("(at_{} {a} {i})", ident(&sa)), &elt, exp);
                }
                if let Some(ty) = f.strip_prefix("w_is_") {
                    let (a, _) = self.term(&args[0], None)?;
                    let a = match a.strip_prefix("(some ") {
                        Some(_) => return None,
                        None => a,
                    };
                    return self.coerce(format!("(is_{ty} {a})"), "Bool", exp);
                }
                let (sorts, res, _) = self.fns.get(f)?;
                if sorts.len() != args.len() {
                    return None;
                }
                let mut parts = vec![ident(f)];
                for (a, s) in args.iter().zip(sorts) {
                    parts.push(self.term(a, Some(s))?.0);
                }
                let t = if parts.len() == 1 { parts.remove(0) } else { format!("({})", parts.join(" ")) };
                self.coerce(t, res, exp)
            }
        }
    }

    fn is_null(s: &Sx) -> bool {
        *s == Sx::A("w_null".into())
    }

    /// A term the differ's facts make non-null, of a sort Lean models without null.
    fn non_null(&self, s: &Sx) -> bool {
        let Sx::L(v) = s else { return false };
        match v.first() {
            Some(Sx::A(f)) if f == "w_at" => true,
            Some(Sx::A(f)) => self.fns.get(f).is_some_and(|(_, r, sym)| opt_inner(r).is_none() && sym.contains("SyntaxReference.GetSyntax")),
            _ => false,
        }
    }

    fn len_empty(&self, a: &Sx, empty: bool) -> Option<Vec<String>> {
        let (t, sa) = self.term(a, None)?;
        let mut v = vec![format!("empty_{} {t} = {empty}", ident(&sa))];
        if let Some((f, _)) = &self.fod {
            if self.fns.get(f).is_some_and(|(args, _, _)| args.first() == Some(&sa)) {
                v.push(if empty {
                    format!("{} {t} = none", ident(f))
                } else {
                    format!("{} {t} = some (at_{} {t} (0 : Int))", ident(f), ident(&sa))
                });
            }
        }
        Some(v)
    }

    /// Propositions for literal `s` (`pos`) or its negation.
    fn lit(&self, s: &Sx, pos: bool) -> Option<Vec<String>> {
        if let Sx::L(v) = s {
            if let Some(Sx::A(h)) = v.first() {
                match (h.as_str(), &v[1..]) {
                    ("not", [x]) => return self.lit(x, !pos),
                    ("and", xs) if pos => {
                        let mut out = vec![];
                        for x in xs {
                            out.extend(self.lit(x, true)?);
                        }
                        return Some(out);
                    }
                    ("and", xs) => {
                        // ¬(A ∧ B) with every conjunct but one implied: the negation of that one.
                        let open: Vec<&Sx> = xs.iter().filter(|x| self.lit(x, true) != Some(vec![])).collect();
                        return if open.len() == 1 { self.lit(open[0], false) } else { None };
                    }
                    ("or", xs) if !pos => {
                        let mut out = vec![];
                        for x in xs {
                            out.extend(self.lit(x, false)?);
                        }
                        return Some(out);
                    }
                    ("<=", [Sx::L(l), Sx::A(z)]) if z == "0" && l.len() == 2 && l[0] == Sx::A("w_len".into()) => {
                        return self.len_empty(&l[1], pos);
                    }
                    ("=", [a, b]) if Self::is_null(b) || Self::is_null(a) => {
                        let x = if Self::is_null(b) { a } else { b };
                        if let Some((t, s)) = self.term(x, None) {
                            if opt_inner(&s).is_some() {
                                return Some(vec![format!("{t} {} none", if pos { "=" } else { "≠" })]);
                            }
                        }
                        return if !pos && self.non_null(x) { Some(vec![]) } else { None };
                    }
                    ("=", [a, b]) => {
                        let (ta, sa) = self.term(a, None)?;
                        let (tb, _) = self.term(b, Some(&sa))?;
                        return Some(vec![format!("{ta} {} {tb}", if pos { "=" } else { "≠" })]);
                    }
                    _ => {}
                }
            }
        }
        let (t, s) = self.term(s, Some("Bool"))?;
        (s == "Bool").then(|| vec![format!("{t} = {pos}")])
    }

    fn facts(&self) -> Vec<String> {
        let mut v = vec![];
        for (n, (args, res, sym)) in &self.fns {
            if res == "Bool" && args.len() == 2 && opt_inner(&args[1]).is_some() && sym.contains("ReportIfLocatedInCompilation") {
                v.push(format!("∀ c, {} c none = false", ident(n)));
            }
            if res == "Bool" && args.len() == 1 && opt_inner(&args[0]).is_some() && sym.contains("Location.IsInSource") {
                v.push(format!("{} none = false", ident(n)));
            }
        }
        v.sort();
        v
    }
}

/// The bridge for one unit's rows.
pub fn bridge_json(old: &Model, new: &Model, r: &UnitResult) -> Value {
    let env = Env::new(old, new);
    let mut lits: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut facts = env.facts();
    for row in &r.rows {
        for l in &row.literals {
            if lits.contains_key(l) {
                continue;
            }
            let Some(s) = parse(l) else { continue };
            for f in env.fod_facts(&s) {
                if !facts.contains(&f) {
                    facts.push(f);
                }
            }
            if let Some(ps) = env.lit(&s, true) {
                lits.insert(l.clone(), ps);
            }
        }
    }
    json!({"literals": lits, "facts": facts})
}

impl Env {
    /// The FirstOrDefault library fact (exec.rs: `fod a = ite(len a > 0, a[0], null)`, with
    /// non-null elements) for every array term `a` it is instantiated on in this literal.
    fn fod_facts(&self, s: &Sx) -> Vec<String> {
        let mut out = vec![];
        let Sx::L(v) = s else { return out };
        if let (Some(Sx::A(f)), [a]) = (v.first(), &v[1..]) {
            let hit = f == "w_len" || self.fod.as_ref().is_some_and(|(n, _)| n == f);
            // Elements are non-null only for DeclaringSyntaxReferences (exec::elements_non_null).
            let dsr = matches!(a, Sx::L(w) if matches!(w.first(), Some(Sx::A(g))
                if self.fns.get(g).is_some_and(|(_, _, sym)| sym.contains("DeclaringSyntaxReferences"))));
            if let (true, true, Some((fod, _))) = (hit, dsr, &self.fod) {
                if let (Some(e), Some(n)) = (self.len_empty(a, true), self.len_empty(a, false)) {
                    if e.len() == 2 && n.len() == 2 && self.fns.get(fod).is_some() {
                        out.push(format!("(({}) ∧ ({})) ∨ (({}) ∧ ({}))", e[0], e[1], n[0], n[1]));
                    }
                }
            }
        }
        for x in &v[1..] {
            out.extend(self.fod_facts(x));
        }
        out
    }
}
