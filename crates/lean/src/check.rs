//! Runs `lean --json` on one file and decides per theorem: no error and no `hasSorry`
//! in its span, and `#print axioms` lists nothing beyond the allowed three. Wall-clock
//! and memory limits kill the process; a killed run accepts nothing. D4, D17, L3, L4.

use crate::emit::Span;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const ALLOWED_AXIOMS: &[&str] = &["propext", "Quot.sound", "Classical.choice"];

/// One Lean process at a time from this crate.
static LEAN_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub secs_per_theorem: f64,
    pub max_rss_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { secs_per_theorem: 30.0, max_rss_bytes: 1536 * 1024 * 1024 }
    }
}

#[derive(Clone, Debug)]
pub struct Verdict {
    pub accepted: bool,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct Run {
    pub verdicts: BTreeMap<String, Verdict>,
    pub seconds: f64,
    pub killed: Option<String>,
    pub messages: Vec<Value>,
}

pub fn lean_version() -> Result<String, String> {
    let o = Command::new("lean").arg("--version").output().map_err(|e| format!("lean: {e}"))?;
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn rss_bytes(pid: u32) -> u64 {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|k| k.parse::<u64>().ok())
        })
        .map(|k| k * 1024)
        .unwrap_or(0)
}

/// Parse the axiom list of a `#print axioms` message; None if it is not one.
pub fn axioms_of(data: &str) -> Option<Vec<String>> {
    if data.contains("does not depend on any axioms") {
        return Some(vec![]);
    }
    let i = data.find("depends on axioms: [")?;
    let rest = &data[i + "depends on axioms: [".len()..];
    let list = &rest[..rest.find(']')?];
    Some(list.split(',').map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect())
}

pub fn run_lean(file: &Path, spans: &[Span], limits: Limits) -> Run {
    let _g = LEAN_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let budget = limits.secs_per_theorem * spans.len().max(1) as f64;
    let out_path = file.with_extension("json.out");
    let fail_all = |why: &str, seconds: f64, messages: Vec<Value>| Run {
        verdicts: spans.iter().map(|s| (s.name.clone(), Verdict { accepted: false, reason: why.into() })).collect(),
        seconds,
        killed: Some(why.into()),
        messages,
    };
    let out = match std::fs::File::create(&out_path) {
        Ok(f) => f,
        Err(e) => return fail_all(&format!("output file: {e}"), 0.0, vec![]),
    };
    let dir = file.parent().unwrap_or(Path::new("."));
    let start = Instant::now();
    let mut child = match Command::new("lean")
        .arg("--json")
        .arg(file.file_name().unwrap())
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return fail_all(&format!("lean: {e}"), 0.0, vec![]),
    };
    let mut killed = None;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => {
                killed = Some(format!("wait: {e}"));
                break;
            }
        }
        let el = start.elapsed().as_secs_f64();
        if el > budget {
            killed = Some(format!("over {budget:.0} s"));
        } else if rss_bytes(child.id()) > limits.max_rss_bytes {
            killed = Some(format!("over {} MB", limits.max_rss_bytes / (1024 * 1024)));
        }
        if killed.is_some() {
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let seconds = (start.elapsed().as_secs_f64() * 100.0).round() / 100.0;
    let messages: Vec<Value> = std::fs::read_to_string(&out_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    if let Some(k) = killed {
        return fail_all(&k, seconds, messages);
    }
    judge(spans, messages, seconds)
}

pub fn judge(spans: &[Span], messages: Vec<Value>, seconds: f64) -> Run {
    let mut verdicts: BTreeMap<String, Verdict> = BTreeMap::new();
    let mut axioms_seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut outside_error = None;
    for m in &messages {
        let line = m.pointer("/pos/line").and_then(Value::as_u64).unwrap_or(0) as usize;
        let sev = m.get("severity").and_then(Value::as_str).unwrap_or("");
        let kind = m.get("kind").and_then(Value::as_str).unwrap_or("");
        let data = m.get("data").and_then(Value::as_str).unwrap_or("");
        let span = spans.iter().find(|s| s.start <= line && line <= s.end);
        let bad = if sev == "error" {
            Some(format!("error: {}", data.lines().next().unwrap_or("")))
        } else if kind == "hasSorry" || data.contains("declaration uses `sorry`") {
            Some("sorry".to_string())
        } else {
            None
        };
        match (span, bad) {
            (Some(s), Some(b)) => {
                verdicts.entry(s.name.clone()).or_insert(Verdict { accepted: false, reason: b });
            }
            (None, Some(b)) => {
                outside_error.get_or_insert(b);
            }
            (Some(s), None) => {
                if let Some(ax) = axioms_of(data) {
                    axioms_seen.insert(s.name.clone(), ax);
                }
            }
            (None, None) => {}
        }
    }
    for s in spans {
        if let Some(e) = &outside_error {
            verdicts.insert(s.name.clone(), Verdict { accepted: false, reason: format!("file: {e}") });
            continue;
        }
        if verdicts.contains_key(&s.name) {
            continue;
        }
        let v = match axioms_seen.get(&s.name) {
            None => Verdict { accepted: false, reason: "no axioms report".into() },
            Some(ax) => match ax.iter().find(|a| !ALLOWED_AXIOMS.contains(&a.as_str())) {
                Some(a) => Verdict { accepted: false, reason: format!("axiom {a}") },
                None => Verdict { accepted: true, reason: String::new() },
            },
        };
        verdicts.insert(s.name.clone(), v);
    }
    Run { verdicts, seconds, killed: None, messages }
}
