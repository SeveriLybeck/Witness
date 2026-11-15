//! Lowering of Charon's structured LLBC (`charon rustc --preset=aeneas
//! --no-dedup-serialized-ast`) to the model. The body is run symbolically:
//! locals hold model expressions, a `Switch` arm continues with the statements
//! after the switch unless it returns, an `Assert` is `if cond then rest else
//! throw panic`, `core::slice::index` calls are `index` (contract
//! `index-empty`). Anything else is an `unmodelled` opaque and its line is
//! not modelled (playbook X2).

use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Strip Charon's `Untagged` / `Value` wrappers.
fn u(v: &Value) -> &Value {
    match v {
        Value::Object(m) if m.len() == 1 && m.contains_key("Untagged") => u(&m["Untagged"]),
        _ => v,
    }
}

fn sort_of(ty: &Value) -> &'static str {
    let t = u(ty);
    if let Some(s) = t.get("Scalar") {
        return if s == "Bool" { "Bool" } else if s.get("Integer").is_some() { "Int" } else { "Obj" };
    }
    if let Some(r) = t.get("Ref") {
        let inner = u(&r[1]);
        if inner.get("Slice").is_some() || inner.get("Array").is_some() {
            return "Obj";
        }
        return sort_of(&r[1]);
    }
    "Obj"
}

fn elem_sort(ty: &Value) -> Option<&'static str> {
    let t = u(ty);
    if let Some(r) = t.get("Ref") {
        return elem_sort(&r[1]);
    }
    t.get("Slice").or_else(|| t.get("Array")).map(|s| sort_of(&s[0]))
}

fn line_of(span: &Value) -> Option<u64> {
    u(span).get("data")?.get("beg")?.get("line")?.as_u64()
}

struct Lw<'a> {
    file: &'a str,
    names: BTreeMap<u64, String>,
    modelled: Vec<u64>,
    not_modelled: Vec<Value>,
    unmodelled: bool,
    index: bool,
    params: Vec<(String, &'static str)>,
}

#[derive(Clone)]
struct Env(BTreeMap<u64, Value>);

