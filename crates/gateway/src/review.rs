//! Approval gates and the composed review body. Pure functions of bundle records and session;
//! no model call anywhere (D1, D41, D47, D48).
use serde_json::Value;
use std::collections::BTreeMap;

/// A bundle's records by name (`bundle`, `questions`, `levels`, ...); absent records are missing keys.
pub type Records = BTreeMap<String, Value>;

fn arr<'a>(r: &'a Records, rec: &str, key: &str) -> Vec<&'a Value> {
    r.get(rec).and_then(|v| v.get(key)).and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default()
}
fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}
fn strs(v: &Value, k: &str) -> Vec<String> {
    v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

/// One unmet gate and the items (sections, questions, blocks) that keep it open.
#[derive(Debug, Clone, PartialEq)]
pub struct Unmet {
    pub gate: &'static str,
    pub items: Vec<String>,
}

fn answers(session: &Value) -> BTreeMap<String, String> {
    session
        .get("answers")
        .and_then(Value::as_object)
        .map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string()))).collect())
        .unwrap_or_default()
}

/// Questions of this bundle answered `return`. Answers to ids not in `questions.json` are ignored.
pub fn returned(r: &Records, session: &Value) -> Vec<Value> {
    let a = answers(session);
    arr(r, "questions", "questions").into_iter().filter(|q| a.get(s(q, "id")).map(String::as_str) == Some("return")).cloned().collect()
}

/// Every unmet approval gate, in the order of the spec. Empty means approval may be submitted.
pub fn approval_gates(r: &Records, session: &Value) -> Vec<Unmet> {
    let mut out = Vec::new();
    let opened = strs(session, "opened");
    let running: Vec<String> = r
        .get("bundle")
        .and_then(|b| b.get("sections"))
        .and_then(Value::as_object)
        .map(|m| m.iter().filter(|(_, v)| s(v, "state") == "running").map(|(k, _)| k.clone()).collect())
        .unwrap_or_default();
    if !running.is_empty() {
        out.push(Unmet { gate: "section-running", items: running });
    }
    let a = answers(session);
    let open: Vec<String> = arr(r, "questions", "questions")
        .into_iter()
        .map(|q| s(q, "id").to_string())
        .filter(|id| a.get(id).map(String::as_str) != Some("accept"))
        .collect();
    if !open.is_empty() {
        out.push(Unmet { gate: "question-unanswered", items: open });
    }
    let levels = arr(r, "levels", "levels");
    let has_level1 = levels.iter().any(|l| l.get("level").and_then(Value::as_u64) == Some(1));
    if has_level1 {
        let spot = session.get("spotCheck").and_then(Value::as_str);
        let ok = spot.map(|b| opened.iter().any(|o| o == b)).unwrap_or(false);
        if !ok {
            out.push(Unmet { gate: "spot-check", items: spot.map(|b| vec![b.to_string()]).unwrap_or_default() });
        }
    }
    let full = r.get("mode").map(|m| s(m, "mode") == "full").unwrap_or(true);
    if full {
        let missing: Vec<String> = levels
            .iter()
            .filter(|l| l.get("level").and_then(Value::as_u64) == Some(3))
            .map(|l| s(l, "block").to_string())
            .filter(|b| !opened.contains(b))
            .collect();
        if !missing.is_empty() {
            out.push(Unmet { gate: "level-3-unopened", items: missing });
        }
    }
    out
}

/// `File.cs:line` of a block's first part (new side, else old side).
pub fn block_location(r: &Records, block: &str) -> Option<String> {
    let b = arr(r, "blocks", "blocks").into_iter().find(|b| s(b, "id") == block)?;
    let parts = arr(r, "parts", "parts");
    for pid in strs(b, "parts") {
        let Some(p) = parts.iter().find(|p| s(p, "id") == pid) else { continue };
        for side in ["new", "old"] {
            if let Some(loc) = p.get(side).filter(|v| v.is_object()) {
                let file = s(loc, "file").rsplit('/').next().unwrap_or("");
                if let Some(line) = loc.get("line").and_then(Value::as_u64) {
                    return Some(format!("{file}:{line}"));
                }
            }
        }
    }
    None
}

fn list(v: Vec<String>) -> String {
    if v.is_empty() {
        "none".into()
    } else {
        v.join(", ")
    }
}

