//! Pure derivations over contract records: differences from solver rows (D16, D21),
//! replay results (D23), the boundary (D27, D32).

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}
fn list<'a>(rec: &'a Value, key: &str) -> Vec<&'a Value> {
    rec.get(key).and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default()
}
fn num(id: &str) -> u64 {
    id.get(1..).and_then(|n| n.parse().ok()).unwrap_or(u64::MAX)
}

const SOLVER_KINDS: [&str; 7] = ["crash-fixed", "crash", "silent-failure", "null-access", "output-changed", "new-dependency", "public-api"];

/// The fixed kind table over old and new outcome (D21). A row may carry its own `kind` (extra field) from the differ.
pub fn solver_kind(row: &Value) -> String {
    if let Some(k) = row.get("kind").and_then(Value::as_str) {
        if SOLVER_KINDS.contains(&k) {
            return k.to_string();
        }
    }
    if s(row, "property") == "no-new-dependency" {
        return "new-dependency".into();
    }
    let (o, n) = (&row["old"], &row["new"]);
    let (ot, nt) = (s(o, "tag"), s(n, "tag"));
    if ot == "throws" && nt != "throws" {
        "crash-fixed".into()
    } else if nt == "throws" && ot != "throws" {
        if s(n, "text").contains("NullReference") { "null-access".into() } else { "crash".into() }
    } else if ot == "reports" && nt == "nothing" {
        "silent-failure".into()
    } else {
        "output-changed".into()
    }
}

/// Outcome as shown in a difference row: `throws X`, `reports X`, or the text.
pub fn outcome_text(o: &Value) -> String {
    let text = s(o, "text");
    let phrased = ["throws ", "reports ", "calls ", "nothing reported"].iter().any(|p| text.starts_with(p));
    match s(o, "tag") {
        t @ ("throws" | "reports") if !phrased => format!("{t} {text}"),
        _ => text.to_string(),
    }
}

/// Same outcome: same tag and same text (spec: same kind and same old and new outcome).
fn same_outcome(a: &Value, b: &Value) -> bool {
    s(a, "tag") == s(b, "tag") && s(a, "text") == s(b, "text")
}

/// The production block holding a new-side location (by part range or added line).
pub fn block_of(loc: &Value, parts: &Value, blocks: &Value) -> Option<String> {
    let file = s(loc, "file");
    let line = loc.get("line").and_then(Value::as_u64)?;
    let hit = list(parts, "parts").into_iter().find(|p| {
        if s(p, "category") != "production" {
            return false;
        }
        let n = &p["new"];
        let in_range = s(n, "file") == file
            && n.get("line").and_then(Value::as_u64).is_some_and(|a| a <= line)
            && n.get("endLine").and_then(Value::as_u64).is_some_and(|b| line <= b);
        in_range || (s(n, "file") == file && p["addedLines"].as_array().is_some_and(|a| a.iter().any(|x| x.as_u64() == Some(line))))
    })?;
    let pid = s(hit, "id");
    list(blocks, "blocks").into_iter()
        .find(|b| b["parts"].as_array().is_some_and(|a| a.iter().any(|x| x.as_str() == Some(pid))))
        .map(|b| s(b, "id").to_string())
}

/// Result of grouping: new `rows.json`, `differences.json`, and the old→new row ID map.
pub struct Grouped {
    pub rows: Value,
    pub differences: Value,
    pub row_ids: BTreeMap<String, String>,
}

