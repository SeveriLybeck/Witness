//! C# statement and member units from tree-sitter (D29). One side (old or new) of one file.
use std::collections::{BTreeMap, BTreeSet};
use tree_sitter::{Node, Parser};

/// A unit of changed lines on one side of one file.
#[derive(Debug, Clone)]
pub struct Unit {
    pub lines: Vec<u32>,
    pub hunks: BTreeSet<usize>,
    pub is_member: bool,
    /// Declared name when `is_member`.
    pub member: Option<String>,
    /// Nearest enclosing member (or the member itself).
    pub enclosing: Option<String>,
    /// Statement or member kind, for block names.
    pub label: String,
}

const MEMBERS: &[&str] = &[
    "method_declaration", "constructor_declaration", "destructor_declaration", "property_declaration",
    "field_declaration", "event_field_declaration", "event_declaration", "indexer_declaration",
    "operator_declaration", "conversion_operator_declaration", "delegate_declaration", "enum_member_declaration",
];
const TYPES: &[&str] = &[
    "class_declaration", "struct_declaration", "interface_declaration", "record_declaration",
    "record_struct_declaration", "enum_declaration", "namespace_declaration", "file_scoped_namespace_declaration",
];
const CONTAINERS: &[&str] = &["block", "declaration_list", "compilation_unit", "switch_body", "enum_member_declaration_list"];

fn is_member(k: &str) -> bool {
    MEMBERS.contains(&k)
}
fn is_type(k: &str) -> bool {
    TYPES.contains(&k)
}
fn is_cand(k: &str) -> bool {
    k.ends_with("_statement") || k == "switch_section" || k == "using_directive" || k == "extern_alias_directive"
        || is_member(k) || is_type(k)
}
fn is_container(k: &str) -> bool {
    CONTAINERS.contains(&k)
}

pub fn member_name(n: Node, src: &[u8]) -> Option<String> {
    let text = |x: Node| x.utf8_text(src).ok().map(String::from);
    if let Some(nm) = n.child_by_field_name("name") {
        return text(nm);
    }
    match n.kind() {
        "constructor_declaration" | "destructor_declaration" => None,
        "indexer_declaration" => Some("this[]".into()),
        _ => {
            // fields and events: first variable declarator
            let mut stack = vec![n];
            while let Some(x) = stack.pop() {
                if x.kind() == "variable_declarator" {
                    return x.child_by_field_name("name").and_then(text).or_else(|| x.named_child(0).and_then(text));
                }
                let mut c = x.walk();
                let ch: Vec<Node> = x.named_children(&mut c).collect();
                stack.extend(ch.into_iter().rev());
            }
            None
        }
    }
}

fn enclosing(n: Node, src: &[u8]) -> Option<String> {
    let mut c = Some(n);
    while let Some(x) = c {
        if is_member(x.kind()) || x.kind() == "local_function_statement" {
            if let Some(m) = member_name(x, src) {
                return Some(m);
            }
        }
        c = x.parent();
    }
    None
}

fn label(kind: &str) -> String {
    kind.trim_end_matches("_statement").trim_end_matches("_declaration").trim_end_matches("_directive").replace('_', " ")
}

enum A<'t> {
    Strong(Node<'t>),
    Weak(Node<'t>),
    Filler,
    Error,
}

fn is_brace_only(t: &str) -> bool {
    let t = t.trim();
    !t.is_empty() && t.chars().all(|c| matches!(c, '{' | '}' | ';' | ')' | ','))
}

/// Parse check used by tests (playbook B1).
pub fn parse_errors(src: &str) -> usize {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).expect("grammar loads");
    let t = p.parse(src, None).expect("parse");
    fn count(n: Node) -> usize {
        let mut c = n.walk();
        let own = usize::from(n.is_error() || n.is_missing());
        own + n.children(&mut c).map(count).sum::<usize>()
    }
    count(t.root_node())
}

