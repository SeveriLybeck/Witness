//! Witness block cutter (D29, D31, D33): `git diff -U0` plus tree-sitter syntax into parts,
//! blocks and mechanical groups, written as the contract records `files`, `parts`, `blocks`, `groups`.
pub mod csharp;

use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const CATEGORIES: [&str; 5] = ["production", "test", "docs", "snapshot", "generated"];

#[derive(Debug, Clone)]
pub struct DLine {
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
}
#[derive(Debug, Clone, Default)]
pub struct Hunk {
    pub lines: Vec<DLine>,
}
#[derive(Debug, Clone, Default)]
pub struct FileDiff {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub hunks: Vec<Hunk>,
}
impl FileDiff {
    pub fn path(&self) -> &str {
        self.new_path.as_deref().or(self.old_path.as_deref()).unwrap_or("")
    }
}

fn range(s: &str) -> (u32, u32) {
    let mut it = s[1..].split(',');
    let a = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let c = it.next().map(|x| x.parse().unwrap_or(0)).unwrap_or(1);
    (a, c)
}

/// Parse `git diff -U0` output.
pub fn parse_diff(text: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = vec![];
    let (mut o, mut n) = (0u32, 0u32);
    let mut in_hunk = false;
    for l in text.lines() {
        if l.starts_with("diff --git ") {
            files.push(FileDiff::default());
            in_hunk = false;
        } else if !in_hunk && l.starts_with("--- ") {
            files.last_mut().unwrap().old_path = l[4..].strip_prefix("a/").map(|p| p.trim_end_matches('\t').to_string());
        } else if !in_hunk && l.starts_with("+++ ") {
            files.last_mut().unwrap().new_path = l[4..].strip_prefix("b/").map(|p| p.trim_end_matches('\t').to_string());
        } else if l.starts_with("@@ ") {
            let p: Vec<&str> = l.split(' ').collect();
            let (oa, oc) = range(p[1]);
            let (na, nc) = range(p[2]);
            o = if oc == 0 { oa + 1 } else { oa };
            n = if nc == 0 { na + 1 } else { na };
            files.last_mut().unwrap().hunks.push(Hunk::default());
            in_hunk = true;
        } else if !in_hunk {
        } else if let Some(t) = l.strip_prefix('+') {
            files.last_mut().unwrap().hunks.last_mut().unwrap().lines.push(DLine { old: None, new: Some(n), text: t.into() });
            n += 1;
        } else if let Some(t) = l.strip_prefix('-') {
            files.last_mut().unwrap().hunks.last_mut().unwrap().lines.push(DLine { old: Some(o), new: None, text: t.into() });
            o += 1;
        }
    }
    files.retain(|f| !f.hunks.is_empty());
    files
}

/// File category from the path (D29). Rules are repository conventions of the sample.
pub fn category(path: &str, all: &[String]) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if (lower.contains("/resources/") || lower.contains("/snapshots/")) && lower.starts_with("tests/") && path.ends_with(".g.cs")
        || path.ends_with(".verified.cs") || path.ends_with(".verified.txt")
    {
        "snapshot"
    } else if lower.starts_with("tests/") || lower.starts_with("test/") || lower.contains("/tests/") {
        "test"
    } else if lower.ends_with(".md") || lower.starts_with("doc/") || lower.starts_with("docs/") {
        "docs"
    } else if path.ends_with(".cs") && all.iter().any(|p| *p == format!("{}.tt", &path[..path.len() - 3]))
        || path.ends_with(".g.cs") || path.ends_with(".Designer.cs")
    {
        "generated"
    } else {
        "production"
    }
}

/// Thresholds (D33). Defaults: a comment run of 4 lines, or comments over a third of added code.
#[derive(Debug, Clone)]
pub struct Options {
    pub comment_run: usize,
    pub comment_ratio_den: usize,
}
impl Default for Options {
    fn default() -> Self {
        Options { comment_run: 4, comment_ratio_den: 3 }
    }
}

