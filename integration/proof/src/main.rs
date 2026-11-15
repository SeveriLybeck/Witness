//! Proof module for the bundle builder:
//! `witness-proof [--worker CMD] [--bin DIR] <stageDir> <bundleDir> <repo> <old> <new>`
//! Runs worker `cfg` -> `translator lower` -> `translator scope` -> `differ` -> `lean-certify` per compared unit
//! and writes `rows.json`, `scope.json`, `lean.json` into `<stageDir>`.
//!
//! Unit rule (D16, D18, D19), per changed C# production file:
//! - changed statements inside one `foreach` body of a method present in both versions are ONE per-iteration
//!   unit (`Type.M#<first changed line>`, `--per-iteration --exact`): old and new get the same line range, the
//!   translator closes it (one entry, the iteration's unchanged tail left outside);
//! - changed statements outside any loop: the whole method, changed callees and delegates inlined;
//! - when either fails to lower (a loop inside a whole method, no enclosing foreach): one unit per site, as before;
//! - a method only the new version has, not inlined into a compared unit: a new-only unit (`differ --new-only`),
//!   whose throwing paths are rows with `property: no-exception`.
//! Every changed production line is modelled, inside a not-modelled range of a model, or listed with the reason
//! it is not compared (`reasons` of `translator scope`).
//! Checkouts: `$WITNESS_WT_OLD` / `$WITNESS_WT_NEW` (the worker reads them). Worker: `--worker` or `$WITNESS_WORKER`.
//! Tools: `--bin` (default: this program's folder, then `$WITNESS_BIN`) holds `translator`, `differ`, `lean-certify`.

use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn die(m: &str) -> ! {
    eprintln!("witness-proof: {m}");
    std::process::exit(1)
}

fn read_json(p: &Path) -> Value {
    let t = std::fs::read_to_string(p).unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
    serde_json::from_str(&t).unwrap_or_else(|e| die(&format!("{}: {e}", p.display())))
}

fn write_json(p: &Path, v: &Value) {
    std::fs::write(p, serde_json::to_string_pretty(v).unwrap() + "\n").unwrap_or_else(|e| die(&format!("{}: {e}", p.display())));
}

fn lines_of(v: &Value) -> Vec<u64> {
    v.as_array().into_iter().flatten().filter_map(Value::as_u64).collect()
}

fn csv(l: &[u64]) -> String {
    l.iter().map(u64::to_string).collect::<Vec<_>>().join(",")
}

/// One record read from the bundle dir, else the stage dir.
fn record(bundle: &Path, stage: &Path, name: &str) -> Value {
    for d in [bundle, stage] {
        let p = d.join(format!("{name}.json"));
        if p.exists() {
            return read_json(&p);
        }
    }
    die(&format!("{name}.json not found"))
}

fn show(repo: &str, commit: &str, file: &str) -> Option<String> {
    let o = Command::new("git").args(["-C", repo, "-c", "core.hooksPath=/dev/null", "show", &format!("{commit}:{file}")]).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// A member of one C# file, 1-based lines.
struct Member {
    ty: String,
    name: String,
    kind: &'static str,
    line: u64,
    end: u64,
}

/// What the unit rule needs from one version of a file.
#[derive(Default)]
struct Syntax {
    members: Vec<Member>,
    /// Loops: (first line, last line, is a `foreach`).
    loops: Vec<(u64, u64, bool)>,
    /// Jump statements under an `if`: (first line of the `if`, last line of the jump).
    jumps: Vec<(u64, u64)>,
}

fn syntax(text: &str) -> Syntax {
    let mut p = tree_sitter::Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).expect("grammar loads");
    let mut out = Syntax::default();
    if let Some(tree) = p.parse(text, None) {
        walk(tree.root_node(), text.as_bytes(), &mut vec![], &mut out);
    }
    out
}

