//! C# translator (group 5): lowers one changed method's Roslyn control-flow graph, as serialised by
//! the C# worker (`worker-reply.schema.json#/$defs/cfg`), to the model (`model.schema.json`), and
//! counts modelled lines into `scope.json` (D16, D20).

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The five implicit-exception contracts (common.schema.json `contract`).
pub const CONTRACTS: [&str; 5] = ["index-empty", "failed-cast", "nullable-value-null", "dictionary-duplicate-add", "null-receiver"];

/// Expression trees larger than this are `not modelled` (path explosion).
pub const MAX_NODES: usize = 200_000;

#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Graph name of the unit (e.g. `TypeCollector.GetObjectInfo`).
    pub unit: String,
    /// Value of the model's `unit` field; defaults to `unit`.
    pub unit_id: Option<String>,
    /// `old`, `new` or `reverted`.
    pub version: String,
    /// Methods this change adds or edits: graph names or documentation-comment IDs. Calls to them are inlined.
    pub inline: BTreeSet<String>,
    /// Delegate fields assigned in changed code: field symbol -> graph name (method or `lambdas` entry).
    pub delegates: BTreeMap<String, String>,
    /// Changed lines of this version's side in the unit's file, grouped by part.
    /// A part is modelled when one of its lines lies in a lowered operation or graph and none in a not-modelled range.
    pub parts: Vec<Vec<u64>>,
    /// Contracts switched off (for the "removing the contract" check).
    pub disabled: BTreeSet<String>,
    /// Site lines: when non-empty the unit is the region of the graph around these lines (playbook X2 A),
    /// widened to whole statements and to the uses of locals it assigns; values entering it are free params.
    pub region: Vec<u64>,
    /// With `region`: wrap the site in its innermost enclosing `foreach` as a per-iteration unit (D19):
    /// `{"op":"foreach","name":<element>,"sort":…,"args":[<collection>, <site body>]}`.
    pub per_iteration: bool,
    /// With `region`: the region is the blocks on the given lines (old and new get the same line range from the
    /// caller), not widened backwards over preceding statements.
    pub exact: bool,
}

#[derive(Debug)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

/// Accepts a whole worker reply (`{id, method, result}`) or its `result`.
pub fn cfg_result(reply: &Value) -> Result<&Value, Error> {
    if let Some(e) = reply.get("error") {
        return Err(Error(format!("worker error: {}", e.get("message").and_then(Value::as_str).unwrap_or(""))));
    }
    let r = reply.get("result").unwrap_or(reply);
    if r.get("methods").and_then(Value::as_array).is_none() {
        return Err(Error("no cfg methods".into()));
    }
    Ok(r)
}

/// `TypeCollector.GetObjectInfo` from a graph name or a doc ID such as `M:Ns.TypeCollector.GetObjectInfo(...)`.
pub fn method_key(s: &str) -> String {
    let s = s.split_once(':').filter(|(p, _)| p.len() == 1).map(|(_, r)| r).unwrap_or(s);
    let s = s.split('(').next().unwrap_or(s);
    let segs: Vec<&str> = s.split('.').filter(|x| !x.is_empty()).collect();
    let n = segs.len();
    if n >= 2 { format!("{}.{}", segs[n - 2], segs[n - 1]) } else { s.to_string() }
}

/// All graphs (methods and, recursively, lambdas) by name.
fn collect_graphs<'a>(gs: &'a [Value], out: &mut HashMap<String, &'a Value>) {
    for g in gs {
        if let Some(n) = g.get("name").and_then(Value::as_str) {
            out.entry(n.to_string()).or_insert(g);
            out.entry(method_key(n)).or_insert(g);
        }
        if let Some(l) = g.get("lambdas").and_then(Value::as_array) {
            collect_graphs(l, out);
        }
    }
}

fn line_span(loc: &Value) -> Option<(u64, u64)> {
    let a = loc.get("line")?.as_u64()?;
    Some((a, loc.get("endLine").and_then(Value::as_u64).unwrap_or(a)))
}

fn walk_ops<'a>(op: &'a Value, f: &mut dyn FnMut(&'a Value)) {
    f(op);
    if let Some(c) = op.get("children").and_then(Value::as_array) {
        for x in c {
            walk_ops(x, f);
        }
    }
}

fn graph_ops<'a>(g: &'a Value, f: &mut dyn FnMut(&'a Value)) {
    for b in g.get("blocks").and_then(Value::as_array).into_iter().flatten() {
        for o in b.get("operations").and_then(Value::as_array).into_iter().flatten() {
            walk_ops(o, f);
        }
        if let Some(br) = b.get("branch").filter(|v| v.is_object()) {
            walk_ops(br, f);
        }
    }
}

/// Delegate fields assigned in changed code: in every graph, a `SimpleAssignment` to a field on a changed line,
/// paired with a lambda of that graph that starts on the same line (2283: `reportDiagnostic` -> `lambda@239`).
pub fn derive_delegates(cfg: &Value, changed: &BTreeSet<u64>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack: Vec<&Value> = cfg.get("methods").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    while let Some(g) = stack.pop() {
        let lambdas: Vec<&Value> = g.get("lambdas").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
        stack.extend(lambdas.iter().copied());
        // flow captures of fields (the real worker assigns `this.f = …` through a captured field reference)
        let mut cap_field: HashMap<String, String> = HashMap::new();
        let mut lambda_at: Vec<(u64, String)> = Vec::new();
        graph_ops(g, &mut |op| {
            if kind_of(op) == "FlowCapture" {
                if let (Some(c), Some(ch)) = (op.get("capture"), children(op).first()) {
                    if kind_of(ch) == "FieldReference" {
                        cap_field.insert(c.to_string(), s(ch, "symbol").to_string());
                    }
                }
            }
            if let (Some(gn), Some((l, _))) = (op.get("graph").and_then(Value::as_str), op.get("location").and_then(line_span)) {
                lambda_at.push((l, gn.to_string()));
            }
        });
        let mut assigns: Vec<(u64, String)> = Vec::new();
        graph_ops(g, &mut |op| {
            if kind_of(op) != "SimpleAssignment" {
                return;
            }
            let Some((line, _)) = op.get("location").and_then(line_span) else { return };
            if !changed.contains(&line) {
                return;
            }
            let Some(t) = children(op).first() else { return };
            let sym = match kind_of(t) {
                "FieldReference" => s(t, "symbol").to_string(),
                "FlowCaptureReference" => match t.get("capture").and_then(|c| cap_field.get(&c.to_string())) {
                    Some(f) => f.clone(),
                    None => return,
                },
                _ => return,
            };
            if !sym.is_empty() {
                assigns.push((line, sym));
            }
        });
        for (line, sym) in assigns {
            let lam = lambda_at.iter().find(|(l, _)| *l == line).map(|x| x.1.clone()).or_else(|| {
                lambdas.iter().find(|l| l.get("location").and_then(line_span).map(|s| s.0) == Some(line)).map(|l| s(l, "name").to_string())
            });
            if let Some(n) = lam {
                out.insert(sym, n);
            }
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
enum Del {
    No,
    Sink,
    Graph(String),
}

#[derive(Clone, Debug)]
struct Val {
    e: Value,
    sort: String,
    del: Del,
    diag: Option<(String, Vec<Val>)>,
    maybe_null: bool,
    /// a `DiagnosticDescriptor` field: (symbol, diagnostic ID if the worker gives `diagnosticId`)
    desc: Option<(String, Option<String>)>,
    /// An object built in this unit: its constructor arguments by parameter name.
    fields: Vec<(String, Val)>,
}

impl Val {
    fn new(e: Value, sort: String) -> Val {
        Val { e, sort, del: Del::No, diag: None, maybe_null: false, desc: None, fields: vec![] }
    }
}

#[derive(Clone, Default)]
struct Env {
    vars: HashMap<String, Val>,
    captures: Vec<(String, Val)>,
    reg: Option<Val>,
}

enum Frame {
    Effect(Value),
    Let(String, Value),
}

struct Lowerer<'a> {
    graphs: HashMap<String, &'a Value>,
    opts: &'a Options,
    inline: BTreeSet<String>,
    opaque: Vec<Value>,
    opaque_by_key: HashMap<String, String>,
    names: BTreeSet<String>,
    contracts: Vec<Value>,
    not_modelled: Vec<Value>,
    covered: Vec<(u64, u64)>,
    stack: Vec<String>,
    fresh: usize,
    seen: BTreeSet<String>,
    site: String,
    /// Values entering a region unit: name -> sort.
    free: BTreeMap<String, String>,
    /// Blocks of the region unit (top graph only).
    region: Option<BTreeSet<u64>>,
    /// Exits of a closed per-iteration region.
    closure: Option<Closure>,
    /// Locals and parameters of the unit graph that the code compares with null.
    nullable: BTreeSet<String>,
    /// Graphs lowered by inlining (their lines are compared).
    inlined: BTreeSet<String>,
}

/// Roslyn operation kind without the `Operation` class-name suffix (`FlowCaptureOperation` -> `FlowCapture`).
fn kind_of(op: &Value) -> &str {
    let k = s(op, "kind");
    k.strip_suffix("Operation").filter(|x| !x.is_empty()).unwrap_or(k)
}

const ACTION_DIAG: &str = "System.Action<Microsoft.CodeAnalysis.Diagnostic>";

/// Model sort of a C# type.
pub fn sort_of(ty: Option<&str>) -> String {
    let t = match ty {
        None => return "Unit".into(),
        Some(t) => t,
    };
    let (t, opt) = match t.strip_suffix('?') {
        Some(b) => (b, true),
        None => (t, false),
    };
    let base = match t {
        "System.Boolean" | "bool" => "Bool".to_string(),
        "System.Int32" | "System.Int64" | "System.Int16" | "System.Byte" | "System.UInt32" | "System.UInt64" | "int" | "long" => "Int".into(),
        "System.String" | "string" => "String".into(),
        "System.Void" | "void" => "Unit".into(),
        _ => {
            let head = t.split('<').next().unwrap_or(t);
            let s: String = head.rsplit('.').next().unwrap_or(head).chars().filter(|c| c.is_ascii_alphanumeric()).collect();
            if s.is_empty() { "Obj".into() } else { s }
        }
    };
    if opt { format!("Opt{base}") } else { base }
}

fn short_name(sym: &str) -> String {
    let s = sym.split_once(':').filter(|(p, _)| p.len() == 1).map(|(_, r)| r).unwrap_or(sym);
    let s = s.split('(').next().unwrap_or(s);
    let last = s.rsplit('.').next().unwrap_or(s);
    let n: String = last.chars().take_while(|c| *c != '`').filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    let n = if n.is_empty() { "f".to_string() } else { n };
    let mut c = n.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => n,
    }
}

fn s<'v>(op: &'v Value, k: &str) -> &'v str {
    op.get(k).and_then(Value::as_str).unwrap_or("")
}

