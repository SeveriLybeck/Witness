//! Conditions a reviewer can read: a solver literal printed in the source's terms, from the
//! opaque table's resolved symbols (`declaringSyntaxReferences_35f1 item` → `item.DeclaringSyntaxReferences`),
//! and the source text of a branch test where the model carries it (`syntax` on `if`).
//! The solver literals themselves stay in `literals`.

use crate::model::Opaque;
use std::collections::HashMap;

#[derive(Debug)]
enum Sx {
    Atom(String),
    List(Vec<Sx>),
}

fn parse(src: &str) -> Option<Sx> {
    let b: Vec<char> = src.chars().collect();
    let mut i = 0;
    let r = parse_at(&b, &mut i)?;
    Some(r)
}

fn parse_at(b: &[char], i: &mut usize) -> Option<Sx> {
    while *i < b.len() && b[*i].is_whitespace() {
        *i += 1;
    }
    if *i >= b.len() {
        return None;
    }
    match b[*i] {
        '(' => {
            *i += 1;
            let mut v = vec![];
            loop {
                while *i < b.len() && b[*i].is_whitespace() {
                    *i += 1;
                }
                if *i >= b.len() {
                    return None;
                }
                if b[*i] == ')' {
                    *i += 1;
                    return Some(Sx::List(v));
                }
                v.push(parse_at(b, i)?);
            }
        }
        q @ ('"' | '|') => {
            let start = *i;
            *i += 1;
            while *i < b.len() && b[*i] != q {
                *i += 1;
            }
            *i += 1;
            let t: String = b[start..(*i).min(b.len())].iter().collect();
            Some(Sx::Atom(if q == '|' { t.trim_matches('|').to_string() } else { t }))
        }
        _ => {
            let start = *i;
            while *i < b.len() && !b[*i].is_whitespace() && b[*i] != '(' && b[*i] != ')' {
                *i += 1;
            }
            Some(Sx::Atom(b[start..*i].iter().collect()))
        }
    }
}

/// Last segment of a documentation ID without parameters and arity marks: `P:A.B.C` → `C`.
fn member(sym: &str) -> String {
    let body = sym.split_once(':').map(|x| x.1).unwrap_or(sym);
    let head = body.split('(').next().unwrap_or(body);
    let last = head.rsplit('.').next().unwrap_or(head);
    last.split('`').next().unwrap_or(last).to_string()
}

fn owner(sym: &str) -> String {
    let body = sym.split_once(':').map(|x| x.1).unwrap_or(sym);
    let head = body.split('(').next().unwrap_or(body);
    let mut it = head.rsplit('.');
    it.next();
    let o = it.next().unwrap_or("");
    o.split('`').next().unwrap_or(o).to_string()
}

fn param_count(sym: &str) -> usize {
    match sym.split_once('(') {
        None => 0,
        Some((_, rest)) => {
            let inner = rest.trim_end_matches(')');
            let (mut depth, mut n) = (0, 1);
            for c in inner.chars() {
                match c {
                    '{' | '[' | '(' => depth += 1,
                    '}' | ']' | ')' => depth -= 1,
                    ',' if depth == 0 => n += 1,
                    _ => {}
                }
            }
            if inner.is_empty() { 0 } else { n }
        }
    }
}

fn strip_hash(name: &str) -> String {
    match name.rsplit_once('_') {
        Some((a, h)) if h.len() == 4 && h.chars().all(|c| c.is_ascii_hexdigit()) && !a.is_empty() => a.to_string(),
        _ => name.to_string(),
    }
}

pub struct Reader<'a> {
    pub opaque: &'a HashMap<String, Opaque>,
}