/// The review body: requirement; looked at; tried; relied on machine checks; not opened;
/// not checked; answers; risk. `risk` is the reviewer's sentence, copied verbatim.
pub fn compose_body(r: &Records, session: &Value, risk: &str) -> String {
    let bundle = r.get("bundle").cloned().unwrap_or(Value::Null);
    let requirement = bundle.get("requirement").and_then(|q| q.get("id")).and_then(Value::as_str).unwrap_or("none");
    let opened = strs(session, "opened");
    let mut looked = opened.clone();
    if let Some(spot) = session.get("spotCheck").and_then(Value::as_str) {
        if opened.iter().any(|o| o == spot) {
            if let Some(i) = looked.iter().position(|o| o == spot) {
                looked[i] = format!("{spot} (spot-check)");
            }
        }
    }
    let tried: Vec<String> = session
        .get("runs")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|run| match run.get("difference").and_then(Value::as_str) {
                    Some(d) => format!("difference {}", d.trim_start_matches('d')),
                    None => s(run, "input").to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    // Machine checks: differences whose rows are all checked by Lean, and tests run.
    let lean_checked: Vec<String> =
        arr(r, "lean", "rows").into_iter().filter(|x| s(x, "state") == "checked").map(|x| s(x, "row").to_string()).collect();
    let diffs = arr(r, "differences", "differences");
    let checked = diffs.iter().filter(|d| {
        let rows = strs(d, "rows");
        !rows.is_empty() && rows.iter().all(|row| lean_checked.contains(row))
    });
    let mut relied = vec![format!("{} differences checked by Lean", checked.count())];
    if diffs.len() > 0 {
        relied[0] = format!("{} differences, {}", diffs.len(), relied[0]);
    }
    let tests = arr(r, "tests", "tests");
    if !tests.is_empty() {
        relied.push(format!("{} tests", tests.len()));
    }
    let not_opened: Vec<String> =
        arr(r, "blocks", "blocks").into_iter().map(|b| s(b, "id").to_string()).filter(|b| !opened.contains(b)).collect();
    let scope = r.get("scope").cloned().unwrap_or(Value::Null);
    let not_modelled: u64 = scope
        .get("notModelled")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|n| {
                    let l = n.get("location").unwrap_or(&Value::Null);
                    let (a, b) = (l.get("line").and_then(Value::as_u64).unwrap_or(0), l.get("endLine").and_then(Value::as_u64).unwrap_or(0));
                    if b >= a && a > 0 {
                        b - a + 1
                    } else {
                        1
                    }
                })
                .sum()
        })
        .unwrap_or(0);
    let assumptions: Vec<String> =
        scope.get("assumptions").and_then(Value::as_array).map(|a| a.iter().map(|x| s(x, "contract").to_string()).collect()).unwrap_or_default();
    let mut not_checked = vec![format!("{not_modelled} lines not modelled")];
    let not_run: Vec<String> = bundle
        .get("sections")
        .and_then(Value::as_object)
        .map(|m| m.iter().filter(|(_, v)| s(v, "state") != "ready").map(|(k, v)| format!("{k} {}", s(v, "state"))).collect())
        .unwrap_or_default();
    if !not_run.is_empty() {
        not_checked.push(format!("checks not run: {}", not_run.join(", ")));
    }
    let lean_open = arr(r, "lean", "rows").into_iter().filter(|x| s(x, "state") != "checked").count();
    if lean_open > 0 {
        not_checked.push(format!("{lean_open} rows not checked by Lean"));
    }
    not_checked.push(if assumptions.is_empty() {
        "0 assumptions".into()
    } else {
        format!("{} assumptions ({})", assumptions.len(), assumptions.join(", "))
    });
    let a = answers(session);
    let answer_list: Vec<String> = arr(r, "questions", "questions")
        .into_iter()
        .map(|q| {
            let id = s(q, "id");
            match a.get(id).map(String::as_str) {
                Some("return") => {
                    let b = s(q, "block");
                    match block_location(r, b) {
                        Some(loc) => format!("{id} return ({b} {loc})"),
                        None => format!("{id} return ({b})"),
                    }
                }
                Some(v) => format!("{id} {v}"),
                None => format!("{id} unanswered"),
            }
        })
        .collect();
    [
        format!("Requirement: {requirement}"),
        format!("Looked at: {}", list(looked)),
        format!("Tried: {}", list(tried)),
        format!("Relied on machine checks: {}", relied.join("; ")),
        format!("Not opened: {}", list(not_opened)),
        format!("Not checked: {}", not_checked.join("; ")),
        format!("Answers: {}", list(answer_list)),
        format!("Risk: {risk}"),
    ]
    .join("\n")
}