fn walk(n: tree_sitter::Node, src: &[u8], types: &mut Vec<String>, out: &mut Syntax) {
    let name = |n: tree_sitter::Node| n.child_by_field_name("name").and_then(|x| x.utf8_text(src).ok()).unwrap_or("").to_string();
    let k = n.kind();
    let (a, b) = (n.start_position().row as u64 + 1, n.end_position().row as u64 + 1);
    let is_type = matches!(k, "class_declaration" | "struct_declaration" | "record_declaration" | "record_struct_declaration" | "interface_declaration");
    let mk = match k {
        "method_declaration" => Some("method"),
        "constructor_declaration" => Some("ctor"),
        "property_declaration" => Some("property"),
        _ => None,
    };
    if let (Some(kind), Some(ty)) = (mk, types.last()) {
        let nm = if kind == "ctor" { ".ctor".to_string() } else { name(n) };
        out.members.push(Member { ty: ty.clone(), name: nm, kind, line: a, end: b });
    }
    match k {
        "foreach_statement" => out.loops.push((a, b, true)),
        "for_statement" | "while_statement" | "do_statement" => out.loops.push((a, b, false)),
        "continue_statement" | "break_statement" | "return_statement" | "throw_statement" => {
            // the jump belongs to the `if` that guards it: the changed statement is that `if`
            let mut p = n.parent();
            if p.map(|x| x.kind()) == Some("block") {
                p = p.and_then(|x| x.parent());
            }
            if let Some(i) = p.filter(|x| x.kind() == "if_statement") {
                out.jumps.push((i.start_position().row as u64 + 1, b));
            }
        }
        _ => {}
    }
    if is_type {
        types.push(name(n));
    }
    let mut c = n.walk();
    for ch in n.children(&mut c) {
        walk(ch, src, types, out);
    }
    if is_type {
        types.pop();
    }
}

fn enclosing(ms: &[Member], line: u64) -> Option<&Member> {
    ms.iter().filter(|m| m.kind != "property" && m.line <= line && line <= m.end).min_by_key(|m| m.end - m.line)
}

/// The innermost loop around a line: (first line, is a `foreach`).
fn innermost_loop(sx: &Syntax, line: u64) -> Option<(u64, bool)> {
    sx.loops.iter().filter(|l| l.0 <= line && line <= l.1).min_by_key(|l| l.1 - l.0).map(|l| (l.0, l.2))
}

/// Changed lines widened to the statement they change: a jump under an `if` is that `if` from its first line.
fn widen(sx: &Syntax, lines: &[u64]) -> Vec<u64> {
    let mut out: BTreeSet<u64> = lines.iter().copied().collect();
    for l in lines {
        for (a, b) in sx.jumps.iter().filter(|(a, b)| a <= l && l <= b) {
            out.extend(*a..=*b);
        }
    }
    out.into_iter().collect()
}

/// Unchanged lines of the two versions of a file, paired: (old line, new line), ascending.
fn line_map(deleted: &BTreeSet<u64>, added: &BTreeSet<u64>, old_len: u64, new_len: u64) -> Vec<(u64, u64)> {
    let (mut o, mut n, mut out) = (1u64, 1u64, vec![]);
    while o <= old_len || n <= new_len {
        if o <= old_len && deleted.contains(&o) {
            o += 1;
        } else if n <= new_len && added.contains(&n) {
            n += 1;
        } else {
            out.push((o, n));
            o += 1;
            n += 1;
        }
    }
    out
}

/// The other side's line for a line of side `s` (0 = old); a changed line takes the unchanged line before it
/// (`up` false) or after it (`up` true).
fn other_line(map: &[(u64, u64)], s: usize, line: u64, up: bool) -> Option<u64> {
    let key = |p: &(u64, u64)| if s == 0 { p.0 } else { p.1 };
    let val = |p: &(u64, u64)| if s == 0 { p.1 } else { p.0 };
    if up {
        map.iter().find(|p| key(p) >= line).map(val)
    } else {
        map.iter().rev().find(|p| key(p) <= line).map(val)
    }
}

struct Worker {
    child: std::process::Child,
    out: BufReader<std::process::ChildStdout>,
    id: u64,
}