impl Lw<'_> {
    fn loc(&self, l: u64) -> Value {
        json!({"file": self.file, "line": l, "endLine": l})
    }

    fn unsupported(&mut self, l: Option<u64>, what: &str) -> Value {
        self.unmodelled = true;
        if let Some(l) = l {
            self.not_modelled.push(json!({"location": self.loc(l), "reason": format!("unsupported: {what}")}));
        }
        json!({"op": "call", "name": "unmodelled", "args": []})
    }

    fn place(&mut self, p: &Value, env: &Env, l: Option<u64>) -> Value {
        let k = &p["kind"];
        if let Some(n) = k.get("Local").and_then(Value::as_u64) {
            return env.0.get(&n).cloned().unwrap_or_else(|| self.unsupported(l, "uninitialised local"));
        }
        if let Some(pr) = k.get("Projection") {
            if pr[1] == "Deref" {
                return self.place(&pr[0], env, l);
            }
        }
        self.unsupported(l, "place projection")
    }

    fn operand(&mut self, o: &Value, env: &Env, l: Option<u64>) -> Value {
        for k in ["Copy", "Move"] {
            if let Some(p) = o.get(k) {
                let p = if p.is_array() { &p[0] } else { p };
                return self.place(p, env, l);
            }
        }
        if let Some(c) = o.get("Const") {
            let c = u(c);
            let lit = if c.is_array() { &c[0] } else { c.get("kind").unwrap_or(c) };
            let lit = u(lit);
            if let Some(b) = lit.get("Bool").and_then(Value::as_bool) {
                return json!({"op": "lit", "value": b, "sort": "Bool"});
            }
            if let Some(i) = lit.get("Integer") {
                let v = i.as_object().and_then(|m| m.values().next()).and_then(|a| a[1].as_str()).and_then(|s| s.parse::<i64>().ok());
                if let Some(n) = v {
                    return json!({"op": "lit", "value": n, "sort": "Int"});
                }
            }
            return self.unsupported(l, "constant");
        }
        self.unsupported(l, "operand")
    }

    fn rvalue(&mut self, r: &Value, env: &Env, l: Option<u64>) -> Value {
        if let Some(a) = r.get("Use") {
            let o = if a.is_array() { &a[0] } else { a };
            return self.operand(o, env, l);
        }
        if let Some(rf) = r.get("Ref") {
            return self.place(&rf["place"], env, l);
        }
        if let Some(a) = r.get("UnaryOp") {
            if a[0] == "Not" {
                let x = self.operand(&a[1], env, l);
                return json!({"op": "not", "args": [x]});
            }
        }
        if let Some(a) = r.get("BinaryOp") {
            let (x, y) = (self.operand(&a[1], env, l), self.operand(&a[2], env, l));
            return match a[0].as_str() {
                Some("Eq") => json!({"op": "eq", "args": [x, y]}),
                Some("Ne") => json!({"op": "not", "args": [{"op": "eq", "args": [x, y]}]}),
                Some("BitAnd") => json!({"op": "and", "args": [x, y]}),
                Some("BitOr") => json!({"op": "or", "args": [x, y]}),
                _ => self.unsupported(l, "binary operator"),
            };
        }
        self.unsupported(l, "rvalue")
    }

    fn mark(&mut self, l: Option<u64>) {
        if let Some(l) = l {
            if !self.modelled.contains(&l) {
                self.modelled.push(l);
            }
        }
    }

    fn panic(&mut self, l: Option<u64>) -> Value {
        let mut t = json!({"op": "throw", "name": "panic"});
        if let Some(l) = l {
            t["location"] = self.loc(l);
        }
        t
    }

    /// Run `stmts` (a block followed by its continuation) and return the unit's value.
    fn run(&mut self, stmts: &[&Value], mut env: Env) -> Value {
        let mut i = 0;
        while i < stmts.len() {
            let s = stmts[i];
            let l = s.get("span").and_then(line_of);
            let k = &s["kind"];
            let rest = &stmts[i + 1..];
            if k == "Return" {
                return env.0.get(&0).cloned().unwrap_or(json!({"op": "nothing"}));
            }
            if let Some(a) = k.get("Assign") {
                self.mark(l);
                let v = self.rvalue(&a[1], &env, l);
                match a[0]["kind"].get("Local").and_then(Value::as_u64) {
                    Some(n) => {
                        env.0.insert(n, v);
                    }
                    None => {
                        self.unsupported(l, "assignment target");
                    }
                }
            } else if let Some(a) = k.get("Assert") {
                self.mark(l);
                let a = a.get("assert").unwrap_or(a);
                let c = self.operand(&a["cond"], &env, l);
                let ok = if a["expected"] == true { c } else { json!({"op": "not", "args": [c]}) };
                let cont = self.run(rest, env.clone());
                let p = self.panic(l);
                let mut e = json!({"op": "if", "args": [ok, cont, p]});
                if let Some(l) = l {
                    e["location"] = self.loc(l);
                }
                return e;
            } else if k.get("Abort").is_some() || k.get("Panic").is_some() {
                self.mark(l);
                return self.panic(l);
            } else if let Some(sw) = k.get("Switch") {
                self.mark(l);
                let data = &sw["data"];
                let scr = data["scrutinee"].get("Value").unwrap_or(&data["scrutinee"]);
                let c = self.operand(scr, &env, l);
                let arms = sw["branches"].as_array().cloned().unwrap_or_default();
                let cases = data["branches"].as_array().cloned().unwrap_or_default();
                // boolean switch: one listed case (true or false), fallback the other
                if cases.len() != 1 || arms.len() != 2 {
                    return self.unsupported(l, "switch");
                }
                let lit = u(&u(&cases[0][0])[0]);
                let on = lit.get("Bool").and_then(Value::as_bool);
                let case_arm = cases[0][1].as_u64().unwrap_or(0) as usize;
                let fall = data["fallback"].as_u64().unwrap_or(0) as usize;
                let Some(on) = on else { return self.unsupported(l, "non-boolean switch") };
                let go =|me: &mut Self, arm: usize| {
                    let mut v: Vec<&Value> = arms[arm]["statements"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
                    v.extend_from_slice(rest);
                    // arms borrow from a cloned array; run immediately
                    me.run(&v, env.clone())
                };
                let (t, f) = if on { (go(self, case_arm), go(self, fall)) } else { (go(self, fall), go(self, case_arm)) };
                let mut e = json!({"op": "if", "args": [c, t, f]});
                if let Some(l) = l {
                    e["location"] = self.loc(l);
                }
                return e;
            } else if let Some(c) = k.get("Call") {
                self.mark(l);
                let call = &c["call"];
                let fid = call["func"]["Regular"]["kind"]["Fun"].as_u64();
                let name = fid.and_then(|f| self.names.get(&f)).cloned().unwrap_or_default();
                let args: Vec<Value> = call["args"].as_array().cloned().unwrap_or_default();
                let dest = c["dest"]["kind"]["Local"].as_u64();
                if name.contains("panic") {
                    return self.panic(l);
                }
                let v = if name.starts_with("core::slice::index") && name.ends_with("::index") && args.len() == 2 {
                    self.index = true;
                    let i = self.operand(&args[0], &env, l);
                    let a = self.operand(&args[1], &env, l);
                    let mut e = json!({"op": "index", "contract": "index-empty", "args": [a, i]});
                    if let Some(s) = c["dest"]["ty"].get("Untagged").and_then(|t| t.get("Ref")).map(|r| sort_of(&r[1])) {
                        e["sort"] = json!(s);
                    }
                    if let Some(l) = l {
                        e["location"] = self.loc(l);
                    }
                    e
                } else {
                    self.unsupported(l, &format!("call {name}"))
                };
                if let Some(d) = dest {
                    env.0.insert(d, v);
                }
            } else if k.get("StorageLive").is_some() || k.get("StorageDead").is_some() || k == "Nop" {
            } else {
                self.unsupported(l, "statement");
            }
            i += 1;
        }
        env.0.get(&0).cloned().unwrap_or(json!({"op": "nothing"}))
    }
}

