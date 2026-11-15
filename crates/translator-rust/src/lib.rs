//! Rust front end (D44). Default path: Charon's LLBC lowered to the model
//! (`llbc`); Aeneas's Lean via `tools`. `lower` below is the FALLBACK used
//! only when Charon is not installed: the source is parsed with `syn` and
//! lowered directly. Fallback subset: `bool`/integer parameters, slices and
//! `Vec`s, `if`/`else`, `return`, `panic!`, `!`, `&&`, `||`, `==`, `!=`,
//! integer and boolean literals, `let`, slice indexing (contract
//! `index-empty`). Anything else is an `unmodelled` opaque call and its line
//! is listed as not modelled.

pub mod llbc;
pub mod tools;

use serde_json::{json, Value};
use std::path::Path;
use syn::spanned::Spanned;

/// Which front end produced a model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Producer {
    Charon,
    SynFallback,
}

impl Producer {
    pub fn label(self) -> &'static str {
        match self {
            Producer::Charon => "charon",
            Producer::SynFallback => "syn-fallback",
        }
    }
}

/// Lower `function` of the Rust file `src`: through Charon when installed, else the `syn` fallback.
/// `work` receives Charon's LLBC.
pub fn lower_file(src: &Path, o: &Options, work: &Path) -> Result<(Value, Producer), String> {
    if tools::charon().is_some() {
        std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
        let krate = format!("{}_{}", o.function, o.version);
        let out = work.join(format!("{krate}.json.llbc"));
        tools::charon_llbc(src, &krate, &out, false)?;
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&out).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        return Ok((llbc::lower(&v, &o.function, &o.file, &o.version)?, Producer::Charon));
    }
    let text = std::fs::read_to_string(src).map_err(|e| e.to_string())?;
    Ok((lower(&text, o)?, Producer::SynFallback))
}
use syn::{BinOp, Expr, Item, Lit, Pat, Stmt, Type, UnOp};

pub struct Options {
    pub function: String,
    pub file: String,
    pub version: String,
}

struct Lower<'a> {
    o: &'a Options,
    modelled: Vec<u64>,
    not_modelled: Vec<Value>,
    unmodelled: bool,
    sorts: Vec<(String, String)>,
}

fn line(s: &impl Spanned) -> u64 {
    s.span().start().line as u64
}

fn sort_of(t: &Type) -> &'static str {
    match t {
        Type::Path(p) => match p.path.segments.last().map(|s| s.ident.to_string()).as_deref() {
            Some("bool") => "Bool",
            Some("i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize") => "Int",
            Some("String") => "String",
            _ => "Obj",
        },
        Type::Reference(r) => sort_of(&r.elem),
        Type::Tuple(t) if t.elems.is_empty() => "Unit",
        _ => "Obj",
    }
}

fn elem_sort(t: &Type) -> &'static str {
    match t {
        Type::Reference(r) => elem_sort(&r.elem),
        Type::Slice(s) => sort_of(&s.elem),
        Type::Array(a) => sort_of(&a.elem),
        Type::Path(p) => match p.path.segments.last().map(|s| &s.arguments) {
            Some(syn::PathArguments::AngleBracketed(a)) => match a.args.first() {
                Some(syn::GenericArgument::Type(t)) => sort_of(t),
                _ => "Obj",
            },
            _ => "Obj",
        },
        _ => "Obj",
    }
}