impl Worker {
    fn start(cmd: &str) -> Worker {
        let mut child = Command::new("sh").args(["-c", cmd]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn()
            .unwrap_or_else(|e| die(&format!("worker: {e}")));
        let out = BufReader::new(child.stdout.take().unwrap());
        Worker { child, out, id: 0 }
    }
    /// None when the worker died or replied with something unreadable (its units are then not compared).
    fn call(&mut self, method: &str, params: Value) -> Option<Value> {
        self.id += 1;
        let line = json!({"id": self.id, "method": method, "params": params}).to_string() + "\n";
        self.child.stdin.as_mut()?.write_all(line.as_bytes()).ok()?;
        let mut s = String::new();
        self.out.read_line(&mut s).ok()?;
        serde_json::from_str(&s).ok()
    }
}

fn run(bin: &Path, tool: &str, args: &[String], stdout_to: Option<&Path>) -> Result<(), String> {
    let exe = bin.join(tool);
    let mut c = Command::new(&exe);
    c.args(args).stderr(Stdio::piped());
    let o = c.output().map_err(|e| format!("{}: {e}", exe.display()))?;
    if !o.status.success() {
        return Err(format!("{tool}: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("failed")));
    }
    if let Some(p) = stdout_to {
        std::fs::write(p, &o.stdout).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// One iteration of a `foreach` body holding every changed site of that loop.
    Loop,
    /// The whole method.
    Method,
    /// The region around one changed site (fallback).
    Site,
    /// A method only the new version has.
    NewOnly,
}

#[derive(Clone)]
struct Unit {
    file: String,
    /// Graph name, `Type.Member`.
    unit: String,
    id: String,
    kind: Kind,
    /// `--site` lines per side (empty for whole methods).
    site: [Vec<u64>; 2],
    /// Indexes (into the file's parts) of the parts this unit compares.
    parts: Vec<usize>,
}

/// Units to try for one group of parts, in order of preference: the first that lowers is used; `Site` units
/// are tried one by one.
struct Candidate {
    first: Unit,
    fallback: Vec<Unit>,
}

fn main() {
    let mut a: Vec<String> = std::env::args().skip(1).collect();
    let mut worker_cmd = std::env::var("WITNESS_WORKER").ok();
    let mut bin: Option<PathBuf> = None;
    while a.first().is_some_and(|x| x.starts_with("--")) {
        let k = a.remove(0);
        let v = if a.is_empty() { die(&format!("{k} needs a value")) } else { a.remove(0) };
        match k.as_str() {
            "--worker" => worker_cmd = Some(v),
            "--bin" => bin = Some(v.into()),
            _ => die(&format!("unknown option {k}")),
        }
    }
    let [stage, bundle, repo, old, new] = <[String; 5]>::try_from(a).unwrap_or_else(|_| die("usage: witness-proof [--worker CMD] [--bin DIR] <stageDir> <bundleDir> <repo> <old> <new>"));
    let bin = bin.or_else(|| std::env::var("WITNESS_BIN").ok().map(PathBuf::from))
        .unwrap_or_else(|| std::env::current_exe().unwrap().parent().unwrap().to_path_buf());
    let (stage, bundle) = (PathBuf::from(stage), PathBuf::from(bundle));
    std::fs::create_dir_all(&stage).ok();
    let tmp = stage.join(".proof");
    std::fs::create_dir_all(&tmp).ok();
    let sides = ["old", "new"];
    let commits = [old.as_str(), new.as_str()];
    let checkouts = [std::env::var("WITNESS_WT_OLD").unwrap_or_default(), std::env::var("WITNESS_WT_NEW").unwrap_or_default()];

    let parts = record(&bundle, &stage, "parts");
    let parts: Vec<Value> = parts["parts"].as_array().cloned().unwrap_or_default();
    let line_key = ["deletedLines", "addedLines"];
    let file_of = |p: &Value| p["new"]["file"].as_str().or(p["old"]["file"].as_str()).unwrap_or("").to_string();

    // changed-line counts and production line list for the scope
    let mut counts: BTreeMap<String, u64> = ["production", "test", "docs", "snapshot", "generated"].iter().map(|k| (k.to_string(), 0)).collect();
    let mut production = vec![];
    for p in &parts {
        let cat = p["category"].as_str().unwrap_or("").to_string();
        for (s, side) in sides.iter().enumerate() {
            let ls = lines_of(&p[line_key[s]]);
            *counts.entry(cat.clone()).or_default() += ls.len() as u64;
            if cat == "production" {
                let file = p[side]["file"].as_str().or(p[sides[1 - s]]["file"].as_str()).unwrap_or("").to_string();
                for l in ls {
                    production.push(json!({"side": side, "file": file, "line": l}));
                }
            }
        }
    }

    // production parts per file; why a part's lines are not compared when no unit covers them
    let mut by_file: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    let mut reasons: Vec<Value> = vec![];
    let mut default_reasons: Vec<Value> = vec![];
    let reason_for = |file: &str, p: &Value, why: &str| -> Vec<Value> {
        (0..2).filter_map(|s| {
            let ls = lines_of(&p[line_key[s]]);
            Some(json!({"side": sides[s], "file": file, "line": ls.iter().min()?, "endLine": ls.iter().max()?, "reason": why}))
        }).collect()
    };
    for p in parts.iter().filter(|p| p["category"] == "production") {
        let f = file_of(p);
        if f.ends_with(".cs") {
            by_file.entry(f).or_default().push(p);
        } else {
            default_reasons.extend(reason_for(&f, p, "no front end for this file type"));
        }
    }

    let mut worker: Option<Worker> = None;
    let mut units: Vec<Unit> = vec![];
    let mut models: Vec<Value> = vec![];
    for (fi, (file, ps)) in by_file.iter().enumerate() {
        let texts: Vec<String> = (0..2).map(|s| show(&repo, commits[s], file).unwrap_or_default()).collect();
        let sx: Vec<Syntax> = texts.iter().map(|t| syntax(t)).collect();
        let changed: Vec<BTreeSet<u64>> = (0..2).map(|s| ps.iter().flat_map(|p| lines_of(&p[line_key[s]])).collect()).collect();
        let map = line_map(&changed[0], &changed[1], texts[0].lines().count() as u64, texts[1].lines().count() as u64);
        let has = |s: usize, ty: &str, name: &str| sx[s].members.iter().any(|m| m.kind != "property" && m.ty == ty && m.name == name);
        let member_names: Vec<String> = ps.iter().filter(|p| p["kind"] == "member").filter_map(|p| p["member"].as_str().map(String::from)).collect();

        // group the parts: (method, loop) -> parts, in order of first appearance
        let mut groups: Vec<((String, Option<u64>), Vec<usize>)> = vec![];
        let mut new_only: Vec<(String, usize)> = vec![];
        for (pi, p) in ps.iter().enumerate() {
            let at: Vec<Option<u64>> = (0..2).map(|s| lines_of(&p[line_key[s]]).first().copied().or(p[sides[s]]["line"].as_u64())).collect();
            let at_new = at[1].or_else(|| at[0].and_then(|l| other_line(&map, 0, l, false)));
            let at_old = at[0].or_else(|| at[1].and_then(|l| other_line(&map, 1, l, false)));
            let mut en = at_new.and_then(|l| enclosing(&sx[1].members, l));
            let mut eo = at_old.and_then(|l| enclosing(&sx[0].members, l));
            if p["kind"] == "member" {
                // a member part starts at its comments: find the method by name inside the part
                let nm = p["member"].as_str().unwrap_or("");
                let (a, b) = (p["new"]["line"].as_u64().unwrap_or(0), p["new"]["endLine"].as_u64().unwrap_or(0));
                en = sx[1].members.iter().find(|m| m.kind != "property" && m.name == nm && a <= m.line && m.line <= b);
                eo = en.and_then(|n| sx[0].members.iter().find(|m| m.kind != "property" && m.ty == n.ty && m.name == n.name));
            }
            let why = match (eo, en) {
                (Some(o), Some(n)) if o.ty == n.ty && o.name == n.name => {
                    // a whole edited member is compared as its method; statements by their loop
                    let lp = if p["kind"] == "member" { None } else { at_new.and_then(|l| innermost_loop(&sx[1], l)).filter(|l| l.1).map(|l| l.0) };
                    let key = (format!("{}.{}", n.ty, n.name), lp);
                    match groups.iter_mut().find(|g| g.0 == key) {
                        Some(g) => g.1.push(pi),
                        None => groups.push((key, vec![pi])),
                    }
                    "not reached by its unit"
                }
                (_, Some(n)) if !has(0, &n.ty, &n.name) => {
                    let u = format!("{}.{}", n.ty, n.name);
                    if !new_only.iter().any(|x| x.0 == u) {
                        new_only.push((u, pi));
                    }
                    "new method: not lowered"
                }
                (Some(_), None) => "removed code: no new version to compare",
                _ if at_new.is_some_and(|l| sx[1].members.iter().any(|m| m.kind == "property" && m.line <= l && l <= m.end)) => "property body: not lowered",
                _ => "declaration: no statements to compare",
            };
            default_reasons.extend(reason_for(file, p, why));
        }
        let site_unit = |unit: &str, pi: usize| -> Unit {
            let p = ps[pi];
            let site: Vec<Vec<u64>> = (0..2).map(|s| {
                let ls = lines_of(&p[line_key[s]]);
                if ls.is_empty() { p[sides[s]]["line"].as_u64().into_iter().collect() } else { ls }
            }).collect();
            let first = site[1].first().or(site[0].first()).copied().unwrap_or(0);
            Unit { file: file.clone(), unit: unit.to_string(), id: format!("{unit}#{first}"), kind: Kind::Site, site: [site[0].clone(), site[1].clone()], parts: vec![pi] }
        };
        let mut cands: Vec<Candidate> = vec![];
        for ((unit, lp), pis) in &groups {
            let first_line = pis.iter().flat_map(|pi| lines_of(&ps[*pi]["addedLines"]).into_iter().chain(ps[*pi]["new"]["line"].as_u64())).min().unwrap_or(0);
            let id = format!("{unit}#{first_line}");
            let fallback: Vec<Unit> = pis.iter().filter(|pi| ps[**pi]["kind"] == "statements").map(|pi| site_unit(unit, *pi)).collect();
            let first = match lp {
                Some(_) => {
                    // the same line range on both sides: each side's changed statements and the other side's bounds
                    let own: Vec<Vec<u64>> = (0..2).map(|s| {
                        let ls: Vec<u64> = pis.iter().flat_map(|pi| {
                            let l = lines_of(&ps[*pi][line_key[s]]);
                            if l.is_empty() { ps[*pi][sides[s]]["line"].as_u64().into_iter().collect() } else { l }
                        }).collect();
                        widen(&sx[s], &ls)
                    }).collect();
                    let mut site = [own[0].clone(), own[1].clone()];
                    for s in 0..2 {
                        if let (Some(lo), Some(hi)) = (own[s].iter().min(), own[s].iter().max()) {
                            site[1 - s].extend(other_line(&map, s, *lo, false));
                            site[1 - s].extend(other_line(&map, s, *hi, true));
                        }
                    }
                    for s in site.iter_mut() {
                        s.sort();
                        s.dedup();
                    }
                    Unit { file: file.clone(), unit: unit.clone(), id, kind: Kind::Loop, site, parts: pis.clone() }
                }
                None => Unit { file: file.clone(), unit: unit.clone(), id, kind: Kind::Method, site: [vec![], vec![]], parts: pis.clone() },
            };
            cands.push(Candidate { first, fallback });
        }
        let cfg_units: Vec<String> = groups.iter().map(|g| g.0 .0.clone()).chain(new_only.iter().map(|x| x.0.clone())).collect();
        if cfg_units.is_empty() {
            continue;
        }
        let w = worker.get_or_insert_with(|| Worker::start(worker_cmd.as_deref().unwrap_or_else(|| die("no worker: --worker or WITNESS_WORKER"))));
        let mut cfg_paths = vec![];
        for s in 0..2 {
            let present: BTreeSet<String> = sx[s].members.iter().map(|m| format!("{}.{}", m.ty, m.name)).collect();
            let mut methods: Vec<String> = cfg_units.clone();
            for m in &sx[s].members {
                if m.kind == "ctor" || (m.kind == "method" && member_names.contains(&m.name)) {
                    methods.push(format!("{}.{}", m.ty, m.name));
                }
            }
            let mut seen = BTreeSet::new();
            methods.retain(|m| present.contains(m) && seen.insert(m.clone()));
            let p = tmp.join(format!("cfg.{}.{fi}.json", sides[s]));
            if methods.is_empty() {
                write_json(&p, &json!({"result": {"methods": []}}));
                cfg_paths.push(p);
                continue;
            }
            let Some(reply) = w.call("cfg", json!({"checkout": {"worktree": checkouts[s], "commit": commits[s]},
                "methods": methods.iter().map(|m| json!({"file": file, "name": m})).collect::<Vec<_>>()})) else {
                eprintln!("witness-proof: worker failed on cfg {file} ({}); its units are not compared", sides[s]);
                let _ = w.child.kill();
                worker = None;
                break;
            };
            write_json(&p, &reply);
            cfg_paths.push(p);
        }
        if cfg_paths.len() < 2 {
            for p in ps {
                reasons.extend(reason_for(file, p, "worker failed: not compared"));
            }
            continue;
        }
        // lower one unit on both sides; Err names why it cannot be compared
        let lower = |u: &Unit, n: usize| -> Result<Vec<PathBuf>, String> {
            let mut pair = vec![];
            for s in 0..2 {
                let out = tmp.join(format!("model.{}.{n}.json", sides[s]));
                if u.kind == Kind::NewOnly && s == 0 {
                    continue;
                }
                let mut args: Vec<String> = vec!["lower".into(), "--cfg".into(), cfg_paths[s].display().to_string(), "--unit".into(), u.unit.clone(),
                    "--version".into(), sides[s].into(), "--unit-id".into(), u.id.clone()];
                if !u.site[s].is_empty() {
                    args.extend(["--site".into(), csv(&u.site[s])]);
                }
                if u.kind == Kind::Loop {
                    args.extend(["--per-iteration".into(), "--exact".into()]);
                }
                for m in &member_names {
                    args.extend(["--inline".into(), m.clone()]);
                }
                if !changed[s].is_empty() {
                    args.extend(["--derive-delegates".into(), csv(&changed[s].iter().copied().collect::<Vec<_>>())]);
                }
                for p in ps {
                    let ls = lines_of(&p[line_key[s]]);
                    if !ls.is_empty() {
                        args.extend(["--part".into(), csv(&ls)]);
                    }
                }
                run(&bin, "translator", &args, Some(&out)).map_err(|e| format!("{} side: {e}", sides[s]))?;
                let m = read_json(&out);
                if u.kind == Kind::Method || u.kind == Kind::NewOnly {
                    // a whole method that does not lower as one piece is not a unit
                    if let Some(r) = m["lines"]["notModelled"].as_array().into_iter().flatten().filter_map(|n| n["reason"].as_str())
                        .find(|r| *r == "loop" || *r == "path explosion") {
                        return Err(format!("{} side: whole method not lowered ({r})", sides[s]));
                    }
                }
                pair.push(out);
            }
            if u.kind == Kind::NewOnly {
                // the old side of new code: the same inputs, nothing done
                let m = read_json(&pair[0]);
                let out = tmp.join(format!("model.old.{n}.json"));
                write_json(&out, &json!({"provenance": "pipeline", "unit": m["unit"], "version": "old", "params": m["params"], "opaque": [],
                    "contracts": [], "body": {"op": "nothing"}, "lines": {"modelled": [], "notModelled": []}}));
                pair.insert(0, out);
            }
            Ok(pair)
        };
        let accept = |u: Unit, pair: Vec<PathBuf>, units: &mut Vec<Unit>, models: &mut Vec<Value>| {
            for p in &pair {
                models.push(json!({"file": u.file, "model": read_json(p)}));
            }
            units.push(u);
        };
        for c in cands {
            match lower(&c.first, units.len()) {
                Ok(pair) => accept(c.first, pair, &mut units, &mut models),
                Err(e) => {
                    eprintln!("witness-proof: {} ({:?}): {e}; falling back to its sites", c.first.id, c.first.kind);
                    for pi in &c.first.parts {
                        if ps[*pi]["kind"] != "statements" {
                            reasons.extend(reason_for(file, ps[*pi], &format!("not lowered: {e}")));
                        }
                    }
                    for u in c.fallback {
                        match lower(&u, units.len()) {
                            Ok(pair) => accept(u, pair, &mut units, &mut models),
                            Err(e2) => {
                                eprintln!("witness-proof: {}: {e2}", u.id);
                                reasons.extend(reason_for(file, ps[u.parts[0]], &format!("not lowered: {e2}")));
                            }
                        }
                    }
                }
            }
        }
        // new methods no compared unit inlined
        for (unit, pi) in new_only {
            let lines = lines_of(&ps[pi]["addedLines"]);
            let covered = models.iter().any(|m| m["file"] == file.as_str() && m["model"]["version"] == "new"
                && (m["model"]["lines"]["modelled"].as_array().is_some_and(|a| a.iter().any(|l| lines.contains(&l.as_u64().unwrap_or(0))))
                    || m["model"]["lines"]["notModelled"].as_array().is_some_and(|a| a.iter().any(|n| lines.contains(&n["location"]["line"].as_u64().unwrap_or(0))))));
            if covered {
                continue;
            }
            let first = lines.first().copied().unwrap_or(0);
            let u = Unit { file: file.clone(), id: format!("{unit}#{first}"), unit, kind: Kind::NewOnly, site: [vec![], vec![]], parts: vec![pi] };
            match lower(&u, units.len()) {
                Ok(pair) => accept(u, pair, &mut units, &mut models),
                Err(e) => {
                    eprintln!("witness-proof: {}: {e}", u.id);
                    reasons.extend(reason_for(file, ps[pi], &format!("new method, not lowered: {e}")));
                }
            }
        }
    }
    // architectural facts (public API, signatures) for the builder's public-api differences, staged as-is
    if worker.is_none() {
        worker = worker_cmd.as_deref().map(Worker::start);
    }
    if let Some(w) = worker.as_mut() {
        let req = json!({"old": {"worktree": checkouts[0], "commit": commits[0]}, "new": {"worktree": checkouts[1], "commit": commits[1]}});
        match w.call("facts", req) {
            Some(r) if r.get("error").is_none() => write_json(&stage.join("facts.json"), &r),
            r => eprintln!("witness-proof: no facts ({})", r.map(|v| v["error"].to_string()).unwrap_or_else(|| "worker failed".into())),
        }
    }
    if let Some(mut w) = worker {
        drop(w.child.stdin.take());
        let _ = w.child.wait();
    }

    // each differ run; a unit the differ rejects is not compared, and says so in the scope
    let (mut rows, mut lean_rows, mut coverage) = (vec![], vec![], vec![]);
    let (mut first_row, mut first_diff) = (1usize, 1usize);
    let lean_dir = stage.join("lean");
    std::fs::create_dir_all(&lean_dir).ok();
    let mut dropped: BTreeSet<String> = BTreeSet::new();
    for (i, u) in units.iter().enumerate() {
        let (mo, mn) = (tmp.join(format!("model.old.{i}.json")), tmp.join(format!("model.new.{i}.json")));
        let (sr, rr, br) = (tmp.join(format!("solver-rows.{i}.json")), tmp.join(format!("rows.{i}.json")), tmp.join(format!("bridge.{i}.json")));
        let s = |p: &Path| p.display().to_string();
        let changed_new: Vec<u64> = by_file[&u.file].iter().flat_map(|p| lines_of(&p["addedLines"])).collect();
        let mut args = vec!["--old".into(), s(&mo), "--new".into(), s(&mn), "--out".into(), s(&sr), "--rows".into(), s(&rr), "--bridge".into(), s(&br),
            "--first-row".into(), first_row.to_string(), "--first-difference".into(), first_diff.to_string(), "--file".into(), u.file.clone(),
            "--changed".into(), csv(&changed_new)];
        if u.kind == Kind::NewOnly {
            args.push("--new-only".into());
        }
        if let Err(e) = run(&bin, "differ", &args, None) {
            eprintln!("witness-proof: {}: {e}", u.id);
            dropped.insert(u.id.clone());
            for pi in &u.parts {
                reasons.extend(reason_for(&u.file, by_file[&u.file][*pi], &format!("not compared: {e}")));
            }
            continue;
        }
        let r = read_json(&rr);
        let urows = r["rows"].as_array().cloned().unwrap_or_default();
        let num = |v: &Value, p: char| v.as_str().and_then(|x| x.strip_prefix(p)).and_then(|x| x.parse::<usize>().ok());
        let srv = read_json(&sr);
        let max_row = srv["rows"].as_array().into_iter().flatten().filter_map(|x| num(&x["id"], 'r')).chain(urows.iter().filter_map(|x| num(&x["id"], 'r'))).max();
        if let Some(m) = max_row {
            first_row = m + 1;
        }
        if let Some(m) = urows.iter().filter_map(|x| num(&x["difference"], 'd')).max() {
            first_diff = m + 1;
        }
        rows.extend(urows);
        let ld = tmp.join(format!("lean.{i}"));
        let largs = vec!["--old".into(), s(&mo), "--new".into(), s(&mn), "--rows".into(), s(&sr), "--bridge".into(), s(&br), "--out".into(), s(&ld)];
        match run(&bin, "lean-certify", &largs, None) {
            Ok(()) => {
                let l = read_json(&ld.join("lean.json"));
                if let Ok(rd) = std::fs::read_dir(ld.join("lean")) {
                    for e in rd.flatten() {
                        let _ = std::fs::copy(e.path(), lean_dir.join(e.file_name()));
                    }
                }
                lean_rows.extend(l["rows"].as_array().cloned().unwrap_or_default());
                coverage.extend(l["coverage"].as_array().cloned().unwrap_or_default());
            }
            Err(e) => eprintln!("witness-proof: lean {}: {e}", u.id),
        }
    }
    // scope over the models of the units that were compared, then each unit's rows fill its assumptions
    let models: Vec<&Value> = models.iter().filter(|m| !dropped.contains(m["model"]["unit"].as_str().unwrap_or(""))).collect();
    reasons.extend(default_reasons);
    let scope_in = tmp.join("scope-input.json");
    write_json(&scope_in, &json!({"changedLines": counts, "production": production, "models": models, "reasons": reasons}));
    let scope_p = tmp.join("scope.json");
    run(&bin, "translator", &["scope".into(), scope_in.display().to_string()], Some(&scope_p)).unwrap_or_else(|e| die(&e));
    let mut scope = read_json(&scope_p);
    fill_assumptions(&mut scope, &rows, &units, &tmp);
    scope["provenance"] = json!("pipeline");
    write_json(&stage.join("scope.json"), &scope);
    write_json(&stage.join("rows.json"), &json!({"provenance": "pipeline", "rows": rows}));
    write_json(&stage.join("lean.json"), &json!({"provenance": "pipeline", "rows": lean_rows, "coverage": coverage}));
    if std::env::var("WITNESS_PROOF_KEEP").is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    let kinds = |k: Kind| units.iter().filter(|u| u.kind == k && !dropped.contains(&u.id)).count();
    eprintln!("witness-proof: {} units ({} loop, {} method, {} site, {} new-only), {} rows",
        units.len() - dropped.len(), kinds(Kind::Loop), kinds(Kind::Method), kinds(Kind::Site), kinds(Kind::NewOnly), rows.len());
}

/// `scope.assumptions[].rows`: the rows whose `assumptions` name the contract, for the units whose model lists
/// that contract with that symbol (what `differ --scope-in` does per unit).
fn fill_assumptions(scope: &mut Value, rows: &[Value], units: &[Unit], tmp: &Path) {
    let Some(asm) = scope.get_mut("assumptions").and_then(Value::as_array_mut) else { return };
    let models: Vec<(String, Vec<Value>)> = (0..units.len()).map(|i| {
        let mut cs = vec![];
        for side in ["old", "new"] {
            let p = tmp.join(format!("model.{side}.{i}.json"));
            if p.exists() {
                cs.extend(read_json(&p)["contracts"].as_array().cloned().unwrap_or_default());
            }
        }
        (units[i].id.clone(), cs)
    }).collect();
    for a in asm {
        let ids: Vec<&Value> = rows.iter().filter(|r| {
            r["assumptions"].as_array().is_some_and(|x| x.contains(&a["contract"]))
                && models.iter().any(|(id, cs)| r["unit"] == id.as_str() && cs.iter().any(|c| c["contract"] == a["contract"] && c["symbol"] == a["symbol"]))
        }).map(|r| &r["id"]).collect();
        a["rows"] = json!(ids);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "class C {\n  void M(int[] xs) {\n    foreach (var x in xs)\n    {\n      if (x > 1)\n      {\n        Report(x);\n        continue;\n      }\n      Add(x);\n    }\n    Done();\n  }\n}\n";

    #[test]
    fn a_jump_is_widened_to_the_if_that_guards_it() {
        let sx = syntax(SRC);
        assert_eq!(widen(&sx, &[8]), vec![5, 6, 7, 8]);
        assert_eq!(widen(&sx, &[10]), vec![10]);
    }

    #[test]
    fn sites_are_grouped_by_their_innermost_foreach() {
        let sx = syntax(SRC);
        assert_eq!(innermost_loop(&sx, 8), Some((3, true)));
        assert_eq!(innermost_loop(&sx, 12), None);
        assert_eq!(enclosing(&sx.members, 8).map(|m| m.name.as_str()), Some("M"));
    }

    #[test]
    fn unchanged_lines_pair_up_around_a_hunk() {
        // old line 3 replaced by new lines 3..=5
        let map = line_map(&BTreeSet::from([3]), &BTreeSet::from([3, 4, 5]), 6, 8);
        assert_eq!(other_line(&map, 1, 6, false), Some(4));
        assert_eq!(other_line(&map, 1, 4, false), Some(2));
        assert_eq!(other_line(&map, 1, 4, true), Some(4));
        assert_eq!(other_line(&map, 0, 2, false), Some(2));
    }
}