/// Group solver rows by (block, kind, old tag, new tag) into numbered differences (D16, D21),
/// attached to the block of each row's `firstChanged`. `extra` are non-solver differences
/// (file or API comparison) kept as they are, attached by `block` or by `location`.
/// IDs follow the contract: differences by (block number, first row), rows within their difference.
pub fn group_rows(rows: &Value, extra: &[Value], parts: &Value, blocks: &Value) -> Result<Grouped, String> {
    struct G { block: String, kind: String, rows: Vec<usize>, extra: Option<Value>, order: (u64, u64) }
    let input = list(rows, "rows");
    let mut groups: Vec<G> = Vec::new();
    for (i, r) in input.iter().enumerate() {
        let block = block_of(&r["firstChanged"], parts, blocks)
            .ok_or_else(|| format!("rows.json/rows/{i}: firstChanged {} is in no production part", r["firstChanged"]))?;
        let kind = solver_kind(r);
        let key = |g: &G| g.extra.is_none() && g.block == block && g.kind == kind
            && same_outcome(&input[g.rows[0]]["old"], &r["old"]) && same_outcome(&input[g.rows[0]]["new"], &r["new"]);
        match groups.iter_mut().find(|g| key(g)) {
            Some(g) => g.rows.push(i),
            None => groups.push(G { order: (num(&block), i as u64), block, kind, rows: vec![i], extra: None }),
        }
    }
    for (j, d) in extra.iter().enumerate() {
        let block = match d.get("block").and_then(Value::as_str).filter(|b| list(blocks, "blocks").iter().any(|x| s(x, "id") == *b)) {
            Some(b) => b.to_string(),
            None => block_of(&d["location"], parts, blocks).ok_or_else(|| format!("differences.json/differences/{j}: location is in no production part"))?,
        };
        groups.push(G { order: (num(&block), (input.len() + j) as u64), block, kind: s(d, "solverKind").to_string(), rows: vec![], extra: Some(d.clone()) });
    }
    groups.sort_by_key(|g| g.order);
    let (mut out_rows, mut diffs, mut row_ids) = (Vec::new(), Vec::new(), BTreeMap::new());
    for (k, g) in groups.iter().enumerate() {
        let did = format!("d{}", k + 1);
        if let Some(d) = &g.extra {
            let mut d = d.as_object().cloned().unwrap_or_default();
            d.insert("id".into(), json!(did));
            d.insert("number".into(), json!(k + 1));
            d.insert("block".into(), json!(g.block));
            d.insert("rows".into(), json!([]));
            diffs.push(Value::Object(d));
            continue;
        }
        let mut ids = Vec::new();
        for &i in &g.rows {
            let rid = format!("r{}", out_rows.len() + 1);
            let mut r: Map<String, Value> = input[i].as_object().cloned().unwrap_or_default();
            if let Some(old) = r.get("id").and_then(Value::as_str) {
                row_ids.insert(old.to_string(), rid.clone());
            }
            r.insert("id".into(), json!(rid));
            r.insert("difference".into(), json!(did));
            ids.push(rid);
            out_rows.push(Value::Object(r));
        }
        let first = input[g.rows[0]];
        diffs.push(json!({
            "id": did, "number": k + 1, "kind": g.kind, "solverKind": g.kind, "checker": "solver",
            "rows": ids, "block": g.block, "condition": s(first, "condition"),
            "old": outcome_text(&first["old"]), "new": outcome_text(&first["new"]),
            "location": first["firstChanged"],
        }));
    }
    Ok(Grouped {
        rows: json!({"provenance": "pipeline", "rows": out_rows}),
        differences: json!({"provenance": "pipeline", "differences": diffs}),
        row_ids,
    })
}

/// Rewrite row IDs inside `scope.assumptions[].rows` and `lean.rows[].row` after regrouping.
pub fn remap_rows(rec: &mut Value, ids: &BTreeMap<String, String>) {
    let map = |v: &mut Value| {
        if let Some(n) = v.as_str().and_then(|x| ids.get(x)) {
            *v = json!(n);
        }
    };
    if let Some(a) = rec.get_mut("assumptions").and_then(Value::as_array_mut) {
        for x in a {
            if let Some(rs) = x.get_mut("rows").and_then(Value::as_array_mut) {
                rs.iter_mut().for_each(map);
            }
        }
    }
    if let Some(a) = rec.get_mut("rows").and_then(Value::as_array_mut) {
        for x in a {
            if let Some(r) = x.get_mut("row") {
                map(r);
            }
        }
    }
}