fn path_name(name: &Value) -> String {
    name.as_array()
        .map(|a| a.iter().filter_map(|p| p.get("Ident").and_then(|i| i[0].as_str())).collect::<Vec<_>>().join("::"))
        .unwrap_or_default()
}

/// Lower function `function` (last path segment) of a Charon LLBC file to a model.
pub fn lower(llbc: &Value, function: &str, file: &str, version: &str) -> Result<Value, String> {
    let t = &llbc["translated"];
    let decls = t["fun_decls"].as_array().ok_or("llbc: no fun_decls")?;
    let mut names = BTreeMap::new();
    let mut target = None;
    for (i, d) in decls.iter().enumerate() {
        let d = u(d);
        if d.is_null() {
            continue;
        }
        let n = path_name(&d["item_meta"]["name"]);
        let id = d["def_id"].as_u64().unwrap_or(i as u64);
        if n.rsplit("::").next() == Some(function) && d["body"].get("Structured").is_some() {
            target = Some(d);
        }
        names.insert(id, n);
    }
    let f = target.ok_or_else(|| format!("llbc: no function {function} with a structured body"))?;
    let body = &f["body"]["Structured"];
    let locals = body["locals"]["locals"].as_array().ok_or("llbc: no locals")?;
    let argc = body["locals"]["arg_count"].as_u64().unwrap_or(0);
    let inputs = f["signature"]["inputs"].as_array().cloned().unwrap_or_default();
    let mut lw = Lw { file, names, modelled: vec![], not_modelled: vec![], unmodelled: false, index: false, params: vec![] };
    let mut env = Env(BTreeMap::new());
    for i in 1..=argc {
        let n = locals[i as usize]["name"].as_str().map(str::to_string).unwrap_or(format!("_{i}"));
        let ty = inputs.get(i as usize - 1).cloned().unwrap_or(locals[i as usize]["ty"].clone());
        let s = sort_of(&ty);
        let _ = elem_sort(&ty);
        lw.params.push((n.clone(), s));
        env.0.insert(i, json!({"op": "var", "name": n, "sort": s}));
    }
    let stmts: Vec<&Value> = body["body"]["statements"].as_array().ok_or("llbc: no statements")?.iter().collect();
    let model_body = lw.run(&stmts, env);
    let site = line_of(&f["item_meta"]["span"]).unwrap_or(0);
    let mut opaque = vec![];
    if lw.unmodelled {
        opaque.push(json!({"name": "unmodelled", "symbol": "unmodelled", "args": [], "result": "Obj"}));
    }
    let mut contracts = vec![];
    if lw.index {
        contracts.push(json!({"contract": "index-empty", "symbol": "core::slice::index::SliceIndex<usize>::index"}));
    }
    lw.modelled.sort();
    Ok(json!({
        "unit": format!("{function}#{site}"),
        "version": version,
        "params": lw.params.iter().map(|(n, s)| json!({"name": n, "sort": s})).collect::<Vec<_>>(),
        "opaque": opaque,
        "contracts": contracts,
        "body": model_body,
        "lines": {"modelled": lw.modelled, "notModelled": lw.not_modelled},
    }))
}