/// Cut one side. `changed` holds (line, hunk index) in ascending line order.
pub fn cut_side(src: &str, changed: &[(u32, usize)]) -> Vec<Unit> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).expect("grammar loads");
    let tree = parser.parse(src, None).expect("parse");
    let root = tree.root_node();
    let sb = src.as_bytes();
    let lines: Vec<&str> = src.split('\n').collect();
    let mut starts = vec![0usize];
    for (i, b) in sb.iter().enumerate() {
        if *b == b'\n' {
            starts.push(i + 1);
        }
    }
    let text_of = |ln: u32| -> &str { lines.get(ln as usize - 1).copied().unwrap_or("").trim_end_matches('\r') };

    let find = |ln: u32| -> A {
        let text = text_of(ln);
        if text.trim().is_empty() {
            return A::Filler;
        }
        let lead = text.len() - text.trim_start().len();
        let s = starts[ln as usize - 1] + lead;
        let e = starts[ln as usize - 1] + text.trim_end().len();
        let Some(mut n) = root.descendant_for_byte_range(s, e) else { return A::Filler };
        if n.kind() == "comment" {
            let mut sib = n.next_named_sibling();
            while let Some(x) = sib {
                if x.kind() != "comment" {
                    break;
                }
                sib = x.next_named_sibling();
            }
            return match sib {
                Some(x) if is_cand(x.kind()) && !is_type(x.kind()) => A::Weak(x),
                _ => A::Filler,
            };
        }
        if is_container(n.kind()) {
            match root.descendant_for_byte_range(s, s + 1) {
                Some(x) if x.kind() == "comment" => return A::Filler,
                Some(x) => n = x,
                None => return A::Filler,
            }
        }
        let brace = is_brace_only(text);
        if !n.is_named() && matches!(n.kind(), "{" | "}") {
            if let Some(p) = n.parent() {
                n = if is_container(p.kind()) { p.parent().unwrap_or(p) } else { p };
            }
        } else if is_container(n.kind()) {
            return A::Filler;
        }
        let mut c = Some(n);
        while let Some(x) = c {
            if x.is_error() {
                return A::Error;
            }
            if is_cand(x.kind()) {
                if x.has_error() {
                    return A::Error;
                }
                if is_type(x.kind()) {
                    return A::Filler;
                }
                return if brace { A::Weak(x) } else { A::Strong(x) };
            }
            c = x.parent();
        }
        if n.has_error() { A::Error } else { A::Filler }
    };

    let found: Vec<(u32, usize, A)> = changed.iter().map(|&(l, h)| (l, h, find(l))).collect();
    let errored: BTreeSet<usize> = found.iter().filter(|x| matches!(x.2, A::Error)).map(|x| x.1).collect();

    // Strong units keyed by node id.
    let mut nodes: BTreeMap<usize, Node> = BTreeMap::new();
    for (_, h, a) in &found {
        if let (A::Strong(n), false) = (a, errored.contains(h)) {
            nodes.insert(n.id(), *n);
        }
    }
    // Fold into the nearest strong ancestor that is a member or at most 15 lines.
    let mut target: BTreeMap<usize, usize> = BTreeMap::new();
    for (&id, n) in &nodes {
        let mut p = n.parent();
        while let Some(x) = p {
            if nodes.contains_key(&x.id()) && (is_member(x.kind()) || x.end_position().row - x.start_position().row < 15) {
                target.insert(id, x.id());
                break;
            }
            p = x.parent();
        }
    }
    let resolve = |mut id: usize| -> usize {
        while let Some(&t) = target.get(&id) {
            id = t;
        }
        id
    };
    // line -> root id (or None for filler)
    let mut assign: Vec<(u32, usize, Option<usize>)> = vec![];
    for (l, h, a) in &found {
        if errored.contains(h) {
            continue;
        }
        let r = match a {
            A::Strong(n) => Some(resolve(n.id())),
            A::Weak(n) => nodes.contains_key(&n.id()).then(|| resolve(n.id())),
            _ => None,
        };
        assign.push((*l, *h, r));
    }
    // Merge consecutive statement siblings under one parent.
    let roots: BTreeSet<usize> = assign.iter().filter_map(|x| x.2).collect();
    let mut rnodes: Vec<Node> = roots.iter().map(|id| nodes[id]).collect();
    rnodes.sort_by_key(|n| n.start_byte());
    let sib = |n: Node| -> (usize, usize) {
        match n.parent() {
            None => (0, 0),
            Some(p) => {
                let mut c = p.walk();
                let idx = p.named_children(&mut c).filter(|x| x.kind() != "comment").position(|x| x.id() == n.id()).unwrap_or(0);
                (p.id(), idx)
            }
        }
    };
    let mut merged: BTreeMap<usize, usize> = BTreeMap::new();
    let mut prev: Option<(Node, usize)> = None; // (node, group root id)
    for n in &rnodes {
        let mut g = n.id();
        if let Some((pn, pg)) = prev {
            let (pp, pi) = sib(pn);
            let (np, ni) = sib(*n);
            if !is_member(pn.kind()) && !is_member(n.kind()) && pp == np && pi + 1 == ni {
                g = pg;
            }
        }
        merged.insert(n.id(), g);
        prev = Some((*n, g));
    }
    for a in assign.iter_mut() {
        if let Some(r) = a.2 {
            a.2 = Some(merged[&r]);
        }
    }

    // Units: root id -> unit index; fillers follow the previous changed line of their hunk, else the next.
    let mut units: Vec<Unit> = vec![];
    let mut index: BTreeMap<usize, usize> = BTreeMap::new();
    let mut by_hunk: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, a) in assign.iter().enumerate() {
        by_hunk.entry(a.1).or_default().push(i);
    }
    let mut unit_of: Vec<Option<usize>> = vec![None; assign.len()];
    for (i, a) in assign.iter().enumerate() {
        if let Some(r) = a.2 {
            let u = *index.entry(r).or_insert_with(|| {
                let n = nodes[&r];
                units.push(Unit {
                    lines: vec![],
                    hunks: BTreeSet::new(),
                    is_member: is_member(n.kind()),
                    member: if is_member(n.kind()) { member_name(n, sb) } else { None },
                    enclosing: enclosing(n, sb),
                    label: label(n.kind()),
                });
                units.len() - 1
            });
            unit_of[i] = Some(u);
        }
    }
    for idxs in by_hunk.values() {
        let mut fill: Vec<Option<usize>> = idxs.iter().map(|&i| unit_of[i]).collect();
        let mut last = None;
        for f in fill.iter_mut() {
            if f.is_some() {
                last = *f;
            } else {
                *f = last;
            }
        }
        let mut next = None;
        for (k, &i) in idxs.iter().enumerate().rev() {
            if unit_of[i].is_some() {
                next = unit_of[i];
            } else if fill[k].is_none() {
                fill[k] = next;
            }
        }
        if fill.iter().any(|f| f.is_none()) {
            units.push(hunk_unit());
            let u = units.len() - 1;
            for f in fill.iter_mut().filter(|f| f.is_none()) {
                *f = Some(u);
            }
        }
        for (k, &i) in idxs.iter().enumerate() {
            unit_of[i] = fill[k];
        }
    }
    for (i, a) in assign.iter().enumerate() {
        let u = &mut units[unit_of[i].unwrap()];
        u.lines.push(a.0);
        u.hunks.insert(a.1);
    }
    // Hunks with parse errors: one unit each (playbook B2).
    for h in &errored {
        let mut u = hunk_unit();
        u.label = "hunk".into();
        for (l, hh, _) in &found {
            if hh == h {
                u.lines.push(*l);
                u.hunks.insert(*h);
            }
        }
        units.push(u);
    }
    units.retain(|u| !u.lines.is_empty());
    for u in units.iter_mut() {
        u.lines.sort();
    }
    units.sort_by_key(|u| u.lines[0]);
    units
}

pub fn hunk_unit() -> Unit {
    Unit { lines: vec![], hunks: BTreeSet::new(), is_member: false, member: None, enclosing: None, label: "hunk".into() }
}