/// Apply replay results (D21, D23): every difference gets one replay entry (missing ones read
/// `not-replayed` with no input); a seen-live round trip that does not preserve the value turns the kind into `data-loss`.
pub fn apply_replays(differences: &mut Value, replays: &Value) -> Value {
    let given: BTreeMap<String, &Value> = list(replays, "replays").into_iter().map(|r| (s(r, "difference").to_string(), r)).collect();
    let mut out = Vec::new();
    if let Some(ds) = differences.get_mut("differences").and_then(Value::as_array_mut) {
        for d in ds {
            let id = s(d, "id").to_string();
            let r = match given.get(&id) {
                Some(r) => (*r).clone(),
                None => json!({"difference": id, "input": null, "old": null, "new": null, "state": "not-replayed", "roundTrip": null}),
            };
            let rt = &r["roundTrip"];
            if s(&r, "state") == "seen-live" && rt.is_object() && s(rt, "sent") != s(rt, "received") {
                d["kind"] = json!("data-loss");
            }
            out.push(r);
        }
    }
    json!({"provenance": "pipeline", "replays": out})
}

/// Boundary placement per production block (D32): declared → `declared-region`; not linked, or not
/// declared with a difference or a not-modelled line → `outside`; otherwise `neither`.
pub fn place_links(links: &mut Value, differences: &Value, scope: &Value, parts: &Value, blocks: &Value) {
    let has_diff = |b: &str| list(differences, "differences").iter().any(|d| s(d, "block") == b);
    let not_modelled = |b: &str| list(scope, "notModelled").iter().any(|n| block_of(&n["location"], parts, blocks).as_deref() == Some(b));
    if let Some(ls) = links.get_mut("links").and_then(Value::as_array_mut) {
        for l in ls {
            let b = s(l, "block").to_string();
            let p = match s(l, "state") {
                "declared" => "declared-region",
                "not-linked" => "outside",
                _ if has_diff(&b) || not_modelled(&b) => "outside",
                _ => "neither",
            };
            l["placement"] = json!(p);
        }
    }
}

/// Every production block must have exactly one links entry (evidence-bundle, Block states).
pub fn check_links(links: &Value, blocks: &Value) -> Result<(), String> {
    for b in list(blocks, "blocks").into_iter().filter(|b| s(b, "category") == "production") {
        let n = list(links, "links").iter().filter(|l| s(l, "block") == s(b, "id")).count();
        if n != 1 {
            return Err(format!("links.json: {} entries for block {}", n, s(b, "id")));
        }
    }
    Ok(())
}

fn display_symbol(sym: &str) -> &str {
    match sym.split_once(':') {
        Some((p, rest)) if p.len() == 1 => rest,
        _ => sym,
    }
}

/// `public-api` differences (checker `api-comparison`, D21, D33) from the worker's `facts`
/// (`{facts: [...]}`, or the whole reply with `result.facts`). Kinds used: `public-type`, `public-member`,
/// `changed-signature`; a public member inside a new public type's range is covered by the type.
/// Facts with no location or outside every production part are skipped.
pub fn api_differences(facts: &Value, parts: &Value, blocks: &Value) -> Vec<Value> {
    let all = facts.get("result").unwrap_or(facts);
    let fs = list(all, "facts");
    let types: Vec<&Value> = fs.iter().copied().filter(|f| s(f, "kind") == "public-type").collect();
    let type_names: Vec<&str> = types.iter().map(|t| display_symbol(s(t, "symbol"))).collect();
    let inside = |f: &Value| {
        let (l, file) = (f["location"]["line"].as_u64().unwrap_or(0), s(&f["location"], "file"));
        let by_span = types.iter().any(|t| s(&t["location"], "file") == file
            && t["location"]["line"].as_u64().is_some_and(|a| a <= l) && t["location"]["endLine"].as_u64().is_some_and(|b| l <= b));
        // By name: the reply's `containingType` if any, else the member symbol up to its last `.` (parameters cut).
        let sym = display_symbol(s(f, "symbol"));
        let sym = sym.split('(').next().unwrap_or(sym);
        let owner = f["container"].as_str().or(f["containingType"].as_str()).map(display_symbol)
            .or_else(|| sym.rsplit_once('.').map(|(o, _)| o));
        by_span || owner.is_some_and(|o| type_names.contains(&o))
    };
    let mut out = Vec::new();
    for f in fs {
        let cond = match s(f, "kind") {
            "public-type" => "new public type",
            // Members of a new diagnostic (descriptor, `…Id` const) are covered by its `new-diagnostic` fact.
            "public-member" if !inside(f) && f["diagnostic"].as_str().is_none() => "new public member",
            "changed-signature" => "changed signature",
            "new-diagnostic" => "new diagnostic",
            _ => continue,
        };
        let Some(block) = block_of(&f["location"], parts, blocks) else { continue };
        if cond == "new diagnostic" {
            let id = f["diagnosticId"].as_str().or(f["new"].as_str()).unwrap_or_else(|| display_symbol(s(f, "symbol")));
            out.push(json!({"kind": "public-api", "solverKind": "public-api", "checker": "api-comparison", "rows": [],
                "block": block, "condition": format!("new diagnostic {id}"), "old": "absent", "new": id, "location": f["location"]}));
            continue;
        }
        let sym = display_symbol(s(f, "symbol")).to_string();
        let old = f["old"].as_str().map(String::from).unwrap_or_else(|| "absent".into());
        let new = f["new"].as_str().filter(|_| cond == "changed signature").map(String::from).unwrap_or(sym);
        out.push(json!({"kind": "public-api", "solverKind": "public-api", "checker": "api-comparison", "rows": [],
            "block": block, "condition": cond, "old": old, "new": new, "location": f["location"]}));
    }
    out
}

