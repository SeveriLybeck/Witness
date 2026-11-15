//! Concrete interpreter of the model, with the same semantics as the symbolic executor.
//! Used to check the model against real executions (D23).

use crate::model::{Expr, Model, Opaque};
use crate::{outcome_of, Outcome};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum V {
    B(bool),
    I(i64),
    S(String),
    Null,
    Obj(String),
    Arr(Vec<V>),
    Unit,
}

/// Concrete library behaviour for the opaque calls.
pub trait Oracle {
    fn call(&mut self, o: &Opaque, args: &[V]) -> V;
    fn is_type(&mut self, _ty: &str, _v: &V) -> bool {
        false
    }
    fn contains(&mut self, _dict: &V, _key: &V) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
pub struct Run {
    pub reports: Vec<(String, Vec<V>)>,
    pub calls: Vec<String>,
    pub end: Result<V, String>,
}

impl Run {
    pub fn outcome(&self) -> Outcome {
        let reports: Vec<String> = self.reports.iter().map(|r| r.0.clone()).collect();
        match &self.end {
            Err(x) => outcome_of(Some(x), &reports, &self.calls, None),
            Ok(V::Unit) => outcome_of(None, &reports, &self.calls, None),
            Ok(v) => outcome_of(None, &reports, &self.calls, Some(&format!("{v:?}"))),
        }
    }
}

struct I<'a> {
    opaque: HashMap<String, Opaque>,
    enabled: Vec<String>,
    oracle: &'a mut dyn Oracle,
    run: Run,
}

type R = Result<V, String>;

fn exc(c: &str) -> String {
    crate::exec::exception_of(c).to_string()
}

impl I<'_> {
    fn on(&self, c: &Option<String>) -> bool {
        c.as_ref().is_some_and(|c| self.enabled.contains(c))
    }

    fn eval(&mut self, e: &Expr, env: &HashMap<String, V>) -> Result<R, String> {
        let name = e.name.clone().unwrap_or_default();
        macro_rules! ev {
            ($x:expr) => {
                match self.eval($x, env)? {
                    Ok(v) => v,
                    Err(t) => return Ok(Err(t)),
                }
            };
        }
        let truthy = |v: &V| matches!(v, V::B(true));
        Ok(Ok(match e.op.as_str() {
            "lit" => match e.value.clone().unwrap_or_default() {
                serde_json::Value::Bool(b) => V::B(b),
                serde_json::Value::Number(n) => V::I(n.as_i64().ok_or("literal")?),
                serde_json::Value::String(s) => V::S(s),
                _ => V::Null,
            },
            "var" => env.get(&name).cloned().ok_or_else(|| format!("unbound {name}"))?,
            "none" => V::Null,
            "nothing" => V::Unit,
            "throw" => return Ok(Err(name)),
            "let" => {
                let v = ev!(&e.args[0]);
                let mut env2 = env.clone();
                env2.insert(name, v);
                return self.eval(&e.args[1], &env2);
            }
            "if" => {
                let c = ev!(&e.args[0]);
                return match e.args.get(if truthy(&c) { 1 } else { 2 }) {
                    Some(a) => self.eval(a, env),
                    None => Ok(Ok(V::Unit)),
                };
            }
            "and" | "or" => {
                let is_and = e.op == "and";
                for a in &e.args {
                    if truthy(&ev!(a)) != is_and {
                        return Ok(Ok(V::B(!is_and)));
                    }
                }
                V::B(is_and)
            }
            "not" => V::B(!truthy(&ev!(&e.args[0]))),
            "eq" => {
                let (a, b) = (ev!(&e.args[0]), ev!(&e.args[1]));
                V::B(a == b)
            }
            "isnull" => V::B(ev!(&e.args[0]) == V::Null),
            "some" => ev!(&e.args[0]),
            "index" => {
                let (a, i) = (ev!(&e.args[0]), ev!(&e.args[1]));
                let c = e.contract.clone().or(Some("index-empty".into()));
                match (a, i) {
                    (V::Arr(xs), V::I(i)) if i >= 0 && (i as usize) < xs.len() => xs[i as usize].clone(),
                    (V::Arr(_), _) if self.on(&c) => return Ok(Err(exc("index-empty"))),
                    _ => V::Null,
                }
            }
            "cast" => {
                let v = ev!(&e.args[0]);
                if self.on(&e.contract) && v != V::Null && !self.oracle.is_type(&name, &v) {
                    return Ok(Err(exc("failed-cast")));
                }
                v
            }
            "value" => {
                let v = ev!(&e.args[0]);
                if self.on(&e.contract) && v == V::Null {
                    return Ok(Err(exc("nullable-value-null")));
                }
                v
            }
            "report" => {
                let mut vs = vec![];
                for a in &e.args {
                    vs.push(ev!(a));
                }
                self.run.reports.push((name, vs));
                V::Unit
            }
            "seq" => {
                let mut last = V::Unit;
                for a in &e.args {
                    last = ev!(a);
                }
                last
            }
            "foreach" => {
                let coll = ev!(&e.args[0]);
                let items = match coll {
                    V::Arr(xs) => xs,
                    _ => vec![],
                };
                for x in items {
                    let mut env2 = env.clone();
                    env2.insert(name.clone(), x);
                    ev!(&e.args[1]);
                }
                V::Unit
            }
            "call" => {
                let mut vs = vec![];
                for a in &e.args {
                    vs.push(ev!(a));
                }
                let o = self.opaque.get(&name).cloned().ok_or_else(|| format!("undeclared opaque {name}"))?;
                if self.on(&e.contract) {
                    match e.contract.as_deref() {
                        Some("null-receiver") if vs.first() == Some(&V::Null) => return Ok(Err(exc("null-receiver"))),
                        Some("dictionary-duplicate-add") if vs.len() >= 2 && self.oracle.contains(&vs[0], &vs[1]) => {
                            return Ok(Err(exc("dictionary-duplicate-add")))
                        }
                        _ => {}
                    }
                }
                if o.symbol.starts_with("T:") && vs.len() == 1 && o.result == "Bool" {
                    let ty = o.symbol.rsplit('.').next().unwrap_or(&o.symbol).trim_start_matches("T:").to_string();
                    V::B(self.oracle.is_type(&ty, &vs[0]))
                } else if o.symbol.contains(".ContainsKey(") && vs.len() == 2 {
                    V::B(self.oracle.contains(&vs[0], &vs[1]))
                } else if matches!(o.result.as_str(), "Void" | "Unit") {
                    self.run.calls.push(crate::exec::display(&o));
                    V::Unit
                } else {
                    self.oracle.call(&o, &vs)
                }
            }
            other => return Err(format!("unsupported op {other}")),
        }))
    }
}

/// Run `m` on concrete inputs.
pub fn run(m: &Model, inputs: &HashMap<String, V>, oracle: &mut dyn Oracle, disabled: &[String]) -> Result<Run, String> {
    let mut i = I {
        opaque: m.opaque.iter().map(|o| (o.name.clone(), o.clone())).collect(),
        enabled: m.contracts.iter().map(|c| c.0.clone()).filter(|c| !disabled.contains(c)).collect(),
        oracle,
        run: Run { reports: vec![], calls: vec![], end: Ok(V::Unit) },
    };
    let end = i.eval(&m.body, inputs)?;
    i.run.end = end;
    Ok(i.run)
}