fn children(op: &Value) -> &[Value] {
    op.get("children").and_then(Value::as_array).map(|v| v.as_slice()).unwrap_or(&[])
}

fn is_effectful(e: &Value) -> bool {
    match e.get("op").and_then(Value::as_str) {
        Some("report" | "throw" | "if" | "let" | "seq") => return true,
        _ => {}
    }
    if e.get("contract").is_some() {
        return true;
    }
    e.get("args").and_then(Value::as_array).map(|a| a.iter().any(is_effectful)).unwrap_or(false)
}

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// Name of a local, parameter or argument (`name` from the worker, else the syntax).
fn name_of(op: &Value) -> String {
    op.get("name").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| s(op, "syntax").trim_start_matches("this.").to_string())
}

/// Key of a flow capture: the worker's capture number, else the syntax.
fn cap_key(op: &Value) -> String {
    match op.get("capture") {
        Some(c) if !c.is_null() => format!("#{c}"),
        _ => s(op, "syntax").to_string(),
    }
}

fn ident(s: &str) -> String {
    let v: String = s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let v = v.trim_matches('_').to_string();
    if v.is_empty() { "v".into() } else { v }
}

fn count_nodes(e: &Value) -> usize {
    1 + e.get("args").and_then(Value::as_array).map(|a| a.iter().map(count_nodes).sum()).unwrap_or(0)
}

fn nothing() -> Value {
    json!({"op": "nothing"})
}

impl<'a> Lowerer<'a> {
    fn opaque_call(&mut self, key: &str, display: &str, args: Vec<Val>, result: &str) -> Value {
        let mut k = format!("{key}/{}", args.len());
        // one solver function per sort signature: a second use with other argument sorts (a nullable string where
        // the first use passed a string) is its own function, named from the key and the signature
        let class = |x: &str| match x { "Bool" | "Int" | "String" => x.to_string(), _ => "Obj".to_string() };
        let sig: Vec<String> = args.iter().map(|a| class(&a.sort)).collect();
        if let Some(n) = self.opaque_by_key.get(&k) {
            let declared = self.opaque.iter().find(|o| o["name"] == n.as_str())
                .map(|o| o["args"].as_array().into_iter().flatten().map(|x| class(x.as_str().unwrap_or(""))).collect::<Vec<_>>());
            if declared.as_ref() != Some(&sig) {
                k = format!("{k}:{}", sig.join(","));
            }
        }
        let name = match self.opaque_by_key.get(&k) {
            Some(n) => n.clone(),
            None => {
                // The name depends only on the key, so old and new models name a shared function alike.
                let n = format!("{}_{:04x}", short_name(display), fnv(&k) & 0xffff);
                self.names.insert(n.clone());
                self.opaque_by_key.insert(k, n.clone());
                self.opaque.push(json!({"name": n, "symbol": key,
                    "args": args.iter().map(|a| a.sort.clone()).collect::<Vec<_>>(), "result": result}));
                n
            }
        };
        json!({"op": "call", "name": name, "args": args.into_iter().map(|a| a.e).collect::<Vec<_>>()})
    }

    fn contract(&mut self, c: &str, symbol: &str) -> bool {
        if self.opts.disabled.contains(c) {
            return false;
        }
        // One counted assumption per source site, not per path through the tree.
        if self.seen.insert(format!("{c}|{symbol}|{}", self.site)) {
            self.contracts.push(json!({"contract": c, "symbol": symbol}));
        }
        true
    }

    fn unsupported(&mut self, op: &Value, reason: String) -> Val {
        let loc = op.get("location").cloned().unwrap_or(Value::Null);
        let entry = json!({"location": loc, "reason": reason});
        if !self.not_modelled.contains(&entry) {
            self.not_modelled.push(entry);
        }
        self.fresh += 1;
        let ty = op.get("type").and_then(Value::as_str);
        let e = self.opaque_call(&format!("unmodelled:{}#{}", kind_of(op), self.fresh), "unmodelled", vec![], &sort_of(ty));
        Val::new(e, sort_of(ty))
    }

    fn cover(&mut self, loc: Option<&Value>) {
        if let Some(sp) = loc.and_then(line_span) {
            self.covered.push(sp);
        }
    }

    fn fresh_name(&mut self, base: &str) -> String {
        self.fresh += 1;
        format!("{base}_{}", self.fresh)
    }

    /// The operand of an operation without children: the capture of the same syntax, else the current value.
    fn operand(&mut self, op: &Value, env: &mut Env, frames: &mut Vec<Frame>) -> Option<Val> {
        if let Some(c) = children(op).first() {
            return Some(self.expr(c, env, frames));
        }
        let key = cap_key(op);
        let syn = s(op, "syntax");
        if let Some((_, v)) = env.captures.iter().rev().find(|(k, _)| *k == key || k == syn) {
            return Some(v.clone());
        }
        env.reg.clone()
    }

    fn inline_graph(&mut self, name: &str, args: Vec<Val>, env_outer: &Env, frames: &mut Vec<Frame>, ty: Option<&str>) -> Option<Val> {
        let g = *self.graphs.get(name)?;
        if self.stack.iter().any(|x| x == name) {
            return None;
        }
        let params: Vec<&Value> = g.get("parameters").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
        let mut args = args;
        if args.len() == params.len() + 1 {
            args.remove(0);
        }
        if args.len() != params.len() {
            return None;
        }
        let mut env = Env::default();
        // A lambda sees the enclosing environment's variables (captures).
        env.vars = env_outer.vars.clone();
        for (p, a) in params.iter().zip(args) {
            env.vars.insert(s(p, "name").to_string(), a);
        }
        self.stack.push(name.to_string());
        self.inlined.insert(s(g, "name").to_string());
        self.cover(g.get("location"));
        let body = self.lower_block(g, 0, env, &mut Vec::new(), true);
        self.stack.pop();
        let rs = sort_of(ty);
        if rs == "Unit" {
            frames.push(Frame::Effect(body));
            Some(Val::new(nothing(), rs))
        } else {
            let n = self.fresh_name("ret");
            frames.push(Frame::Let(n.clone(), body));
            Some(Val { e: json!({"op": "var", "name": n, "sort": rs}), sort: rs, del: Del::No, diag: None, desc: None, fields: vec![], maybe_null: ty.map(|t| t.ends_with('?')).unwrap_or(false) })
        }
    }

    fn report(&mut self, d: Val, op: &Value) -> Value {
        let (name, args) = d.diag.clone().map(|(n, a)| (n, a.into_iter().map(|v| v.e).collect::<Vec<_>>())).unwrap_or_else(|| ("diagnostic".into(), vec![d.e.clone()]));
        // `name` is the diagnostic ID when the worker resolved it (`diagnosticId`), else the descriptor's syntax;
        // `descriptor` always holds the descriptor field's symbol.
        let mut r = json!({"op": "report", "name": name, "args": args, "location": op.get("location").cloned().unwrap_or(Value::Null)});
        if let Some((sym, _)) = &d.desc {
            r["descriptor"] = json!(sym);
        }
        r
    }

    fn free_var(&mut self, name: &str, sort: &str) -> Val {
        let n = ident(name);
        let so = if sort == "Unit" { "Obj".to_string() } else { sort.to_string() };
        let so = self.free.entry(n.clone()).or_insert(so).clone();
        Val::new(json!({"op": "var", "name": n, "sort": so}), so)
    }