#[derive(Debug, Clone)]
pub struct Part {
    pub id: String,
    pub category: &'static str,
    pub kind: &'static str,
    pub member: Option<String>,
    pub old_file: Option<String>,
    pub new_file: Option<String>,
    pub added: Vec<u32>,
    pub deleted: Vec<u32>,
    pub added_text: Vec<String>,
    pub deleted_text: Vec<String>,
    pub large_comment: Vec<u32>,
    pub references: Vec<String>,
    /// Enclosing member and statement kind, for block names.
    pub enclosing: Option<String>,
    pub label: String,
}
impl Part {
    pub fn file(&self) -> &str {
        self.new_file.as_deref().or(self.old_file.as_deref()).unwrap_or("")
    }
    pub fn first_line(&self) -> u32 {
        self.added.first().or(self.deleted.first()).copied().unwrap_or(1)
    }
    /// Edit text with each line trimmed, deleted then added: the identical-edit key.
    pub fn trimmed_key(&self) -> String {
        let d: Vec<&str> = self.deleted_text.iter().map(|s| s.trim()).collect();
        let a: Vec<&str> = self.added_text.iter().map(|s| s.trim()).collect();
        format!("-{}\n+{}", d.join("\n"), a.join("\n"))
    }
    pub fn exact_key(&self) -> String {
        format!("-{}\n+{}", self.deleted_text.join("\n"), self.added_text.join("\n"))
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub id: String,
    pub name: String,
    pub category: &'static str,
    pub parts: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct Group {
    pub id: String,
    pub rule: &'static str,
    pub parts: Vec<String>,
    /// Distinct untrimmed edits among the members (indentation variants).
    pub variants: Vec<usize>,
}
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub file: String,
    pub category: &'static str,
    pub added: usize,
    pub deleted: usize,
}

#[derive(Debug, Clone)]
pub struct Cut {
    pub files: Vec<FileEntry>,
    pub parts: Vec<Part>,
    pub blocks: Vec<Block>,
    pub groups: Vec<Group>,
    /// `dependencies.json` from the worker; `None` without a worker.
    pub dependencies: Option<Value>,
}

fn is_comment(t: &str) -> bool {
    let t = t.trim();
    t.starts_with("//") || t.starts_with("/*") || t.starts_with('*')
}

/// Added lines marked as large comment (D33): every comment line when a run of `comment_run`
/// comment lines exists or comments exceed 1/`comment_ratio_den` of added code.
pub fn large_comment(lines: &[(u32, String)], o: &Options) -> Vec<u32> {
    let (mut run, mut best, mut com) = (0, 0, 0);
    for (_, t) in lines {
        if is_comment(t) {
            run += 1;
            com += 1;
            best = best.max(run);
        } else if !t.trim().is_empty() {
            run = 0;
        }
    }
    let code = lines.iter().filter(|(_, t)| !t.trim().is_empty() && !is_comment(t)).count();
    if best >= o.comment_run || (com > 0 && com * o.comment_ratio_den > code + com) {
        lines.iter().filter(|(_, t)| is_comment(t)).map(|(l, _)| *l).collect()
    } else {
        vec![]
    }
}

/// Source text of one side of a file; `None` when the file does not exist there.
pub type Reader<'a> = dyn Fn(&str, bool) -> Option<String> + 'a;

/// Cut a parsed diff into files and parts. `read(path, new_side)` returns file text.
pub fn cut_parts(diff: &[FileDiff], read: &Reader, o: &Options) -> (Vec<FileEntry>, Vec<Part>) {
    let paths: Vec<String> = diff.iter().map(|f| f.path().to_string()).collect();
    let mut files = vec![];
    let mut parts: Vec<Part> = vec![];
    for f in diff {
        let path = f.path();
        let cat = category(path, &paths);
        let all: Vec<&DLine> = f.hunks.iter().flat_map(|h| h.lines.iter()).collect();
        files.push(FileEntry {
            file: path.into(),
            category: cat,
            added: all.iter().filter(|l| l.new.is_some()).count(),
            deleted: all.iter().filter(|l| l.old.is_some()).count(),
        });
        let mk = |lines: &[&DLine], kind: &'static str| -> Part {
            Part {
                id: String::new(),
                category: cat,
                kind,
                member: None,
                old_file: f.old_path.clone(),
                new_file: f.new_path.clone(),
                added: lines.iter().filter_map(|l| l.new).collect(),
                deleted: lines.iter().filter_map(|l| l.old).collect(),
                added_text: lines.iter().filter(|l| l.new.is_some()).map(|l| l.text.clone()).collect(),
                deleted_text: lines.iter().filter(|l| l.old.is_some()).map(|l| l.text.clone()).collect(),
                large_comment: vec![],
                references: vec![],
                enclosing: None,
                label: "hunk".into(),
            }
        };
        match cat {
            "test" | "docs" => parts.push(mk(&all, "file")),
            "production" if path.ends_with(".cs") => parts.extend(cut_csharp(f, read, &mk)),
            _ => {
                for h in &f.hunks {
                    let ls: Vec<&DLine> = h.lines.iter().collect();
                    parts.push(mk(&ls, "statements"));
                }
            }
        }
    }
    let mut parts = split_repeats(parts);
    for p in parts.iter_mut() {
        if p.category == "production" && (p.file().ends_with(".cs") || p.file().ends_with(".tt")) {
            let ls: Vec<(u32, String)> = p.added.iter().copied().zip(p.added_text.iter().cloned()).collect();
            p.large_comment = large_comment(&ls, o);
        }
    }
    let rank = |c: &str| CATEGORIES.iter().position(|x| *x == c).unwrap();
    // Stable: categories in order, files in diff order, parts by line.
    let mut idx: Vec<usize> = (0..parts.len()).collect();
    idx.sort_by_key(|&i| rank(parts[i].category));
    let mut parts: Vec<Part> = idx.into_iter().map(|i| parts[i].clone()).collect();
    for (i, p) in parts.iter_mut().enumerate() {
        p.id = format!("p{}", i + 1);
    }
    (files, parts)
}