/// `output-changed` differences (checker `file-comparison`, D26) from the test contract's
/// `snapshots.json` (`{snapshots: [{part, kind, checker, file, location, before, after, block?}]}`).
/// Entries with `before` null (new snapshot file) or equal texts are skipped. The block is the entry's
/// `block` when given, else the first production block with a part in a `.cs` file, else the first production block.
pub fn snapshot_differences(snaps: &Value, parts: &Value, blocks: &Value) -> Vec<Value> {
    let prod: Vec<&Value> = list(blocks, "blocks").into_iter().filter(|b| s(b, "category") == "production").collect();
    let cs_part = |pid: &str| list(parts, "parts").iter().any(|p| s(p, "id") == pid
        && (s(&p["new"], "file").ends_with(".cs") || s(&p["old"], "file").ends_with(".cs")));
    let fallback = prod.iter().find(|b| b["parts"].as_array().is_some_and(|a| a.iter().any(|x| cs_part(x.as_str().unwrap_or("")))))
        .or(prod.first()).map(|b| s(b, "id").to_string());
    let mut out = Vec::new();
    for e in list(snaps, "snapshots") {
        let (Some(before), Some(after)) = (e["before"].as_str(), e["after"].as_str()) else { continue };
        if before == after || !e["location"].is_object() {
            continue;
        }
        let given = e["block"].as_str().filter(|b| prod.iter().any(|x| s(x, "id") == *b)).map(String::from);
        let rule = if given.is_some() { "test-contract" } else { "first-cs-block" };
        let Some(block) = given.or(fallback.clone()) else { continue };
        let file = s(e, "file");
        let name = file.rsplit('/').nth(1).unwrap_or(file);
        // `blockRule` is an extra field (readers ignore extra fields): how the block was chosen.
        out.push(json!({"kind": "output-changed", "solverKind": "output-changed", "checker": "file-comparison", "rows": [],
            "block": block, "blockRule": rule, "condition": format!("snapshot {name}"), "old": before, "new": after, "location": e["location"]}));
    }
    out
}

/// Append differences after the existing ones, keeping existing IDs (`d<n+1>`...).
pub fn append_differences(differences: &mut Value, new: Vec<Value>) -> Vec<String> {
    if !differences["differences"].is_array() {
        *differences = json!({"provenance": "pipeline", "differences": []});
    }
    let arr = differences["differences"].as_array_mut().unwrap();
    let mut ids = Vec::new();
    for mut d in new {
        let n = arr.len() + 1;
        d["id"] = json!(format!("d{n}"));
        d["number"] = json!(n);
        ids.push(format!("d{n}"));
        arr.push(d);
    }
    ids
}