    fn expr(&mut self, op: &Value, env: &mut Env, frames: &mut Vec<Frame>) -> Val {
        self.cover(op.get("location"));
        if let Some(l) = op.get("location") {
            self.site = format!("{}:{}:{}", s(l, "file"), l.get("line").and_then(Value::as_u64).unwrap_or(0), s(op, "syntax"));
        }
        let kind = kind_of(op);
        let ty = op.get("type").and_then(Value::as_str);
        let sym = op.get("symbol").and_then(Value::as_str).unwrap_or("");
        let syn = s(op, "syntax").to_string();
        let sort = sort_of(ty);
        match kind {
            "Literal" if op.get("constant").is_some() => {
                let c = &op["constant"];
                // the worker may give the constant as text; the operation's type decides
                if let Value::String(t) = c {
                    if sort == "Int" {
                        if let Ok(i) = t.parse::<i64>() {
                            return Val::new(json!({"op": "lit", "value": i, "sort": "Int"}), "Int".into());
                        }
                    }
                    if sort == "Bool" && (t == "true" || t == "false" || t == "True" || t == "False") {
                        return Val::new(json!({"op": "lit", "value": t.eq_ignore_ascii_case("true"), "sort": "Bool"}), "Bool".into());
                    }
                }
                match c {
                    Value::Null => Val { maybe_null: true, ..Val::new(json!({"op": "none"}), sort) },
                    Value::Bool(_) => Val::new(json!({"op": "lit", "value": c, "sort": "Bool"}), "Bool".into()),
                    Value::Number(_) if c.is_i64() || c.is_u64() => Val::new(json!({"op": "lit", "value": c, "sort": "Int"}), "Int".into()),
                    Value::String(_) => Val::new(json!({"op": "lit", "value": c, "sort": "String"}), "String".into()),
                    _ => self.unsupported(op, "literal of unsupported type".into()),
                }
            }
            "Literal" => {
                let t = syn.trim();
                let (v, so) = if t == "true" || t == "false" {
                    (json!(t == "true"), "Bool".to_string())
                } else if let Ok(i) = t.parse::<i64>() {
                    (json!(i), "Int".into())
                } else if t == "null" {
                    return Val { maybe_null: true, ..Val::new(json!({"op": "none"}), sort) };
                } else {
                    (json!(t.trim_matches('"')), "String".into())
                };
                Val::new(json!({"op": "lit", "value": v, "sort": so}), so)
            }
            "DefaultValue" => Val { maybe_null: true, ..Val::new(json!({"op": "none"}), sort) },
            "ParameterReference" | "LocalReference" => {
                let key = name_of(op);
                if let Some(v) = env.vars.get(&key) {
                    return v.clone();
                }
                // A value entering the unit (or captured by a lambda) is a free input.
                // a string the code tests for null is a nullable input
                let sort = if sort == "String" && self.nullable.contains(&key) { "OptString".to_string() } else { sort };
                let mut v = self.free_var(&key, &sort);
                if ty.map(|t| t.starts_with(ACTION_DIAG)).unwrap_or(false) {
                    v.del = Del::Sink;
                }
                // worker `maybeNull`: a reference-type local some path assigns `default`/`null`
                v.maybe_null = ty.map(|t| t.ends_with('?')).unwrap_or(false) || op.get("maybeNull").and_then(Value::as_bool) == Some(true);
                v
            }
            "FieldReference" | "InstanceReference" => {
                let key = if sym.is_empty() { "this".to_string() } else { sym.to_string() };
                let e = self.opaque_call(&key, if sym.is_empty() { "this" } else { sym }, vec![], &sort);
                let mut v = Val::new(e, sort);
                if ty == Some("Microsoft.CodeAnalysis.DiagnosticDescriptor") && !sym.is_empty() {
                    let id = op.get("diagnosticId").and_then(Value::as_str).map(str::to_string);
                    v.desc = Some((sym.to_string(), id));
                }
                if let Some(g) = self.opts.delegates.get(sym) {
                    v.del = Del::Graph(g.clone());
                } else if ty.map(|t| t.starts_with(ACTION_DIAG)).unwrap_or(false) {
                    v.del = Del::Sink;
                }
                v
            }
            "FlowCaptureReference" => {
                let key = cap_key(op);
                match env.captures.iter().rev().find(|(k, _)| *k == key) {
                    Some((_, v)) => v.clone(),
                    // captured before the region starts: a free input named after its syntax
                    None => {
                        let mut v = self.free_var(&format!("cap_{syn}"), &sort);
                        if ty.map(|t| t.starts_with(ACTION_DIAG)).unwrap_or(false) {
                            v.del = Del::Sink;
                        }
                        v
                    }
                }
            }
            "Increment" | "Decrement" if children(op).first().map(|c| kind_of(c) == "LocalReference").unwrap_or(false) => {
                // `x++` / `++x` on a local: the local becomes the opaque successor (no arithmetic in the model)
                let c = &children(op)[0];
                let old = self.expr(c, env, frames);
                let e = self.opaque_call(&format!("{kind}:{}", old.sort), kind, vec![old.clone()], &old.sort);
                let nv = self.bind(Val::new(e, old.sort.clone()), frames);
                env.vars.insert(name_of(c), nv.clone());
                if syn.trim_end().ends_with("++") || syn.trim_end().ends_with("--") { old } else { nv }
            }
            "InterpolatedString" => {
                // the text is fixed by the source; the value is a function of the interpolated expressions
                let mut args = vec![];
                for c in children(op).iter().filter(|c| kind_of(c) == "Interpolation") {
                    if let Some(x) = children(c).first() {
                        args.push(self.expr(x, env, frames));
                    }
                }
                let e = self.opaque_call(&format!("interpolated:{syn}"), "interpolated", args, "String");
                Val::new(e, "String".into())
            }
            "ArrayCreation" | "ArrayInitializer" | "Tuple" => {
                let args: Vec<Val> = children(op).iter().map(|c| self.expr(c, env, frames)).collect();
                let key = format!("{kind}:{}", ty.unwrap_or(""));
                let so = if sort == "Unit" { "Obj".to_string() } else { sort };
                let e = self.opaque_call(&key, kind, args, &so);
                Val::new(e, so)
            }
            "Argument" | "Parenthesized" => match children(op).first() {
                Some(c) => self.expr(c, env, frames),
                None => self.unsupported(op, format!("{kind} without operand")),
            },
            "IsNull" => match self.operand(op, env, frames) {
                Some(v) => Val::new(json!({"op": "isnull", "args": [v.e]}), "Bool".into()),
                None => self.unsupported(op, "IsNull without operand".into()),
            },
            "UnaryOperator" | "Unary" if s(op, "operator") == "Not" || syn.trim_start().starts_with('!') => match self.operand(op, env, frames) {
                Some(v) => Val::new(json!({"op": "not", "args": [v.e]}), "Bool".into()),
                None => self.unsupported(op, "operator without operand".into()),
            },
            "BinaryOperator" | "Binary" => {
                let c = children(op);
                if c.len() != 2 {
                    return self.unsupported(op, "binary operator arity".into());
                }
                let a = self.expr(&c[0], env, frames);
                let b = self.expr(&c[1], env, frames);
                let top = match s(op, "operator") {
                    "" => top_level_operator(&syn),
                    "Equals" => Some("==".into()),
                    "NotEquals" => Some("!=".into()),
                    "ConditionalAnd" => Some("&&".into()),
                    "ConditionalOr" => Some("||".into()),
                    "And" => Some("&".into()),
                    "Or" => Some("|".into()),
                    o => Some(o.to_string()),
                };
                match top.as_deref() {
                    Some("==") => Val::new(json!({"op": "eq", "args": [a.e, b.e]}), "Bool".into()),
                    Some("!=") => Val::new(json!({"op": "not", "args": [{"op": "eq", "args": [a.e, b.e]}]}), "Bool".into()),
                    Some("&&") | Some("&") if sort == "Bool" => Val::new(json!({"op": "and", "args": [a.e, b.e]}), "Bool".into()),
                    Some("||") | Some("|") if sort == "Bool" => Val::new(json!({"op": "or", "args": [a.e, b.e]}), "Bool".into()),
                    Some(o) => {
                        let o = o.to_string();
                        let e = self.opaque_call(&format!("operator:{o}:{}:{}:{}", a.sort, b.sort, sort), &format!("op{}", o.len()), vec![a, b], &sort);
                        Val::new(e, sort)
                    }
                    None => self.unsupported(op, "binary operator not recognised".into()),
                }
            }
            "IsPattern" | "IsType" => {
                let c = children(op);
                let Some(v) = (match c.first() { Some(x) => Some(self.expr(x, env, frames)), None => self.operand(op, env, frames) }) else {
                    return self.unsupported(op, "pattern without operand".into());
                };
                let pat = c.get(1);
                let pk = pat.map(|p| kind_of(p)).unwrap_or("");
                let ps = pat.map(|p| s(p, "syntax").trim().to_string()).unwrap_or_default();
                if pk == "DiscardPattern" {
                    return Val::new(json!({"op": "lit", "value": true, "sort": "Bool"}), "Bool".into());
                }
                if pk == "ConstantPattern" && ps == "null" {
                    return Val::new(json!({"op": "isnull", "args": [v.e]}), "Bool".into());
                }
                if pk == "NegatedPattern" && ps.replace(' ', "") == "notnull" {
                    return Val::new(json!({"op": "not", "args": [{"op": "isnull", "args": [v.e]}]}), "Bool".into());
                }
                let key = if kind == "IsType" { format!("is:{}", s(op, "syntax")) } else { format!("is:{pk}:{ps}") };
                let e = self.opaque_call(&key, "is", vec![v], "Bool");
                Val::new(e, "Bool".into())
            }
            "Conversion" => {
                let Some(v) = self.operand(op, env, frames) else { return self.unsupported(op, "conversion without operand".into()) };
                let t = syn.trim();
                let conv = s(op, "conversion");
                let target = sort_of(ty);
                let primitive = matches!(target.trim_start_matches("Opt"), "Bool" | "Int" | "String");
                if conv == "implicit" || (conv == "explicit" && primitive) {
                    // boxing keeps the value's own sort (the term is still Bool/Int/String)
                    let keep = matches!(v.sort.as_str(), "Bool" | "Int" | "String") && !primitive;
                    if keep { v } else { Val { sort, ..v } }
                } else if conv == "try-cast" || t.contains(" as ") {
                    // x as T  =  x is null ? null : (x is T ? x : null); the type test is a `T:` opaque (differ: w_is_T)
                    let v = self.bind(v, frames);
                    let tsym = format!("T:{}", ty.unwrap_or("").trim_end_matches('?'));
                    let test = self.opaque_call(&tsym, &format!("is{}", sort_of(ty).trim_start_matches("Opt")), vec![v.clone()], "Bool");
                    let e = json!({"op": "if", "args": [{"op": "isnull", "args": [v.e.clone()]}, {"op": "none"},
                        {"op": "if", "args": [test, v.e.clone(), {"op": "none"}]}]});
                    Val { maybe_null: true, ..Val::new(e, sort) }
                } else if conv == "explicit" || (conv.is_empty() && is_cast_syntax(t)) {
                    let tname = sort_of(ty).trim_start_matches("Opt").to_string();
                    let symbol = format!("({}){}", ty.unwrap_or("?"), children(op).first().and_then(|c| c.get("type")).and_then(Value::as_str).unwrap_or("?"));
                    if self.contract("failed-cast", &symbol) {
                        Val::new(json!({"op": "cast", "name": tname, "contract": "failed-cast", "args": [v.e], "syntax": syn}), sort)
                    } else {
                        Val::new(json!({"op": "cast", "name": tname, "args": [v.e]}), sort)
                    }
                } else {
                    Val { sort, ..v }
                }
            }
            "PropertyReference" | "ArrayElementReference" => {
                let args: Vec<Val> = children(op).iter().map(|c| self.expr(c, env, frames)).collect();
                let indexer = kind == "ArrayElementReference" || sym.contains(".Item(") || sym.contains("this[") || sym.ends_with(".Item");
                if indexer && args.len() == 2 {
                    let symbol = if sym.is_empty() { format!("{}[]", args[0].sort) } else { sym.to_string() };
                    let mut a = args;
                    let recv = a.remove(0);
                    let idx = a.remove(0);
                    let mut e = json!({"op": "index", "args": [recv.e, idx.e], "sort": sort, "syntax": syn});
                    if self.contract("index-empty", &symbol) {
                        e["contract"] = json!("index-empty");
                    }
                    return Val::new(e, sort);
                }
                // Diagnostic.Create(d, location, ...).Location is that location.
                if sym == "P:Microsoft.CodeAnalysis.Diagnostic.Location" && args.len() == 1 {
                    if let Some((_, rest)) = &args[0].diag {
                        // a null location becomes Location.None, never null: model null as None, no null-receiver
                        if let Some(l) = rest.first() {
                            return Val { maybe_null: false, ..l.clone() };
                        }
                    }
                }
                if sym.starts_with("P:System.Nullable`1.Value") && args.len() == 1 {
                    let mut e = json!({"op": "value", "sort": sort, "args": [args[0].e], "syntax": syn});
                    if self.contract("nullable-value-null", sym) {
                        e["contract"] = json!("nullable-value-null");
                    }
                    return Val::new(e, sort);
                }
                if sym.is_empty() {
                    return self.unsupported(op, "property without symbol".into());
                }
                // A positional record property (worker `positionalParameter`: its primary-constructor parameter,
                // which has the property's name) of an object built in this unit reads back that argument.
                if args.len() == 1 && op.get("positionalParameter").and_then(Value::as_u64).is_some() {
                    let p = sym.rsplit('.').next().unwrap_or("");
                    if let Some((_, v)) = args[0].fields.iter().find(|(n, _)| n == p) {
                        return v.clone();
                    }
                }
                self.call(op, sym, args, &sort, ty)
            }
            "Invocation" | "ObjectCreation" | "FlowCapture" => {
                if sym.is_empty() {
                    return self.unsupported(op, format!("{kind} without symbol"));
                }
                let args: Vec<Val> = children(op).iter().map(|c| self.expr(c, env, frames)).collect();
                // Delegate invocation: inline an installed delegate, or report to the sink.
                if sym.contains(".Invoke(") {
                    if let Some(recv) = args.first().cloned() {
                        match recv.del {
                            Del::Graph(g) => {
                                if let Some(v) = self.inline_graph(&g, args[1..].to_vec(), env, frames, ty) {
                                    return v;
                                }
                                return self.unsupported(op, format!("delegate {g} not inlinable"));
                            }
                            Del::Sink if args.len() == 2 => {
                                let r = self.report(args[1].clone(), op);
                                frames.push(Frame::Effect(r));
                                return Val::new(nothing(), "Unit".into());
                            }
                            _ => {}
                        }
                    }
                }
                if sym.starts_with("M:Microsoft.CodeAnalysis.Diagnostic.Create") && !args.is_empty() {
                    let d0 = args[0].desc.clone();
                    let desc = match &d0 {
                        Some((_, Some(id))) => id.clone(),
                        _ => children(op)[0].get("syntax").and_then(Value::as_str).unwrap_or("diagnostic").to_string(),
                    };
                    let rest: Vec<Val> = args[1..].to_vec();
                    let e = self.opaque_call(sym, "diagnosticCreate", args, &sort);
                    return Val { diag: Some((desc, rest)), desc: d0, ..Val::new(e, sort) };
                }
                let key = method_key(sym);
                let bare = key.rsplit('.').next().unwrap_or(&key).to_string();
                if kind == "Invocation" && (self.inline.contains(&key) || self.inline.contains(sym) || self.inline.contains(&bare)) && self.graphs.contains_key(&key) {
                    if let Some(v) = self.inline_graph(&key, args.clone(), env, frames, ty) {
                        return v;
                    }
                    return self.unsupported(op, format!("call to changed method {key} not inlinable"));
                }
                if kind == "ObjectCreation" {
                    // remember the constructor arguments by parameter name (see the property read above)
                    let fields: Vec<(String, Val)> = children(op).iter().zip(&args)
                        .filter(|(c, _)| kind_of(c) == "Argument")
                        .filter_map(|(c, a)| c.get("name").and_then(Value::as_str).map(|n| (n.to_string(), a.clone()))).collect();
                    let v = self.call(op, sym, args, &sort, ty);
                    return Val { fields, ..v };
                }
                self.call(op, sym, args, &sort, ty)
            }
            "DelegateCreation" | "FlowAnonymousFunction" | "AnonymousFunction" => {
                let line = op.get("location").and_then(line_span).map(|x| x.0);
                let mut linked = None;
                walk_ops(op, &mut |o| {
                    if linked.is_none() {
                        linked = o.get("graph").and_then(Value::as_str).map(str::to_string);
                    }
                });
                let lambda = linked.or_else(|| self.graphs.values().find(|g| s(g, "name").starts_with("lambda") && g.get("location").and_then(line_span).map(|x| x.0) == line).map(|g| s(g, "name").to_string()));
                match lambda.or_else(|| children(op).first().and_then(|c| c.get("symbol")).and_then(Value::as_str).map(method_key)) {
                    Some(g) => {
                        // a lambda is its text (worker `graphHash` / `hash`): one that only moved is the same value,
                        // one whose text changed is another
                        let mut hash = None;
                        walk_ops(op, &mut |o| {
                            if hash.is_none() {
                                hash = o.get("graphHash").and_then(Value::as_str).map(str::to_string);
                            }
                        });
                        let hash = hash.or_else(|| self.graphs.get(&g).and_then(|x| x.get("hash")).and_then(Value::as_str).map(str::to_string));
                        let id = hash.map(|h| h.chars().take(16).collect::<String>()).unwrap_or_else(|| g.clone());
                        let e = self.opaque_call(&format!("delegate:{id}"), "delegate", vec![], &sort);
                        Val { del: Del::Graph(g), ..Val::new(e, sort) }
                    }
                    None => self.unsupported(op, format!("{kind} without body")),
                }
            }
            _ => self.unsupported(op, format!("unsupported operation {kind}")),
        }
    }