impl Reader<'_> {
    fn atom(&self, a: &str) -> String {
        if a == "w_null" {
            return "null".into();
        }
        if let Some(o) = self.opaque.get(a) {
            return self.app(o, a, &[]);
        }
        // values that entered a region as captures are named after their source text
        let a = a.strip_prefix("cap_").unwrap_or(a);
        a.strip_prefix("this_").map(|x| format!("this.{x}")).unwrap_or_else(|| a.to_string())
    }

    fn app(&self, o: &Opaque, name: &str, args: &[String]) -> String {
        let sym = o.symbol.as_str();
        let list = |v: &[String]| v.join(", ");
        let kind = sym.split_once(':').map(|x| x.0).unwrap_or("");
        match kind {
            "F" | "P" => match args.first() {
                Some(r) if args.len() == 1 => format!("{r}.{}", member(sym)),
                Some(r) => format!("{r}.{}[{}]", member(sym), list(&args[1..])),
                None if kind == "F" && sym.contains("Analyzer.") => member(sym),
                None => member(sym),
            },
            "M" if sym.contains(".#ctor") => format!("new {}({})", owner(sym), list(args)),
            "M" => {
                let ext = owner(sym).ends_with("Extensions") || owner(sym) == "Enumerable";
                if !args.is_empty() && (args.len() == param_count(sym) + 1 || ext) {
                    format!("{}.{}({})", args[0], member(sym), list(&args[1..]))
                } else {
                    format!("{}.{}({})", owner(sym), member(sym), list(args))
                }
            }
            "operator" => {
                let op = sym.split(':').nth(1).unwrap_or("?");
                match args {
                    [a, b] => format!("({a} {op} {b})"),
                    _ => format!("{op}({})", list(args)),
                }
            }
            "is" => {
                let pat = sym.rsplit(':').next().unwrap_or("");
                match args.first() {
                    Some(a) if sym.matches(':').count() >= 2 => format!("{a} is {pat}"),
                    _ => sym.trim_start_matches("is:").to_string(),
                }
            }
            "interpolated" => sym["interpolated:".len()..].to_string(),
            "set" => match args {
                [a, b] => format!("{a} with {} = {b}", member(sym.trim_start_matches("set:"))),
                _ => format!("{}({})", strip_hash(name), list(args)),
            },
            "Increment" => format!("{} + 1", args.first().cloned().unwrap_or_default()),
            "Decrement" => format!("{} - 1", args.first().cloned().unwrap_or_default()),
            "" if sym == "this" => "this".into(),
            _ if args.is_empty() => strip_hash(name),
            _ => format!("{}({})", strip_hash(name), list(args)),
        }
    }

    fn term(&self, x: &Sx) -> String {
        match x {
            Sx::Atom(a) => self.atom(a),
            Sx::List(v) => {
                let Some(Sx::Atom(h)) = v.first() else { return String::new() };
                let a: Vec<String> = v[1..].iter().map(|y| self.term(y)).collect();
                match (h.as_str(), a.as_slice()) {
                    ("not", [_]) => self.neg(&v[1]),
                    ("=", [x, y]) if y == "null" => format!("{x} is null"),
                    ("=", [x, y]) if x == "null" => format!("{y} is null"),
                    ("=", [x, y]) => format!("{x} == {y}"),
                    ("<=", [x, y]) if y == "0" && x.ends_with(".Length") => format!("{} is empty", x.trim_end_matches(".Length")),
                    ("<=" | "<" | ">=" | ">", [x, y]) => format!("{x} {h} {y}"),
                    ("and", _) => a.iter().map(|x| format!("({x})")).collect::<Vec<_>>().join(" and "),
                    ("or", _) => a.iter().map(|x| format!("({x})")).collect::<Vec<_>>().join(" or "),
                    ("ite", [c, x, y]) => format!("({c} ? {x} : {y})"),
                    ("-", [x]) => format!("-{x}"),
                    ("w_len", [x]) => format!("{x}.Length"),
                    ("w_at", [x, i]) => format!("{x}[{i}]"),
                    (f, [x, i]) if f.starts_with("w_at_") => format!("{x}[{i}]"),
                    (f, [x]) if f.starts_with("w_is_") => format!("{x} is {}", &f[5..]),
                    (f, [d, k]) if f.starts_with("w_contains_") => format!("{d}.ContainsKey({k})"),
                    (f, [x]) if f.starts_with("w_val_") => format!("{x}.Value"),
                    (f, _) => match self.opaque.get(f) {
                        Some(o) => self.app(o, f, &a),
                        None => format!("{}({})", strip_hash(f), a.join(", ")),
                    },
                }
            }
        }
    }

    fn neg(&self, x: &Sx) -> String {
        if let Sx::List(v) = x {
            if let Some(Sx::Atom(h)) = v.first() {
                let a: Vec<String> = v[1..].iter().map(|y| self.term(y)).collect();
                match (h.as_str(), a.as_slice()) {
                    ("not", [x]) => return x.clone(),
                    ("=", [x, y]) if y == "null" => return format!("{x} is not null"),
                    ("=", [x, y]) if x == "null" => return format!("{y} is not null"),
                    ("=", [x, y]) => return format!("{x} != {y}"),
                    ("<=", [x, y]) if y == "0" && x.ends_with(".Length") => return format!("{} is not empty", x.trim_end_matches(".Length")),
                    ("<=", [x, y]) => return format!("{x} > {y}"),
                    ("<", [x, y]) => return format!("{x} >= {y}"),
                    (f, [x]) if f.starts_with("w_is_") => return format!("{x} is not {}", &f[5..]),
                    _ => {}
                }
            }
        }
        format!("not {}", self.term(x))
    }

    /// One solver literal in the source's terms.
    pub fn literal(&self, smt: &str) -> String {
        match parse(smt) {
            Some(x) => self.term(&x),
            None => smt.to_string(),
        }
    }
}

/// The negation of a branch test given as source text.
pub fn negate_source(s: &str) -> String {
    let t = s.trim();
    let simple = !t.contains("&&") && !t.contains("||") && !t.contains('?');
    if simple {
        for (a, b) in [(" is not ", " is "), (" is ", " is not "), (" == ", " != "), (" != ", " == ")] {
            if t.matches(a).count() == 1 && (a != " is " || !t.contains(" is not ")) {
                return t.replacen(a, b, 1);
            }
        }
        if let Some(r) = t.strip_prefix('!') {
            return r.to_string();
        }
    }
    if t.chars().all(|c| c.is_alphanumeric() || "_.!()[]".contains(c)) { format!("not {t}") } else { format!("not ({t})") }
}