impl Lower<'_> {
    fn loc(&self, l: u64) -> Value {
        json!({"file": self.o.file, "line": l, "endLine": l})
    }

    fn mark(&mut self, l: u64) {
        if !self.modelled.contains(&l) {
            self.modelled.push(l);
        }
    }

    fn unsupported(&mut self, l: u64, what: &str) -> Value {
        self.unmodelled = true;
        self.not_modelled.push(json!({"location": self.loc(l), "reason": format!("unsupported: {what}")}));
        json!({"op": "call", "name": "unmodelled", "args": []})
    }

    fn block(&mut self, stmts: &[Stmt]) -> Value {
        let Some((first, rest)) = stmts.split_first() else {
            return json!({"op": "nothing"});
        };
        let l = line(first);
        match first {
            Stmt::Local(local) => {
                self.mark(l);
                let name = match &local.pat {
                    Pat::Ident(i) => i.ident.to_string(),
                    Pat::Type(t) => match &*t.pat {
                        Pat::Ident(i) => i.ident.to_string(),
                        _ => return self.unsupported(l, "pattern"),
                    },
                    _ => return self.unsupported(l, "pattern"),
                };
                let v = match &local.init {
                    Some(init) => self.expr(&init.expr),
                    None => return self.unsupported(l, "let without value"),
                };
                let body = self.block(rest);
                json!({"op": "let", "name": name, "args": [v, body]})
            }
            Stmt::Expr(e, semi) => {
                // `if c { diverges }` followed by more statements: the rest is the else arm
                if let Expr::If(i) = e {
                    if i.else_branch.is_none() && !rest.is_empty() {
                        self.mark(line(&i.cond));
                        let c = self.expr(&i.cond);
                        let t = self.block(&i.then_branch.stmts);
                        let r = self.block(rest);
                        return json!({"op": "if", "args": [c, t, r], "location": self.loc(line(&i.cond))});
                    }
                }
                if rest.is_empty() || diverges(e) {
                    let v = self.expr(e);
                    if semi.is_some() && !diverges(e) {
                        return json!({"op": "seq", "args": [v, {"op": "nothing"}]});
                    }
                    return v;
                }
                let v = self.expr(e);
                let r = self.block(rest);
                json!({"op": "seq", "args": [v, r]})
            }
            Stmt::Macro(m) => {
                let e = Expr::Macro(syn::ExprMacro { attrs: vec![], mac: m.mac.clone() });
                self.expr(&e)
            }
            Stmt::Item(_) => self.unsupported(l, "item"),
        }
    }

    fn expr(&mut self, e: &Expr) -> Value {
        let l = line(e);
        self.mark(l);
        match e {
            Expr::Paren(p) => self.expr(&p.expr),
            Expr::Group(g) => self.expr(&g.expr),
            Expr::Block(b) => self.block(&b.block.stmts),
            Expr::Lit(lit) => match &lit.lit {
                Lit::Bool(b) => json!({"op": "lit", "value": b.value, "sort": "Bool"}),
                Lit::Int(i) => match i.base10_parse::<i64>() {
                    Ok(n) => json!({"op": "lit", "value": n, "sort": "Int"}),
                    Err(_) => self.unsupported(l, "integer literal"),
                },
                Lit::Str(s) => json!({"op": "lit", "value": s.value(), "sort": "String"}),
                _ => self.unsupported(l, "literal"),
            },
            Expr::Path(p) if p.path.segments.len() == 1 => {
                let n = p.path.segments[0].ident.to_string();
                let sort = self.sorts.iter().find(|(k, _)| *k == n).map(|(_, s)| s.clone());
                match sort {
                    Some(s) => json!({"op": "var", "name": n, "sort": s}),
                    None => json!({"op": "var", "name": n}),
                }
            }
            Expr::Unary(u) if matches!(u.op, UnOp::Not(_)) => {
                let a = self.expr(&u.expr);
                json!({"op": "not", "args": [a]})
            }
            Expr::Binary(b) => {
                let (x, y) = (self.expr(&b.left), self.expr(&b.right));
                match b.op {
                    BinOp::And(_) => json!({"op": "and", "args": [x, y]}),
                    BinOp::Or(_) => json!({"op": "or", "args": [x, y]}),
                    BinOp::Eq(_) => json!({"op": "eq", "args": [x, y]}),
                    BinOp::Ne(_) => json!({"op": "not", "args": [{"op": "eq", "args": [x, y]}]}),
                    _ => self.unsupported(l, "binary operator"),
                }
            }
            Expr::If(i) => {
                let c = self.expr(&i.cond);
                let t = self.block(&i.then_branch.stmts);
                let f = match &i.else_branch {
                    Some((_, e)) => self.expr(e),
                    None => json!({"op": "nothing"}),
                };
                json!({"op": "if", "args": [c, t, f], "location": self.loc(line(&i.cond))})
            }
            Expr::Return(r) => match &r.expr {
                Some(v) => self.expr(v),
                None => json!({"op": "nothing"}),
            },
            Expr::Index(ix) => {
                let el = match &*ix.expr {
                    Expr::Path(p) if p.path.segments.len() == 1 => {
                        let n = p.path.segments[0].ident.to_string();
                        self.sorts.iter().find(|(k, _)| *k == format!("{n}[]")).map(|(_, s)| s.clone())
                    }
                    _ => None,
                };
                let (a, i) = (self.expr(&ix.expr), self.expr(&ix.index));
                let mut v = json!({"op": "index", "contract": "index-empty", "args": [a, i], "location": self.loc(l)});
                if let Some(s) = el {
                    v["sort"] = json!(s);
                }
                v
            }
            Expr::Macro(m) if m.mac.path.is_ident("panic") => {
                json!({"op": "throw", "name": "panic", "location": self.loc(l)})
            }
            _ => self.unsupported(l, "expression"),
        }
    }
}

fn diverges(e: &Expr) -> bool {
    matches!(e, Expr::Return(_)) || matches!(e, Expr::Macro(m) if m.mac.path.is_ident("panic"))
}

/// Lower function `o.function` of the Rust source `src` to a model (schema `model`).
pub fn lower(src: &str, o: &Options) -> Result<Value, String> {
    let file = syn::parse_file(src).map_err(|e| format!("parse: {e}"))?;
    let f = file
        .items
        .iter()
        .find_map(|i| match i {
            Item::Fn(f) if f.sig.ident == o.function => Some(f),
            _ => None,
        })
        .ok_or_else(|| format!("no function {}", o.function))?;
    let mut lw = Lower { o, modelled: vec![], not_modelled: vec![], unmodelled: false, sorts: vec![] };
    let mut params = vec![];
    for a in &f.sig.inputs {
        let syn::FnArg::Typed(pt) = a else { return Err("methods with self are not supported".into()) };
        let Pat::Ident(id) = &*pt.pat else { return Err("parameter pattern".into()) };
        let n = id.ident.to_string();
        let s = sort_of(&pt.ty);
        lw.sorts.push((n.clone(), s.to_string()));
        if s == "Obj" {
            lw.sorts.push((format!("{n}[]"), elem_sort(&pt.ty).to_string()));
        }
        params.push(json!({"name": n, "sort": s}));
    }
    let body = lw.block(&f.block.stmts);
    let site = line(&f.sig.ident);
    let mut opaque = vec![];
    if lw.unmodelled {
        opaque.push(json!({"name": "unmodelled", "symbol": "unmodelled", "args": [], "result": "Obj"}));
    }
    let mut contracts = vec![];
    if body.to_string().contains("index-empty") {
        contracts.push(json!({"contract": "index-empty", "symbol": "core::ops::Index<usize> for [T]"}));
    }
    lw.modelled.sort();
    Ok(json!({
        "unit": format!("{}#{}", o.function, site),
        "version": o.version,
        "params": params,
        "opaque": opaque,
        "contracts": contracts,
        "body": body,
        "lines": {"modelled": lw.modelled, "notModelled": lw.not_modelled},
    }))
}