    fn call(&mut self, op: &Value, sym: &str, args: Vec<Val>, sort: &str, ty: Option<&str>) -> Val {
        let static_ext = {
            let k = method_key(sym);
            let owner = k.split('.').next().unwrap_or("");
            owner.ends_with("Extensions") || owner == "Enumerable"
        };
        let null_recv = !static_ext && args.first().map(|a| a.maybe_null).unwrap_or(false) && !sym.starts_with("M:Microsoft.CodeAnalysis.Diagnostic.");
        let dict_add = sym.starts_with("M:System.Collections.Generic.Dictionary`2.Add(") || sym.starts_with("M:System.Collections.Generic.IDictionary`2.Add(");
        let result = if sort == "Unit" { "Unit".to_string() } else { sort.to_string() };
        let mut e = self.opaque_call(sym, sym, args, &result);
        if dict_add && self.contract("dictionary-duplicate-add", sym) {
            e["contract"] = json!("dictionary-duplicate-add");
            e["syntax"] = json!(s(op, "syntax"));
        } else if null_recv && self.contract("null-receiver", sym) {
            e["contract"] = json!("null-receiver");
            e["syntax"] = json!(s(op, "syntax"));
        }
        let short = method_key(sym);
        let maybe_null = ty.map(|t| t.ends_with('?')).unwrap_or(false) || short.ends_with("OrDefault");
        Val { maybe_null, ..Val::new(e, sort.to_string()) }
    }

