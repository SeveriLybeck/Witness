//! The language-neutral model (contracts/messages/model.schema.json), parsed from JSON.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Loc {
    pub file: String,
    pub line: u64,
    pub end_line: u64,
}

impl Loc {
    pub fn to_json(&self) -> Value {
        serde_json::json!({"file": self.file, "line": self.line, "endLine": self.end_line})
    }
    fn parse(v: &Value) -> Result<Loc, String> {
        Ok(Loc {
            file: v["file"].as_str().ok_or("location.file")?.to_string(),
            line: v["line"].as_u64().ok_or("location.line")?,
            end_line: v["endLine"].as_u64().ok_or("location.endLine")?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub op: String,
    pub name: Option<String>,
    pub value: Option<Value>,
    pub sort: Option<String>,
    pub contract: Option<String>,
    pub args: Vec<Expr>,
    pub location: Option<Loc>,
    /// Source text of an `if` test or of an operation under a contract (readable conditions).
    pub syntax: Option<String>,
    /// The `if` test reads "<syntax> is null".
    pub null_test: bool,
    /// On `nothing` at the end of a region: the values it hands on, compared between versions.
    pub carried: Vec<Expr>,
    /// Source line of an `if` test.
    pub line: Option<u64>,
}

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
    /// (contract, symbol)
    pub contracts: Vec<(String, String)>,
    pub body: Expr,
    pub modelled: Vec<u64>,
    pub not_modelled: Vec<(Loc, String)>,
}

pub const OPS: &[&str] = &[
    "lit", "var", "let", "if", "call", "not", "and", "or", "eq", "isnull", "some", "none", "index", "cast", "throw",
    "report", "nothing", "value", "seq", "foreach",
];

fn s(v: &Value, k: &str) -> Result<String, String> {
    v[k].as_str().map(str::to_string).ok_or_else(|| format!("missing string {k}"))
}

impl Expr {
    pub fn parse(v: &Value) -> Result<Expr, String> {
        let op = s(v, "op")?;
        if !OPS.contains(&op.as_str()) {
            return Err(format!("unknown op {op}"));
        }
        let args = match v.get("args") {
            Some(Value::Array(a)) => a.iter().map(Expr::parse).collect::<Result<Vec<_>, _>>()?,
            _ => vec![],
        };
        Ok(Expr {
            op,
            name: v.get("name").and_then(Value::as_str).map(str::to_string),
            value: v.get("value").cloned(),
            sort: v.get("sort").and_then(Value::as_str).map(str::to_string),
            contract: v.get("contract").and_then(Value::as_str).map(str::to_string),
            syntax: v.get("syntax").and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string),
            null_test: v.get("nullTest").and_then(Value::as_bool).unwrap_or(false),
            line: v.get("line").and_then(Value::as_u64),
            carried: match v.get("carried") {
                Some(Value::Array(a)) => a.iter().map(Expr::parse).collect::<Result<Vec<_>, _>>()?,
                _ => vec![],
            },
            args,
            location: match v.get("location") {
                Some(l) if !l.is_null() => Some(Loc::parse(l)?),
                _ => None,
            },
        })
    }

    /// First location in the tree, depth first.
    pub fn first_loc(&self) -> Option<&Loc> {
        self.location.as_ref().or_else(|| self.args.iter().find_map(Expr::first_loc))
    }
}

impl Model {
    pub fn parse(v: &Value) -> Result<Model, String> {
        let arr = |k: &str| v[k].as_array().cloned().ok_or_else(|| format!("missing array {k}"));
        let params = arr("params")?.iter().map(|p| Ok((s(p, "name")?, s(p, "sort")?))).collect::<Result<_, String>>()?;
        let opaque = arr("opaque")?
            .iter()
            .map(|o| {
                Ok(Opaque {
                    name: s(o, "name")?,
                    symbol: s(o, "symbol")?,
                    args: o["args"].as_array().ok_or("opaque.args")?.iter().map(|a| a.as_str().unwrap_or("Obj").to_string()).collect(),
                    result: s(o, "result")?,
                })
            })
            .collect::<Result<_, String>>()?;
        let contracts = arr("contracts")?.iter().map(|c| Ok((s(c, "contract")?, s(c, "symbol")?))).collect::<Result<_, String>>()?;
        let lines = &v["lines"];
        let modelled = lines["modelled"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
        let not_modelled = match lines["notModelled"].as_array() {
            Some(a) => a.iter().map(|n| Ok((Loc::parse(&n["location"])?, s(n, "reason")?))).collect::<Result<_, String>>()?,
            None => vec![],
        };
        Ok(Model {
            unit: s(v, "unit")?,
            version: s(v, "version")?,
            params,
            opaque,
            contracts,
            body: Expr::parse(&v["body"])?,
            modelled,
            not_modelled,
        })
    }

    pub fn from_str(text: &str) -> Result<Model, String> {
        Model::parse(&serde_json::from_str(text).map_err(|e| e.to_string())?)
    }
}