/// In snapshot and generated files, a hunk that holds a pure insert repeated as a whole hunk
/// elsewhere (2+ times) is split: the repeat becomes its own part, the rest stays one part (D31).
fn split_repeats(parts: Vec<Part>) -> Vec<Part> {
    let foldable = |p: &Part| p.kind == "statements" && matches!(p.category, "snapshot" | "generated");
    let mut freq: BTreeMap<(&str, Vec<String>), usize> = BTreeMap::new();
    for p in parts.iter().filter(|p| foldable(p) && p.deleted.is_empty()) {
        *freq.entry((p.category, p.added_text.iter().map(|s| s.trim().to_string()).collect())).or_default() += 1;
    }
    let mut keys: Vec<(&str, Vec<String>)> = freq.into_iter().filter(|(_, n)| *n >= 2).map(|(k, _)| k).collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.1.len()));
    let mut out = vec![];
    for p in &parts {
        let trimmed: Vec<String> = p.added_text.iter().map(|s| s.trim().to_string()).collect();
        let hit = foldable(p).then_some(()).and_then(|_| {
            keys.iter().filter(|k| k.0 == p.category && k.1.len() < trimmed.len()).find_map(|k| {
                (0..=trimmed.len() - k.1.len()).find(|&s| trimmed[s..s + k.1.len()] == k.1[..]).map(|s| (s, k.1.len()))
            })
        });
        match hit {
            Some((s, len)) => {
                let mut rep = p.clone();
                rep.added = p.added[s..s + len].to_vec();
                rep.added_text = p.added_text[s..s + len].to_vec();
                rep.deleted.clear();
                rep.deleted_text.clear();
                rep.old_file = None;
                let mut rest = p.clone();
                rest.added = [&p.added[..s], &p.added[s + len..]].concat();
                rest.added_text = [&p.added_text[..s], &p.added_text[s + len..]].concat();
                let rest_first = rest.added.first().or(rest.deleted.first()).copied().unwrap_or(0);
                if rest_first < rep.added[0] {
                    out.push(rest);
                    out.push(rep);
                } else {
                    out.push(rep);
                    out.push(rest);
                }
            }
            None => out.push(p.clone()),
        }
    }
    out
}