    fn stmt(&mut self, op: &Value, env: &mut Env, frames: &mut Vec<Frame>) {
        let kind = kind_of(op);
        match kind {
            "FlowCapture" if children(op).first().is_some() || op.get("symbol").map(Value::is_null).unwrap_or(true) => {
                self.cover(op.get("location"));
                let v = match children(op).first() {
                    Some(c) => self.expr(c, env, frames),
                    None => match env.reg.clone() {
                        Some(v) => v,
                        None => self.unsupported(op, "capture without value".into()),
                    },
                };
                let v = self.bind(v, frames);
                env.captures.push((cap_key(op), v.clone()));
                env.reg = Some(v);
            }
            "SimpleAssignment" => {
                self.cover(op.get("location"));
                let c = children(op);
                let target = c.first();
                if c.len() == 2 && target.map(|t| kind_of(t) == "LocalReference").unwrap_or(false) {
                    let v = self.expr(&c[1], env, frames);
                    let v = self.bind(v, frames);
                    env.vars.insert(name_of(&c[0]), v);
                } else if let Some(k) = target
                    .filter(|t| kind_of(t) == "PropertyReference" && !s(t, "symbol").is_empty())
                    .and_then(|t| children(t).first().filter(|r| kind_of(r) == "FlowCaptureReference").map(cap_key))
                    .filter(|k| env.captures.iter().any(|(x, _)| x == k))
                {
                    // object initializer member (`new T { P = v }`) on a captured object built in this unit:
                    // a functional update `set_P(obj, v)` of that capture
                    let obj = env.captures.iter().rev().find(|(x, _)| *x == k).unwrap().1.clone();
                    let v = self.expr(&c[1], env, frames);
                    let psym = s(&c[0], "symbol").to_string();
                    let e = self.opaque_call(&format!("set:{psym}"), &format!("set{}", short_name(&psym)), vec![obj.clone(), v], &obj.sort);
                    let nv = self.bind(Val { e, ..obj }, frames);
                    env.captures.push((k, nv));
                } else {
                    self.unsupported(op, "assignment to non-local state".into());
                }
            }
            "ExpressionStatement" if children(op).first().map(|c| kind_of(c) == "SimpleAssignment").unwrap_or(false) => {
                self.cover(op.get("location"));
                self.stmt(&children(op)[0], env, frames);
            }
            "ExpressionStatement" => {
                self.cover(op.get("location"));
                if let Some(c) = children(op).first() {
                    let v = self.expr(c, env, frames);
                    // void library calls (e.g. List.Add) are effects: the differ records them as events
                    let void_call = v.sort == "Unit" && v.e["op"] == "call";
                    if is_effectful(&v.e) || void_call {
                        frames.push(Frame::Effect(v.e));
                    }
                }
            }
            "Return" => {
                self.cover(op.get("location"));
                env.reg = children(op).first().map(|c| self.expr(c, env, frames));
            }
            "Throw" => {
                self.cover(op.get("location"));
                let t = children(op).first().and_then(|c| c.get("type")).and_then(Value::as_str).unwrap_or("System.Exception");
                frames.push(Frame::Effect(json!({"op": "throw", "name": t, "location": op.get("location").cloned().unwrap_or(Value::Null)})));
            }
            _ => {
                let v = self.expr(op, env, frames);
                if kind_of(op) == "FlowCapture" {
                    env.captures.push((cap_key(op), v.clone()));
                }
                if v.sort == "Unit" || op.get("type").and_then(Value::as_str) == Some("System.Void") {
                    if is_effectful(&v.e) {
                        frames.push(Frame::Effect(v.e));
                    }
                } else {
                    env.reg = Some(v);
                }
            }
        }
    }

    /// Effectful values (contracts, inlined bodies) are bound once so they are not duplicated.
    fn bind(&mut self, v: Val, frames: &mut Vec<Frame>) -> Val {
        if !is_effectful(&v.e) {
            return v;
        }
        let n = self.fresh_name("t");
        frames.push(Frame::Let(n.clone(), v.e.clone()));
        Val { e: json!({"op": "var", "name": n, "sort": v.sort}), ..v }
    }

    fn lower_block(&mut self, g: &Value, ord: u64, mut env: Env, path: &mut Vec<u64>, value_mode: bool) -> Value {
        let blocks = g.get("blocks").and_then(Value::as_array).map(|v| v.as_slice()).unwrap_or(&[]);
        let top = self.stack.len() == 1;
        // Leaving a region unit ends it.
        if top {
            if let Some(r) = &self.region {
                if !r.contains(&ord) {
                    let Some(c) = self.closure.clone() else { return nothing() };
                    // the values the region hands on (compared by the differ, no event of their own)
                    let mut end = nothing();
                    if !c.carried.is_empty() {
                        let vals: Vec<Value> = c.carried.iter().map(|(n, so)| match env.vars.get(n) {
                            Some(v) => v.e.clone(),
                            None => self.free_var(n, so).e,
                        }).collect();
                        end["carried"] = json!(vals);
                        end["carriedNames"] = json!(c.carried.iter().map(|x| x.0.clone()).collect::<Vec<_>>());
                    }
                    return match c.tail {
                        Some(t) if t != ord => {
                            // the iteration ends here although its unchanged tail exists: an observable effect
                            let key = if ord == c.header { "region:skip" } else { "region:leave" };
                            let e = self.opaque_call(key, key, vec![], "Unit");
                            json!({"op": "seq", "args": [e, end]})
                        }
                        _ => end,
                    };
                }
            }
        }
        let Some(b) = blocks.iter().find(|b| b.get("ordinal").and_then(Value::as_u64) == Some(ord)) else {
            self.not_modelled.push(json!({"location": g.get("location").cloned().unwrap_or(Value::Null), "reason": format!("missing block {ord}")}));
            return nothing();
        };
        if path.contains(&ord) {
            self.not_modelled.push(json!({"location": g.get("location").cloned().unwrap_or(Value::Null), "reason": "loop"}));
            return nothing();
        }
        if s(b, "kind") == "exit" {
            // A unit's value is its body's result (the differ reads `value` as Nullable.Value).
            return match env.reg.take() {
                Some(v) if value_mode || v.sort != "Unit" => v.e,
                _ => nothing(),
            };
        }
        path.push(ord);
        let mut frames = Vec::new();
        for op in b.get("operations").and_then(Value::as_array).into_iter().flatten() {
            self.stmt(op, &mut env, &mut frames);
        }
        let branch = b.get("branch").filter(|v| v.is_object());
        let ft = b.get("fallThrough").and_then(Value::as_u64);
        let cond = b.get("conditional").and_then(Value::as_u64);
        let tail = if b.get("throws").and_then(Value::as_bool) == Some(true) {
            // the thrown type: the worker's `thrownType` on the block, else the type of the thrown operation
            let t = b.get("thrownType").and_then(Value::as_str).or(branch.and_then(|x| x.get("type")).and_then(Value::as_str)).unwrap_or("System.Exception");
            if let Some(x) = branch {
                self.cover(x.get("location"));
            }
            json!({"op": "throw", "name": t, "location": branch.and_then(|x| x.get("location")).cloned().unwrap_or(Value::Null)})
        } else if let (Some(c), Some(br)) = (cond, branch) {
            let cv = self.expr(br, &mut env, &mut frames);
            let cv = self.bind(cv, &mut frames);
            let jt = b.get("jumpIfTrue").and_then(Value::as_bool).unwrap_or(true);
            // jumpIfTrue: condition true goes to `conditional`; otherwise condition false does.
            let (on_true, on_false) = if jt { (c, ft) } else { (ft.unwrap_or(c), Some(c)) };
            // In the successor where an IsNull test is false, the tested capture is no longer maybe-null.
            let tested = (kind_of(br) == "IsNull").then(|| children(br).first().map(cap_key).unwrap_or_else(|| s(br, "syntax").to_string()));
            let env_t = env.clone();
            let mut env_f = env.clone();
            if let Some(t) = &tested {
                for (k, v) in env_f.captures.iter_mut() {
                    if k == t || *k == s(br, "syntax") {
                        v.maybe_null = false;
                    }
                }
            }
            let t = self.lower_block(g, on_true, env_t, path, value_mode);
            let f = match on_false {
                Some(x) => self.lower_block(g, x, env_f, path, value_mode),
                None => nothing(),
            };
            // `syntax` is the source text of the test (for readable conditions); `nullTest`: it reads "<syntax> is null"
            json!({"op": "if", "args": [cv.e, t, f], "syntax": s(br, "syntax"), "nullTest": kind_of(br) == "IsNull",
                "line": br.get("location").and_then(line_span).map(|x| x.0)})
        } else {
            if let Some(br) = branch {
                let v = self.expr(br, &mut env, &mut frames);
                env.reg = Some(v);
            }
            match ft {
                Some(x) => self.lower_block(g, x, env, path, value_mode),
                None => nothing(),
            }
        };
        path.pop();
        let mut e = tail;
        for f in frames.into_iter().rev() {
            e = match f {
                Frame::Effect(x) => json!({"op": "seq", "args": [x, e]}),
                Frame::Let(n, x) => json!({"op": "let", "name": n, "args": [x, e]}),
            };
        }
        e
    }
}

/// `(T)x` or `((T)x)`: a parenthesised type name followed directly by an operand.
fn is_cast_syntax(t: &str) -> bool {
    let r = t.trim_start_matches('(');
    if r.len() == t.len() {
        return false;
    }
    let Some(close) = r.find(')') else { return false };
    let ty = &r[..close];
    let rest = &r[close + 1..];
    !ty.is_empty()
        && ty.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_').unwrap_or(false)
        && ty.chars().all(|c| c.is_ascii_alphanumeric() || "._<>?,[] ".contains(c))
        && !ty.contains(" as ")
        && rest.chars().next().map(|c| c.is_ascii_alphanumeric() || c == '_' || c == '(' || c == '@').unwrap_or(false)
}