fn cut_csharp(f: &FileDiff, read: &Reader, mk: &dyn Fn(&[&DLine], &'static str) -> Part) -> Vec<Part> {
    let side_units = |new_side: bool| -> Vec<csharp::Unit> {
        let changed: Vec<(u32, usize)> = f.hunks.iter().enumerate()
            .flat_map(|(h, hk)| hk.lines.iter().filter_map(move |l| if new_side { l.new } else { l.old }.map(|x| (x, h))))
            .collect();
        if changed.is_empty() {
            return vec![];
        }
        let path = if new_side { f.new_path.as_deref() } else { f.old_path.as_deref() };
        match path.and_then(|p| read(p, new_side)) {
            Some(src) => csharp::cut_side(&src, &changed),
            None => {
                // No source: one unit per hunk.
                let mut by: BTreeMap<usize, csharp::Unit> = BTreeMap::new();
                for (l, h) in changed {
                    let u = by.entry(h).or_insert_with(csharp::hunk_unit);
                    u.lines.push(l);
                    u.hunks.insert(h);
                }
                by.into_values().collect()
            }
        }
    };
    let newu = side_units(true);
    let oldu = side_units(false);
    // Pair old units with new units sharing a hunk; components become parts.
    let n = newu.len();
    let all: Vec<(&csharp::Unit, bool)> = newu.iter().map(|u| (u, true)).chain(oldu.iter().map(|u| (u, false))).collect();
    let mut parent: Vec<usize> = (0..all.len()).collect();
    fn find(p: &mut Vec<usize>, x: usize) -> usize {
        let mut r = x;
        while p[r] != r {
            r = p[r];
        }
        p[x] = r;
        r
    }
    for (oi, o) in oldu.iter().enumerate() {
        for h in &o.hunks {
            let cands: Vec<usize> = (0..n).filter(|&i| newu[i].hunks.contains(h)).collect();
            if cands.is_empty() {
                continue;
            }
            let pick = cands.iter().copied()
                .find(|&i| newu[i].is_member == o.is_member && newu[i].member == o.member && newu[i].label == o.label)
                .or_else(|| cands.iter().copied().find(|&i| newu[i].is_member == o.is_member && newu[i].member == o.member && o.is_member))
                .or_else(|| cands.iter().copied().find(|&i| newu[i].enclosing == o.enclosing))
                .unwrap_or(cands[0]);
            let (a, b) = (find(&mut parent, n + oi), find(&mut parent, pick));
            parent[a] = b;
        }
    }
    let mut comps: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..all.len() {
        let r = find(&mut parent, i);
        comps.entry(r).or_default().push(i);
    }
    let mut out: Vec<(usize, u32, Part)> = vec![];
    for members in comps.values() {
        let mut added: BTreeSet<u32> = BTreeSet::new();
        let mut deleted: BTreeSet<u32> = BTreeSet::new();
        let mut hunks: BTreeSet<usize> = BTreeSet::new();
        for &i in members {
            let (u, new_side) = all[i];
            hunks.extend(u.hunks.iter().copied());
            if new_side { added.extend(u.lines.iter()) } else { deleted.extend(u.lines.iter()) }
        }
        let lines: Vec<&DLine> = f.hunks.iter().flat_map(|h| h.lines.iter())
            .filter(|l| l.new.map_or(false, |x| added.contains(&x)) || l.old.map_or(false, |x| deleted.contains(&x)))
            .collect();
        let head = members.iter().map(|&i| all[i].0).find(|u| u.is_member).unwrap_or(all[members[0]].0);
        let mut p = mk(&lines, if head.is_member { "member" } else { "statements" });
        p.member = if head.is_member { head.member.clone() } else { None };
        p.enclosing = head.enclosing.clone();
        p.label = head.label.clone();
        let first = *hunks.iter().next().unwrap();
        out.push((first, p.first_line(), p));
    }
    out.sort_by_key(|x| (x.0, x.1));
    out.into_iter().map(|x| x.2).collect()
}

/// Worker references per part id, from a `references` worker reply.
pub fn references_from_reply(v: &Value) -> BTreeMap<String, Vec<String>> {
    let mut m = BTreeMap::new();
    for p in v["result"]["parts"].as_array().into_iter().flatten() {
        let refs = p["references"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
        m.insert(p["id"].as_str().unwrap_or("").to_string(), refs);
    }
    m
}

fn base(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Join parts into blocks (D29) and name them by rule (D33).
pub fn join_blocks(parts: &mut [Part], refs: &BTreeMap<String, Vec<String>>) -> Vec<Block> {
    for p in parts.iter_mut() {
        if let Some(r) = refs.get(&p.id) {
            p.references = r.clone();
        }
    }
    // key -> (first part index, name, category, part ids)
    let mut keys: BTreeMap<String, (usize, String, &'static str, Vec<String>)> = BTreeMap::new();
    let members: BTreeSet<String> = parts.iter()
        .filter(|p| p.category == "production" && p.kind == "member")
        .filter_map(|p| p.member.clone()).collect();
    for (i, p) in parts.iter().enumerate() {
        let (key, name) = match (p.category, p.kind) {
            ("test" | "docs", _) => (format!("file:{}", p.id), base(p.file()).to_string()),
            ("production", "member") => {
                let m = p.member.clone().unwrap_or_else(|| format!("{}:{}", base(p.file()), p.first_line()));
                (format!("member:{}:{m}", p.file()), m)
            }
            ("production", _) => {
                let hit: Vec<&String> = p.references.iter().filter(|r| members.contains(*r)).collect::<BTreeSet<_>>().into_iter().collect();
                let owner = parts.iter().find(|q| q.kind == "member" && q.category == "production" && hit.len() == 1 && q.member.as_ref() == Some(hit[0]));
                match owner {
                    Some(q) => (format!("member:{}:{}", q.file(), hit[0]), hit[0].clone()),
                    None => {
                        let name = match &p.enclosing {
                            Some(e) if p.label != "hunk" => format!("{e} {}", p.label),
                            _ => format!("{}:{}", base(p.file()), p.first_line()),
                        };
                        (format!("edit:{}", p.trimmed_key()), name)
                    }
                }
            }
            _ => continue, // snapshot and generated parts are not blocks
        };
        let e = keys.entry(key).or_insert((i, name, p.category, vec![]));
        e.0 = e.0.min(i);
        e.3.push(p.id.clone());
    }
    let mut v: Vec<_> = keys.into_values().collect();
    v.sort_by_key(|x| x.0);
    v.into_iter().enumerate()
        .map(|(i, (_, name, category, ps))| Block { id: format!("b{}", i + 1), name, category, parts: ps })
        .collect()
}

/// Mechanical groups (D31). `identical-edit`: statement parts of snapshot or generated files
/// whose edits are equal after trimming each line (2+ members, per category). `formatting`: a production
/// part whose old and new text are equal once all whitespace is removed. Nothing else folds.
pub fn mechanical_groups(parts: &[Part]) -> Vec<Group> {
    let mut out: Vec<(usize, &'static str, Vec<usize>)> = vec![];
    let mut ident: BTreeMap<(&str, String), Vec<usize>> = BTreeMap::new();
    let mut grouped = BTreeSet::new();
    for (i, p) in parts.iter().enumerate() {
        if p.kind == "statements" && matches!(p.category, "snapshot" | "generated") {
            ident.entry((p.category, p.trimmed_key())).or_default().push(i);
        }
    }
    for (_, v) in ident {
        if v.len() >= 2 {
            grouped.extend(v.iter().copied());
            out.push((v[0], "identical-edit", v));
        }
    }
    let squash = |t: &[String]| -> String { t.concat().chars().filter(|c| !c.is_whitespace()).collect() };
    for (i, p) in parts.iter().enumerate() {
        if !grouped.contains(&i) && p.kind != "file" && p.category == "production" && !p.added.is_empty() && !p.deleted.is_empty()
            && squash(&p.added_text) == squash(&p.deleted_text)
        {
            out.push((i, "formatting", vec![i]));
        }
    }
    out.sort_by_key(|x| x.0);
    out.into_iter().enumerate()
        .map(|(k, (_, rule, v))| {
            let mut var: BTreeMap<String, usize> = BTreeMap::new();
            for &i in &v {
                *var.entry(parts[i].exact_key()).or_default() += 1;
            }
            let mut variants: Vec<usize> = var.into_values().collect();
            variants.sort_by(|a, b| b.cmp(a));
            Group { id: format!("g{}", k + 1), rule, parts: v.iter().map(|&i| parts[i].id.clone()).collect(), variants }
        })
        .collect()
}

/// Full cut of a parsed diff.
pub fn cut(diff: &[FileDiff], read: &Reader, refs: &BTreeMap<String, Vec<String>>, o: &Options) -> Cut {
    let (files, mut parts) = cut_parts(diff, read, o);
    let blocks = join_blocks(&mut parts, refs);
    let groups = mechanical_groups(&parts);
    Cut { files, parts, blocks, groups, dependencies: None }
}

fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("git").arg("-C").arg(repo).args(args).env("GIT_PAGER", "cat").output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(out.stdout)
}

/// Cut `old..new` of a git repository (read with git only; nothing is built or run).
pub fn cut_repo(repo: &Path, old: &str, new: &str, refs: &BTreeMap<String, Vec<String>>, o: &Options) -> Result<Cut, String> {
    let d = git(repo, &["diff", "-U0", "--no-color", "--no-renames", "--no-ext-diff", old, new])?;
    let diff = parse_diff(&String::from_utf8_lossy(&d));
    let read = |p: &str, new_side: bool| -> Option<String> {
        let rev = format!("{}:{}", if new_side { new } else { old }, p);
        git(repo, &["show", &rev]).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
    };
    Ok(cut(&diff, &read, refs, o))
}

/// The worker `references` request (worker-request.schema.json) for the production parts.
pub fn references_request(parts: &[Part], worktree: &str, commit: &str) -> Value {
    let ps: Vec<Value> = parts.iter().filter(|p| p.category == "production").filter_map(|p| {
        let l = loc(&p.new_file, &p.added);
        let l = if l.is_null() { loc(&p.old_file, &p.deleted) } else { l };
        (!l.is_null()).then(|| json!({"id": p.id, "location": l}))
    }).collect();
    let members: BTreeSet<String> = parts.iter().filter(|p| p.category == "production" && p.kind == "member")
        .filter_map(|p| p.member.clone()).collect();
    json!({"id": 1, "method": "references",
        "params": {"checkout": {"worktree": worktree, "commit": commit}, "parts": ps, "changedMembers": members}})
}

/// Cut in two steps: parts, then the worker (`sh -c <worker>`, one request line on stdin,
/// The worker `dependencies` request: production blocks with their part locations per side.
pub fn dependencies_request(c: &Cut, old_wt: &str, old_sha: &str, new_wt: &str, new_sha: &str) -> Value {
    let blocks: Vec<Value> = c.blocks.iter().filter(|b| b.category == "production").map(|b| {
        let ps: Vec<&Part> = b.parts.iter().filter_map(|id| c.parts.iter().find(|p| &p.id == id)).collect();
        let old: Vec<Value> = ps.iter().map(|p| loc(&p.old_file, &p.deleted)).filter(|v| !v.is_null()).collect();
        let new: Vec<Value> = ps.iter().map(|p| loc(&p.new_file, &p.added)).filter(|v| !v.is_null()).collect();
        json!({"id": b.id, "old": old, "new": new})
    }).collect();
    json!({"id": 2, "method": "dependencies", "params": {
        "old": {"worktree": old_wt, "commit": old_sha}, "new": {"worktree": new_wt, "commit": new_sha}, "blocks": blocks}})
}

/// Worktrees the worker reads. Given paths are used as they are; otherwise the cutter adds
/// detached git worktrees under the temp folder and removes them on drop.
pub struct Checkouts {
    repo: PathBuf,
    pub old: PathBuf,
    pub new: PathBuf,
    made: Vec<PathBuf>,
}
impl Checkouts {
    pub fn prepare(repo: &Path, old: &str, new: &str, old_wt: Option<PathBuf>, new_wt: Option<PathBuf>) -> Result<Checkouts, String> {
        let mut c = Checkouts { repo: repo.into(), old: PathBuf::new(), new: PathBuf::new(), made: vec![] };
        for (rev, given, side) in [(old, old_wt, "old"), (new, new_wt, "new")] {
            let p = match given {
                Some(p) => p,
                None => {
                    let p = std::env::temp_dir().join(format!("cutter-wt-{}-{side}", std::process::id()));
                    let _ = git(repo, &["worktree", "remove", "--force", &p.display().to_string()]);
                    let _ = std::fs::remove_dir_all(&p);
                    git(repo, &["worktree", "add", "--detach", "--quiet", &p.display().to_string(), rev])?;
                    c.made.push(p.clone());
                    p
                }
            };
            if side == "old" { c.old = p } else { c.new = p }
        }
        Ok(c)
    }
}
impl Drop for Checkouts {
    fn drop(&mut self) {
        for p in &self.made {
            let _ = git(&self.repo, &["worktree", "remove", "--force", &p.display().to_string()]);
            let _ = std::fs::remove_dir_all(p);
        }
        let _ = git(&self.repo, &["worktree", "prune"]);
    }
}

struct Worker {
    child: std::process::Child,
    out: std::io::BufReader<std::process::ChildStdout>,
}
impl Worker {
    fn start(cmd: &str) -> Result<Worker, String> {
        let mut child = Command::new("sh").arg("-c").arg(cmd)
            .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped())
            .spawn().map_err(|e| format!("worker: {e}"))?;
        let out = std::io::BufReader::new(child.stdout.take().unwrap());
        Ok(Worker { child, out })
    }
    fn call(&mut self, req: &Value) -> Result<Value, String> {
        use std::io::{BufRead, Write};
        let si = self.child.stdin.as_mut().ok_or("worker: stdin closed")?;
        writeln!(si, "{req}").and_then(|_| si.flush()).map_err(|e| format!("worker stdin: {e}"))?;
        let mut line = String::new();
        while line.trim().is_empty() {
            line.clear();
            if self.out.read_line(&mut line).map_err(|e| format!("worker: {e}"))? == 0 {
                return Err(format!("worker: no reply to {}", req["method"]));
            }
        }
        let v: Value = serde_json::from_str(&line).map_err(|e| format!("worker reply: {e}"))?;
        if v.get("error").is_some_and(|e| !e.is_null()) {
            return Err(format!("worker error on {}: {}", req["method"], v["error"]));
        }
        Ok(v)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let _ = self.child.wait();
    }
}

/// Cut with the worker: parts, `references` (new checkout), blocks and groups, then
/// `dependencies` (old and new checkouts) into `Cut::dependencies`.
pub fn cut_repo_with_worker(repo: &Path, old: &str, new: &str, worker: &str, wt: (Option<PathBuf>, Option<PathBuf>), o: &Options) -> Result<Cut, String> {
    let mut c = cut_repo(repo, old, new, &BTreeMap::new(), o)?;
    let sha = |r: &str| -> Result<String, String> {
        Ok(String::from_utf8_lossy(&git(repo, &["rev-parse", &format!("{r}^{{commit}}")])?).trim().to_string())
    };
    let (old_sha, new_sha) = (sha(old)?, sha(new)?);
    let co = Checkouts::prepare(repo, old, new, wt.0, wt.1)?;
    let mut w = Worker::start(worker)?;
    let reply = w.call(&references_request(&c.parts, &co.new.display().to_string(), &new_sha))?;
    let refs = references_from_reply(&reply);
    let has_members = c.parts.iter().any(|p| p.category == "production" && p.kind == "member");
    let statements: Vec<&Part> = c.parts.iter().filter(|p| p.category == "production" && p.kind == "statements").collect();
    if has_members && !statements.is_empty() && statements.iter().all(|p| refs.get(&p.id).map_or(true, |r| r.is_empty())) {
        return Err(format!("worker returned no references for any production part (worktree {})", co.new.display()));
    }
    c.blocks = join_blocks(&mut c.parts, &refs);
    c.groups = mechanical_groups(&c.parts);
    let dep = w.call(&dependencies_request(&c, &co.old.display().to_string(), &old_sha, &co.new.display().to_string(), &new_sha))?;
    let mut d = json!({"provenance": "pipeline", "dependencies": dep["result"]["dependencies"].as_array().cloned().unwrap_or_default()});
    if let Some(r) = dep["result"].get("reach").filter(|r| r.is_array()) {
        d["reach"] = r.clone();
    }
    c.dependencies = Some(d);
    Ok(c)
}


fn loc(file: &Option<String>, ls: &[u32]) -> Value {
    match (file, ls.iter().min(), ls.iter().max()) {
        (Some(f), Some(a), Some(b)) => json!({"file": f, "line": a, "endLine": b}),
        _ => Value::Null,
    }
}

impl Cut {
    pub fn files_json(&self) -> Value {
        let f: Vec<Value> = self.files.iter()
            .map(|f| json!({"file": f.file, "category": f.category, "added": f.added, "deleted": f.deleted})).collect();
        json!({"provenance": "pipeline", "files": f})
    }
    pub fn parts_json(&self) -> Value {
        let p: Vec<Value> = self.parts.iter().map(|p| {
            let mut v = json!({
                "id": p.id, "category": p.category, "kind": p.kind, "member": p.member,
                "old": loc(&p.old_file, &p.deleted), "new": loc(&p.new_file, &p.added),
                "addedLines": p.added, "deletedLines": p.deleted,
                "largeComment": !p.large_comment.is_empty(), "references": p.references,
            });
            if !p.large_comment.is_empty() {
                v["largeCommentLines"] = json!(p.large_comment);
            }
            v
        }).collect();
        json!({"provenance": "pipeline", "parts": p})
    }
    pub fn blocks_json(&self) -> Value {
        let b: Vec<Value> = self.blocks.iter()
            .map(|b| json!({"id": b.id, "name": b.name, "category": b.category, "parts": b.parts})).collect();
        json!({"provenance": "pipeline", "blocks": b})
    }
    pub fn groups_json(&self) -> Value {
        let g: Vec<Value> = self.groups.iter()
            .map(|g| json!({"id": g.id, "rule": g.rule, "parts": g.parts, "count": g.parts.len(), "variants": g.variants})).collect();
        json!({"provenance": "pipeline", "groups": g})
    }
    /// Write `files.json`, `parts.json`, `blocks.json`, `groups.json` into `dir`.
    pub fn write(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        for (n, v) in [("files", self.files_json()), ("parts", self.parts_json()), ("blocks", self.blocks_json()), ("groups", self.groups_json())] {
            std::fs::write(dir.join(format!("{n}.json")), serde_json::to_string_pretty(&v).unwrap() + "\n")?;
        }
        if let Some(d) = &self.dependencies {
            std::fs::write(dir.join("dependencies.json"), serde_json::to_string_pretty(d).unwrap() + "\n")?;
        }
        Ok(())
    }
}