/// The operator at nesting depth 0 of a binary expression's syntax.
fn top_level_operator(syn: &str) -> Option<String> {
    let b = syn.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    let ops = ["&&", "||", "==", "!=", "<=", ">=", "??", "<", ">", "+", "-", "*", "/", "%", "&", "|", "^"];
    let mut in_str = false;
    while i < b.len() {
        let c = b[i] as char;
        if c == '"' {
            in_str = !in_str;
        }
        if in_str {
            i += 1;
            continue;
        }
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ if depth == 0 => {
                for o in ops {
                    if syn[i..].starts_with(o) && i > 0 && b[i - 1] == b' ' {
                        return Some(o.to_string());
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn sort_params(g: &Value) -> (Vec<Value>, Env) {
    let mut env = Env::default();
    let mut ps = Vec::new();
    for p in g.get("parameters").and_then(Value::as_array).into_iter().flatten() {
        let n = s(p, "name").to_string();
        let ty = p.get("type").and_then(Value::as_str);
        let so = sort_of(ty);
        ps.push(json!({"name": n, "sort": so}));
        let mut v = Val::new(json!({"op": "var", "name": n, "sort": so}), so);
        if ty.map(|t| t.starts_with(ACTION_DIAG)).unwrap_or(false) {
            v.del = Del::Sink;
        }
        v.maybe_null = ty.map(|t| t.ends_with('?')).unwrap_or(false);
        env.vars.insert(n, v);
    }
    (ps, env)
}

/// Spans of all located operations of one block (statement-sized spans only).
fn block_spans(b: &Value) -> Vec<(u64, u64)> {
    let mut v = Vec::new();
    for o in b.get("operations").and_then(Value::as_array).into_iter().flatten().chain(b.get("branch").filter(|x| x.is_object())) {
        walk_ops(o, &mut |x| {
            if let Some(sp) = x.get("location").and_then(line_span) {
                if sp.1 - sp.0 <= 20 {
                    v.push(sp);
                }
            }
        });
    }
    v
}

fn block_reads_local(b: &Value, names: &BTreeSet<String>) -> bool {
    let mut hit = false;
    for o in b.get("operations").and_then(Value::as_array).into_iter().flatten().chain(b.get("branch").filter(|x| x.is_object())) {
        walk_ops(o, &mut |x| {
            if kind_of(x) == "LocalReference" && names.contains(&name_of(x)) {
                hit = true;
            }
        });
    }
    hit
}

/// Blocks of the region around `lines`: blocks touching the lines, widened to whole statements, plus the blocks
/// between them and later readers of locals the region assigns (2283 :1077 -> the `Invoke` at :1080).
pub fn region_blocks(g: &Value, lines: &[u64]) -> BTreeSet<u64> {
    region_blocks_opt(g, lines, true)
}

/// `back`: also widen backwards over the enclosing statement (`extend_back`).
pub fn region_blocks_opt(g: &Value, lines: &[u64], back: bool) -> BTreeSet<u64> {
    let blocks: Vec<&Value> = g.get("blocks").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let ord = |b: &Value| b.get("ordinal").and_then(Value::as_u64).unwrap_or(0);
    let (mut lo, mut hi) = (*lines.iter().min().unwrap_or(&0), *lines.iter().max().unwrap_or(&0));
    let mut r: BTreeSet<u64> = BTreeSet::new();
    loop {
        let before = (lo, hi, r.len());
        for b in &blocks {
            let sp = block_spans(b);
            if sp.iter().any(|(a, e)| *a <= hi && lo <= *e) {
                r.insert(ord(b));
                for (a, e) in sp {
                    lo = lo.min(a);
                    hi = hi.max(e);
                }
            }
        }
        if before == (lo, hi, r.len()) {
            break;
        }
    }
    // locals assigned in the region
    let mut assigned = BTreeSet::new();
    for b in blocks.iter().filter(|b| r.contains(&ord(b))) {
        for o in b.get("operations").and_then(Value::as_array).into_iter().flatten() {
            walk_ops(o, &mut |x| {
                if kind_of(x) == "SimpleAssignment" {
                    if let Some(t) = children(x).first().filter(|t| kind_of(t) == "LocalReference") {
                        assigned.insert(name_of(t));
                    }
                }
            });
        }
    }
    let (Some(&first), Some(&last)) = (r.iter().next(), r.iter().next_back()) else { return r };
    let readers: Vec<u64> = blocks.iter().filter(|b| ord(b) > last && block_reads_local(b, &assigned)).map(|b| ord(b)).collect();
    if let Some(&until) = readers.iter().max() {
        // forward from the region, within ordinals [first, until], keeping blocks that reach a reader
        let succ = |o: u64| -> Vec<u64> {
            blocks.iter().find(|b| ord(b) == o).map(|b| {
                [b.get("fallThrough"), b.get("conditional")].into_iter().flatten().filter_map(Value::as_u64).filter(|x| *x > o && *x <= until).collect()
            }).unwrap_or_default()
        };
        let mut reach: BTreeSet<u64> = BTreeSet::new();
        let mut todo: Vec<u64> = r.iter().copied().collect();
        while let Some(o) = todo.pop() {
            for n in succ(o) {
                if !r.contains(&n) && reach.insert(n) {
                    todo.push(n);
                }
            }
        }
        // keep those that can reach a reader (ordinals only grow along kept edges)
        let mut keep: BTreeSet<u64> = readers.iter().copied().filter(|x| reach.contains(x)).collect();
        for o in reach.iter().rev() {
            if succ(*o).iter().any(|n| keep.contains(n)) {
                keep.insert(*o);
            }
        }
        let _ = first;
        r.extend(keep);
    }
    if back {
        extend_back(&blocks, &mut r);
    }
    r
}

/// Widen the region backwards over the enclosing statement: scan down from the region's first block while every
/// successor of the scanned block stays inside the region, the scanned range, or the region's exits. This takes in
/// the conditions and reports that lead to the site on the same path (2234: `ContainsKey` → MsgPack005 → `Add`),
/// and stops at the first branch that leaves (the enclosing `if`'s other arm, the loop head).
fn extend_back(blocks: &[&Value], r: &mut BTreeSet<u64>) {
    let ord = |b: &Value| b.get("ordinal").and_then(Value::as_u64).unwrap_or(0);
    let get = |o: u64| blocks.iter().find(|b| ord(b) == o).copied();
    let succ = |b: &Value| -> Vec<u64> { [b.get("fallThrough"), b.get("conditional")].into_iter().flatten().filter_map(Value::as_u64).collect() };
    let Some(&min) = r.iter().next() else { return };
    let out: BTreeSet<u64> = r.iter().filter_map(|o| get(*o)).flat_map(succ).filter(|s| !r.contains(s)).collect();
    let mut lowest = None;
    let mut c = min;
    while c > 0 && min - c < 40 {
        c -= 1;
        let Some(b) = get(c) else { break };
        if s(b, "kind") != "block" {
            break;
        }
        // every arm must lead into the region: an arm that bypasses it belongs to an enclosing statement
        let reaches = |start: u64| -> bool {
            let mut seen = BTreeSet::new();
            let mut todo = vec![start];
            while let Some(o) = todo.pop() {
                if r.contains(&o) {
                    return true;
                }
                if o <= c || o >= min || !seen.insert(o) {
                    continue;
                }
                if let Some(x) = get(o) {
                    todo.extend(succ(x));
                }
            }
            false
        };
        let ok = succ(b).iter().all(|s| reaches(*s));
        let _ = &out;
        if !ok {
            break;
        }
        if b.get("conditional").and_then(Value::as_u64).is_some() {
            lowest = Some(c);
        }
    }
    let Some(lo) = lowest else { return };
    // blocks reachable from `lo` inside [lo, min)
    let mut todo = vec![lo];
    let mut add = BTreeSet::new();
    while let Some(o) = todo.pop() {
        if o >= min || !add.insert(o) {
            continue;
        }
        if let Some(b) = get(o) {
            todo.extend(succ(b).into_iter().filter(|s| *s >= lo && *s < min));
        }
    }
    r.extend(add);
}

/// The innermost `foreach` around a region: header block with a `MoveNext` branch before the region whose exit
/// edge lies after it, the element local assigned from `Current` in the next block, and the collection (the
/// receiver of `GetEnumerator` in the capture the header tests).
pub struct Foreach {
    pub header: u64,
    pub element: String,
    pub sort: String,
    pub collection: Value,
    pub location: Value,
}

pub fn enclosing_foreach(g: &Value, r: &BTreeSet<u64>) -> Option<Foreach> {
    let blocks: Vec<&Value> = g.get("blocks").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let ord = |b: &Value| b.get("ordinal").and_then(Value::as_u64).unwrap_or(0);
    let (min, max) = (*r.iter().next()?, *r.iter().next_back()?);
    let h = blocks.iter().filter(|b| {
        let br = &b["branch"];
        br.is_object() && s(br, "symbol").ends_with(".MoveNext") && ord(b) < min && b["conditional"].as_u64().map(|c| c > max).unwrap_or(false)
    }).max_by_key(|b| ord(b))?;
    let cap = children(&h["branch"]).first().map(cap_key)?;
    let next = blocks.iter().find(|b| Some(ord(b)) == h["fallThrough"].as_u64())?;
    let asg = next["operations"].as_array()?.iter().find(|o| kind_of(o) == "SimpleAssignment")?;
    let target = children(asg).first().filter(|t| kind_of(t) == "LocalReference")?;
    let mut collection = None;
    for b in &blocks {
        for o in b["operations"].as_array().into_iter().flatten() {
            if kind_of(o) == "FlowCapture" && cap_key(o) == cap {
                // capture = GetEnumerator(collection)
                let ge = children(o).first()?;
                collection = Some(children(ge).first().cloned().unwrap_or_else(|| ge.clone()));
            }
        }
    }
    Some(Foreach {
        header: ord(h),
        element: name_of(target),
        sort: sort_of(target.get("type").and_then(Value::as_str)),
        collection: collection?,
        location: h["branch"]["location"].clone(),
    })
}

/// Names of locals and parameters that some operation of the graph tests against null.
fn null_tested(g: &Value) -> BTreeSet<String> {
    // worker `maybeNullLocals`: reference-type locals some path assigns `default`/`null`
    let mut out: BTreeSet<String> = g["maybeNullLocals"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect();
    graph_ops(g, &mut |op| {
        let k = kind_of(op);
        let test = k == "IsNull" || (matches!(k, "Binary" | "BinaryOperator" | "IsPattern") && s(op, "syntax").contains("null"));
        if test {
            walk_ops(op, &mut |x| {
                if matches!(kind_of(x), "LocalReference" | "ParameterReference") {
                    out.insert(name_of(x));
                }
            });
        }
    });
    out
}

/// What a closed per-iteration region needs at its exits.
#[derive(Clone, Debug, Default)]
pub struct Closure {
    /// The unchanged tail of the iteration: the one block outside the region where its paths join again.
    pub tail: Option<u64>,
    /// The loop header: reaching it while a tail exists means the rest of the iteration was skipped.
    pub header: u64,
    /// Locals the region assigns that are read after it or by the next iteration: (name, sort).
    pub carried: Vec<(String, String)>,
}

fn succ_of(b: &Value) -> Vec<u64> {
    [b.get("fallThrough"), b.get("conditional")].into_iter().flatten().filter_map(Value::as_u64).collect()
}

/// (assigns `n`, reads `n` before assigning it) for one block, operations in order.
fn local_use(b: &Value, n: &str) -> (bool, bool) {
    fn go(op: &Value, n: &str, assigned: &mut bool, read: &mut bool) {
        let k = kind_of(op);
        let c = children(op);
        let target = c.first().filter(|t| kind_of(t) == "LocalReference" && name_of(t) == n).is_some();
        if target && matches!(k, "SimpleAssignment" | "Increment" | "Decrement" | "CompoundAssignment") {
            if k != "SimpleAssignment" && !*assigned {
                *read = true;
            }
            for x in c.iter().skip(1) {
                go(x, n, assigned, read);
            }
            *assigned = true;
            return;
        }
        if k == "LocalReference" && name_of(op) == n && !*assigned {
            *read = true;
        }
        for x in c {
            go(x, n, assigned, read);
        }
    }
    let (mut a, mut r) = (false, false);
    for o in b.get("operations").and_then(Value::as_array).into_iter().flatten().chain(b.get("branch").filter(|x| x.is_object())) {
        go(o, n, &mut a, &mut r);
    }
    (a, r)
}

/// Close a per-iteration region (D19) so that old and new can be compared on it:
/// one entry (blocks entered from the side pull in their predecessors, up to the block that decides between
/// them) and one way on (when its paths leave to different blocks, e.g. a `continue` beside a fall-through,
/// the earlier targets join the region until one block is left: the unchanged tail of the iteration, which
/// stays outside as code both versions share). The loop header and blocks outside the loop end the iteration.
pub fn close_region(g: &Value, r: &mut BTreeSet<u64>, lp: &Foreach) -> Closure {
    let blocks: Vec<&Value> = g.get("blocks").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let ord = |b: &Value| b.get("ordinal").and_then(Value::as_u64).unwrap_or(0);
    let get = |o: u64| blocks.iter().find(|b| ord(b) == o).copied();
    let loop_exit = get(lp.header).and_then(|h| h["conditional"].as_u64()).unwrap_or(u64::MAX);
    // the block after the header binds the element: the iteration's start, never part of a region
    let start = get(lp.header).and_then(|h| h["fallThrough"].as_u64()).unwrap_or(lp.header);
    let is_term = |o: u64| o <= start || o >= loop_exit || get(o).map(|b| s(b, "kind") != "block").unwrap_or(true);
    let single_entry = |r: &mut BTreeSet<u64>| loop {
        let Some(&first) = r.iter().next() else { break };
        let add: Vec<u64> = blocks.iter().filter(|p| !r.contains(&ord(p)) && !is_term(ord(p)))
            .filter(|p| succ_of(p).iter().any(|x| *x != first && r.contains(x))).map(|p| ord(p)).collect();
        if add.is_empty() {
            break;
        }
        r.extend(add);
    };
    single_entry(r);
    let tail = loop {
        let exits: BTreeSet<u64> = r.iter().filter_map(|o| get(*o)).flat_map(succ_of).filter(|x| !r.contains(x) && !is_term(*x)).collect();
        if exits.len() <= 1 {
            break exits.into_iter().next();
        }
        r.insert(*exits.iter().next().unwrap());
        single_entry(r);
    };
    // locals assigned in the region that are live when it is left
    let mut assigned: BTreeMap<String, String> = BTreeMap::new();
    for b in blocks.iter().filter(|b| r.contains(&ord(b))) {
        for o in b.get("operations").and_then(Value::as_array).into_iter().flatten().chain(b.get("branch").filter(|x| x.is_object())) {
            walk_ops(o, &mut |x| {
                if matches!(kind_of(x), "SimpleAssignment" | "Increment" | "Decrement" | "CompoundAssignment") {
                    if let Some(t) = children(x).first().filter(|t| kind_of(t) == "LocalReference") {
                        assigned.insert(name_of(t), sort_of(t.get("type").and_then(Value::as_str)));
                    }
                }
            });
        }
    }
    let exits: Vec<u64> = r.iter().filter_map(|o| get(*o)).flat_map(succ_of).filter(|x| !r.contains(x)).collect();
    let mut carried = vec![];
    for (n, so) in &assigned {
        let (mut seen, mut todo, mut live) = (BTreeSet::new(), exits.clone(), false);
        while let Some(o) = todo.pop() {
            if live || !seen.insert(o) {
                continue;
            }
            let Some(b) = get(o) else { continue };
            let (a, rd) = local_use(b, n);
            live |= rd;
            if !a {
                todo.extend(succ_of(b));
            }
        }
        if live {
            carried.push((n.clone(), so.clone()));
        }
    }
    Closure { tail, header: lp.header, carried }
}

/// Lower the unit `opts.unit` of a cfg worker reply to a model record (`model.schema.json`).
pub fn lower(reply: &Value, opts: &Options) -> Result<Value, Error> {
    let cfg = cfg_result(reply)?;
    let methods = cfg["methods"].as_array().unwrap();
    let mut graphs = HashMap::new();
    collect_graphs(methods, &mut graphs);
    let g = *graphs.get(&opts.unit).or_else(|| graphs.get(&method_key(&opts.unit))).ok_or_else(|| Error(format!("no graph {}", opts.unit)))?;
    let inline: BTreeSet<String> = opts.inline.iter().map(|x| method_key(x)).chain(opts.inline.iter().cloned()).collect();
    let mut lw = Lowerer { graphs, opts, inline, opaque: vec![], opaque_by_key: HashMap::new(), names: BTreeSet::new(), contracts: vec![], not_modelled: vec![], covered: vec![], stack: vec![s(g, "name").to_string()], fresh: 0, seen: BTreeSet::new(), site: String::new(), free: BTreeMap::new(), region: None, closure: None, inlined: BTreeSet::new(), nullable: null_tested(g) };
    let (mut params, mut env) = sort_params(g);
    if !opts.region.is_empty() {
        env = Env::default();
    }
    let mut lp = None;
    let entry = if opts.region.is_empty() {
        lw.cover(g.get("location"));
        0
    } else {
        let mut r = region_blocks_opt(g, &opts.region, !opts.exact);
        if r.is_empty() {
            return Err(Error(format!("no block of {} on lines {:?}", opts.unit, opts.region)));
        }
        if opts.per_iteration {
            let l = enclosing_foreach(g, &r).ok_or_else(|| Error(format!("no foreach encloses lines {:?}", opts.region)))?;
            lw.closure = Some(close_region(g, &mut r, &l));
            lp = Some(l);
        }
        let first = *r.iter().next().unwrap();
        lw.region = Some(r);
        first
    };
    let mut env = env;
    if let Some(l) = &lp {
        // the element is the iteration's input; everything else entering the body (the dictionaries the loop
        // fills, counters, earlier statements of the body) is a free input: the state before this iteration
        env.vars.insert(l.element.clone(), Val::new(json!({"op": "var", "name": l.element, "sort": l.sort}), l.sort.clone()));
    }
    let mut body = lw.lower_block(g, entry, env, &mut Vec::new(), false);
    if let Some(l) = &lp {
        let mut frames = Vec::new();
        let mut cenv = Env::default();
        let coll = lw.expr(&l.collection, &mut cenv, &mut frames);
        let mut e = json!({"op": "foreach", "name": l.element, "sort": l.sort, "args": [coll.e, body], "location": l.location});
        for f in frames.into_iter().rev() {
            e = match f {
                Frame::Effect(x) => json!({"op": "seq", "args": [x, e]}),
                Frame::Let(n, x) => json!({"op": "let", "name": n, "args": [x, e]}),
            };
        }
        body = e;
    }
    if !opts.region.is_empty() {
        // a region unit's inputs are the values entering it, not the method's parameters
        params = lw.free.iter().map(|(n, so)| json!({"name": n, "sort": so})).collect();
    } else {
        for (n, so) in &lw.free {
            params.push(json!({"name": n, "sort": so}));
        }
    }
    if count_nodes(&body) > MAX_NODES {
        lw.not_modelled.push(json!({"location": g.get("location").cloned().unwrap_or(Value::Null), "reason": "path explosion"}));
        body = nothing();
    }
    // Errors from the worker's compilation make the whole unit not modelled (X1).
    // Two kinds of entry are not compile errors: a requested method the file does not have (it matters only when
    // it is this unit, and then there is no graph), and one operation Roslyn could not classify, which makes its
    // own line not modelled when a lowered graph holds it.
    let mut compile_errors = 0;
    for e in cfg.get("errors").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        if e.ends_with(" not found") && e.contains(": method ") {
            continue;
        }
        let at = e.strip_prefix("unrecognised operation ").and_then(|r| r.split_once(" at ")).map(|x| x.1)
            .and_then(|r| { let mut it = r.splitn(3, ':'); Some((it.next()?.to_string(), it.next()?.parse::<u64>().ok()?)) });
        match at {
            Some((file, line)) => {
                // an untyped `throw` expression is modelled when the worker typed its block (`thrownType`)
                let typed_throw = e.contains(&format!(":{line}: throw ")) && lw.inlined.iter().filter_map(|n| lw.graphs.get(n)).chain(std::iter::once(&g))
                    .filter(|x| s(&x["location"], "file") == file)
                    .flat_map(|x| x["blocks"].as_array().into_iter().flatten())
                    .any(|b| b.get("thrownType").is_some() && b["branch"].get("location").and_then(line_span).is_some_and(|(a, z)| a <= line && line <= z));
                if typed_throw {
                    continue;
                }
                let lowered =lw.inlined.iter().filter_map(|n| lw.graphs.get(n)).chain(std::iter::once(&g))
                    .any(|x| s(&x["location"], "file") == file && x.get("location").and_then(line_span).is_some_and(|(a, b)| a <= line && line <= b));
                if lowered {
                    lw.not_modelled.push(json!({"location": {"file": file, "line": line, "endLine": line}, "reason": "unrecognised operation"}));
                }
            }
            None => compile_errors += 1,
        }
    }
    if compile_errors > 0 {
        lw.not_modelled.push(json!({"location": g.get("location").cloned().unwrap_or(Value::Null), "reason": format!("{compile_errors} compile errors")}));
    }
    let mut nm: Vec<(u64, u64)> = lw.not_modelled.iter().filter_map(|n| n.get("location").and_then(line_span)).collect();
    // the line range of a region unit: a changed line inside it with no operation of its own (`continue;`, a
    // brace) is an edge of the lowered graph
    let range = lw.region.as_ref().map(|r| {
        let sp: Vec<(u64, u64)> = g["blocks"].as_array().into_iter().flatten()
            .filter(|b| r.contains(&b["ordinal"].as_u64().unwrap_or(u64::MAX))).flat_map(block_spans).collect();
        (sp.iter().map(|x| x.0).min().unwrap_or(1), sp.iter().map(|x| x.1).max().unwrap_or(0))
    });
    // lambdas of the unit that were not inlined are opaque values: their bodies are not compared
    let mut lambdas: Vec<&Value> = g["lambdas"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let mut skipped_lambdas = vec![];
    while let Some(l) = lambdas.pop() {
        lambdas.extend(l["lambdas"].as_array().into_iter().flatten());
        if !lw.inlined.contains(s(l, "name")) {
            if let Some(sp) = l.get("location").and_then(line_span) {
                skipped_lambdas.push((sp, l["location"].clone()));
            }
        }
    }
    let mut modelled = BTreeSet::new();
    for part in &opts.parts {
        let hit = |sp: &[(u64, u64)]| part.iter().any(|l| sp.iter().any(|(a, b)| a <= l && l <= b));
        let inside = range.map(|(a, b)| part.iter().all(|l| a <= *l && *l <= b)).unwrap_or(false);
        if !(hit(&lw.covered) || inside) {
            continue;
        }
        if hit(&nm) {
            // one not-modelled operation makes its whole change not modelled: say so for the part's other lines
            let file = s(&g["location"], "file");
            for l in part.iter().filter(|l| !nm.iter().any(|(a, b)| a <= *l && *l <= b)) {
                lw.not_modelled.push(json!({"location": {"file": file, "line": l, "endLine": l}, "reason": "in a change with a not-modelled operation"}));
            }
            continue;
        }
        // a multi-line lambda holding all of the part's lines: the part is inside that lambda's body
        match skipped_lambdas.iter().find(|((a, b), _)| b > a && part.iter().all(|l| a <= l && l <= b)) {
            Some((sp, loc)) => {
                lw.not_modelled.push(json!({"location": loc, "reason": "lambda body not compared"}));
                nm.push(*sp);
            }
            None => modelled.extend(part.iter().copied()),
        }
    }
    if opts.parts.is_empty() {
        // no parts given: the lines of lowered operations (statement-sized spans) outside not-modelled ranges
        for (a, b) in lw.covered.iter().filter(|(a, b)| b - a <= 20) {
            for l in *a..=*b {
                if !nm.iter().any(|(x, y)| *x <= l && l <= *y) {
                    modelled.insert(l);
                }
            }
        }
    }
    let not_modelled: Vec<Value> = lw.not_modelled.into_iter().filter(|n| n["location"].is_object()).collect();
    Ok(json!({
        "provenance": "pipeline",
        "unit": opts.unit_id.clone().unwrap_or_else(|| match opts.region.iter().min() {
            Some(l) => format!("{}#{l}", s(g, "name")),
            None => opts.unit.clone(),
        }),
        "version": opts.version,
        "params": params,
        "opaque": lw.opaque,
        "contracts": lw.contracts,
        "body": body,
        "lines": {"modelled": modelled.into_iter().collect::<Vec<_>>(), "notModelled": not_modelled}
    }))
}

/// A changed production line: `side` is `old` (deleted) or `new` (added).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Line {
    pub side: String,
    pub file: String,
    pub line: u64,
}

/// `scope.json` from changed-line counts, the changed production lines and the models with their unit's file.
/// Every changed production line is either modelled or inside a not-modelled entry; lines no model covers get
/// an entry with reason `outside compared units`. Assumption `rows` are left empty for the differ to fill.
pub fn scope(changed_counts: &Value, production: &[Line], models: &[(String, Value)]) -> Value {
    scope_with(changed_counts, production, models, &[])
}

/// `reasons`: why a range of changed lines (from its first line to the given last line, on that side) is not
/// compared; a line no model covers takes its reason from here, else `outside compared units`.
pub fn scope_with(changed_counts: &Value, production: &[Line], models: &[(String, Value)], reasons: &[(Line, u64, String)]) -> Value {
    let side = |v: &str| if v == "old" { "old".to_string() } else { "new".to_string() };
    let mut modelled: BTreeSet<Line> = BTreeSet::new();
    let mut nm_entries: Vec<Value> = Vec::new();
    let mut nm_spans: Vec<(String, String, u64, u64)> = Vec::new();
    let mut assumptions: Vec<Value> = Vec::new();
    for (file, m) in models {
        let sd = side(m["version"].as_str().unwrap_or("new"));
        for l in m["lines"]["modelled"].as_array().into_iter().flatten().filter_map(Value::as_u64) {
            modelled.insert(Line { side: sd.clone(), file: file.clone(), line: l });
        }
        for n in m["lines"]["notModelled"].as_array().into_iter().flatten() {
            if let Some((a, b)) = line_span(&n["location"]) {
                nm_spans.push((sd.clone(), n["location"]["file"].as_str().unwrap_or(file).to_string(), a, b));
            }
            nm_entries.push(n.clone());
        }
        for c in m["contracts"].as_array().into_iter().flatten() {
            if !assumptions.iter().any(|a| a["contract"] == c["contract"] && a["symbol"] == c["symbol"]) {
                assumptions.push(json!({"contract": c["contract"], "symbol": c["symbol"], "rows": []}));
            }
        }
    }
    let prod: BTreeSet<Line> = production.iter().cloned().collect();
    let count = prod.iter().filter(|l| modelled.contains(l)).count();
    let mut runs: Vec<(String, u64, u64, String)> = Vec::new();
    for l in prod.iter().filter(|l| !modelled.contains(l)) {
        if nm_spans.iter().any(|(sd, f, a, b)| *sd == l.side && *f == l.file && *a <= l.line && l.line <= *b) {
            continue;
        }
        let reason = reasons.iter().find(|(r, hi, _)| r.side == l.side && r.file == l.file && r.line <= l.line && l.line <= *hi)
            .map(|x| x.2.clone()).unwrap_or_else(|| "outside compared units".to_string());
        match runs.last_mut() {
            Some((f, _, e, r)) if *f == l.file && *e + 1 == l.line && *r == reason => *e = l.line,
            _ => runs.push((l.file.clone(), l.line, l.line, reason)),
        }
    }
    for (f, a, b, reason) in runs {
        nm_entries.push(json!({"location": {"file": f, "line": a, "endLine": b}, "reason": reason}));
    }
    let mut seen: Vec<Value> = Vec::new();
    nm_entries.retain(|e| if seen.contains(e) { false } else { seen.push(e.clone()); true });
    let mut out = Map::new();
    out.insert("provenance".into(), json!("pipeline"));
    out.insert("changedLines".into(), changed_counts.clone());
    out.insert("modelledLines".into(), json!(count));
    out.insert("assumptions".into(), json!(assumptions));
    out.insert("notModelled".into(), json!(nm_entries));
    Value::Object(out)
}
