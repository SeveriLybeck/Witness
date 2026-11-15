use model::*;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn fixtures() -> PathBuf {
    root().join("contracts/fixtures")
}
fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("model-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}
fn fixture_recording() -> Value {
    let dir = fixtures().join("recordings");
    let f = fs::read_dir(dir).unwrap().next().unwrap().unwrap().path();
    serde_json::from_str(&fs::read_to_string(f).unwrap()).unwrap()
}

#[test]
fn fallbacks_line1_is_contract_hash() {
    let out = std::process::Command::new("git")
        .args(["log", "-1", "--format=%H", "--", "contracts/"])
        .current_dir(root())
        .output();
    let Ok(out) = out else { return };
    let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if hash.is_empty() {
        return;
    }
    let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("FALLBACKS.md")).unwrap();
    assert_eq!(text.lines().next().unwrap(), hash);
}

#[test]
fn fixture_recording_key_matches_file_name() {
    let rec = fixture_recording();
    let key = recording_key(rec["purpose"].as_str().unwrap(), rec["prompt"].as_str().unwrap(), &rec["schema"]);
    let r = Recorder::replay(fixtures().join("recordings"));
    assert!(r.path_for(&key).exists());
    let (k, resp) = r.call(rec["purpose"].as_str().unwrap(), rec["prompt"].as_str().unwrap(), &rec["schema"]).unwrap();
    assert_eq!(k, key);
    assert_eq!(resp, rec["response"]);
}

#[test]
fn replay_missing_is_no_recording() {
    let r = Recorder::replay(tmp("missing"));
    match r.call("ai-review", "anything", &json!({})) {
        Err(ModelError::NoRecording(k)) => assert_eq!(k.len(), 64),
        other => panic!("expected no recording, got {other:?}"),
    }
    // The review of a bundle with no recording is the empty "no AI review" record.
    let v = review(&fixtures().join("2283"), &r).unwrap();
    assert!(v["recording"].is_null());
    assert_eq!(v["statements"], json!([]));
}

#[cfg(unix)]
#[test]
fn live_records_then_replays() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tmp("live");
    let cli = dir.join("fake-claude");
    let marker = dir.join("calls");
    fs::write(
        &cli,
        format!(
            "#!/bin/sh\ncat >/dev/null\necho x >> '{}'\necho '{{\"type\":\"result\",\"structured_output\":{{\"statements\":[{{\"heading\":\"matches\",\"text\":\"Fixes the crash.\",\"cites\":[\"d3\"]}}]}}}}'\n",
            marker.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
    let recs = dir.join("recordings");
    let first = review(&fixtures().join("2283"), &Recorder::live(&recs, &cli)).unwrap();
    let second = review(&fixtures().join("2283"), &Recorder::replay(&recs)).unwrap();
    assert_eq!(first, second);
    assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
    let key = first["recording"].as_str().unwrap();
    let rec: Value = serde_json::from_str(&fs::read_to_string(recs.join(format!("{key}.json"))).unwrap()).unwrap();
    assert_eq!(rec["purpose"], "ai-review");
    assert_eq!(first["statements"][0]["cites"], json!(["d3"]));
}

#[cfg(unix)]
#[test]
fn binary_records_live_then_replays_with_real_prompt() {
    use std::os::unix::fs::PermissionsExt;
    let bin = env!("CARGO_BIN_EXE_model-review");
    let dir = tmp("bin");
    let cli = dir.join("fake-claude");
    fs::write(&cli, "#!/bin/sh\ncat >/dev/null\necho '{\"type\":\"result\",\"structured_output\":{\"statements\":[{\"heading\":\"matches\",\"text\":\"Fixes the crash.\",\"cites\":[\"d3\"]},{\"heading\":\"matches\",\"text\":\"Cites d4.\",\"cites\":[\"d4\"]}]}}'\n").unwrap();
    fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
    let recs = dir.join("recordings");
    let bundle = fixtures().join("2283");
    let run = |stage: &Path, live: bool| {
        let mut c = std::process::Command::new(bin);
        c.arg("--recordings").arg(&recs);
        if live {
            c.arg("--live").arg(&cli);
        }
        c.arg(stage).arg(&bundle).args(["repo", "old", "new"]);
        assert!(c.status().unwrap().success());
        let v: Value = serde_json::from_str(&fs::read_to_string(stage.join("ai-review.json")).unwrap()).unwrap();
        v
    };
    // Empty recordings, replay only: "no AI review".
    let none = run(&dir.join("s0"), false);
    assert!(none["recording"].is_null());
    let first = run(&dir.join("s1"), true);
    fs::remove_file(&cli).unwrap(); // replay must not need the CLI
    let second = run(&dir.join("s2"), false);
    assert_eq!(first, second);
    assert!(second["recording"].is_string());
    assert_eq!(second["dropped"].as_array().unwrap().len(), 1);
}

#[test]
fn prompt_has_facts_not_code() {
    let b = Bundle::load(&fixtures().join("2283"));
    let p = review_prompt(&b);
    assert!(p.contains(b.bundle["description"].as_str().unwrap()));
    for k in ["kind=data-loss", "kind=crash-fixed"] {
        assert!(p.contains(k));
    }
    for d in ["d1", "d2", "d3"] {
        assert!(p.contains(&format!("{d} kind=")));
    }
    // Spot-check: no source text of the change (member bodies) appears.
    assert!(!p.contains("ReportIfLocatedInCompilation("));
    assert!(!p.contains("reportDiagnostic"));
}

#[test]
fn fake_citation_and_banned_word_dropped_on_2283() {
    let b = Bundle::load(&fixtures().join("2283"));
    let mut resp = fixture_recording()["response"].clone();
    let st = resp["statements"].as_array_mut().unwrap();
    st.push(json!({"heading": "not-in-description", "text": "Changes a fourth lookup.", "cites": ["d4"]}));
    st.push(json!({"heading": "matches", "text": "The fix is safe", "cites": ["d1"]}));
    st.push(json!({"heading": "matches", "text": "No citation.", "cites": []}));
    let v = check_review(&resp, &b, Some("k"));
    let dropped = v["dropped"].as_array().unwrap();
    assert_eq!(dropped.len(), 3);
    assert!(dropped.iter().any(|d| d["cites"] == json!(["d4"])));
    let texts: Vec<&str> = v["statements"].as_array().unwrap().iter().map(|s| s["text"].as_str().unwrap()).collect();
    assert!(!texts.contains(&"The fix is safe"));
    assert!(!texts.contains(&"Changes a fourth lookup."));
}

#[test]
fn headings_order_and_questions_on_2283() {
    let b = Bundle::load(&fixtures().join("2283"));
    let v = check_review(&fixture_recording()["response"], &b, None);
    let heads: Vec<&str> = v["statements"].as_array().unwrap().iter().map(|s| s["heading"].as_str().unwrap()).collect();
    let mut seen: Vec<&str> = heads.clone();
    seen.dedup();
    assert_eq!(seen, ["matches", "not-in-description", "left-to-you"]);
    let left: Vec<&Value> = v["statements"].as_array().unwrap().iter().filter(|s| s["heading"] == "left-to-you").collect();
    for q in b.questions() {
        assert!(left.iter().any(|s| s["cites"].as_array().unwrap().contains(&q["id"])));
    }
    assert!(v["dropped"].as_array().unwrap().is_empty());
    // A question the model left out is still listed.
    let mut resp = fixture_recording()["response"].clone();
    resp["statements"].as_array_mut().unwrap().retain(|s| s["heading"] != "left-to-you");
    let v2 = check_review(&resp, &b, None);
    assert_eq!(v2["statements"].as_array().unwrap().iter().filter(|s| s["heading"] == "left-to-you").count(), 2);
}

#[test]
fn only_unresolved_cites_are_dropped() {
    let b = Bundle::load(&fixtures().join("2283"));
    let mut resp = json!({"statements": []});
    for c in ["reproduction", "preset:d1", "declared", "TypeCollector.cs:985", "d9"] {
        resp["statements"].as_array_mut().unwrap().push(json!({"heading": "matches", "text": format!("cites {c}"), "cites": ["d1", c]}));
    }
    resp["statements"].as_array_mut().unwrap().push(json!({"heading": "matches", "text": "ok", "cites": ["d1", "b2", "p1"]}));
    let v = check_review(&resp, &b, None);
    let dropped = v["dropped"].as_array().unwrap();
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0]["cites"], json!(["d1", "d9"]));
    assert_eq!(v["statements"].as_array().unwrap().iter().filter(|s| s["heading"] == "matches").count(), 5);
    assert_valid(&fixtures().join("2283"), &v);
}

fn assert_valid(bundle: &Path, review: &Value) {
    // One directory per call: tests run in parallel and several validate the same fixture.
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let d = tmp(&format!("valid-{}-{n}", bundle.parent().unwrap().file_name().unwrap().to_string_lossy()));
    for e in fs::read_dir(bundle).unwrap().flatten() {
        if e.path().is_file() && e.file_name() != "ai-review.json" {
            fs::copy(e.path(), d.join(e.file_name())).unwrap();
        }
    }
    fs::write(d.join("ai-review.json"), review.to_string()).unwrap();
    contracts::validate_bundle(&d, &contracts::Options::default()).unwrap();
}

#[test]
fn real_2283_recording_passes_contract() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let bundle = data.join("2283/bundle");
    let v = review(&bundle, &Recorder::replay(data.join("recordings"))).unwrap();
    assert_eq!(v["recording"], REAL_2283);
    assert_valid(&bundle, &v);
    let st = v["statements"].as_array().unwrap();
    let dropped = v["dropped"].as_array().unwrap().len();
    println!("kept {} dropped {dropped}", st.len());
    assert_eq!((st.len(), dropped), (KEPT_2283, DROPPED_2283));
    // The short check: a bounded number of statements, each one line, a line per heading.
    assert!(st.len() <= MAX_STATEMENTS);
    for (i, h) in HEADINGS.iter().enumerate() {
        let of: Vec<&Value> = st.iter().filter(|s| s["heading"] == *h).collect();
        assert!(of.len() <= MAX_PER_HEADING[i], "{h}: {}", of.len());
        if !of.is_empty() {
            let line = v["summaries"][*h].as_str().unwrap_or_else(|| panic!("no summary for {h}"));
            assert!(words(line) <= LINE_WORDS, "{h} summary: {line}");
        }
    }
    for s in st {
        let text = s["text"].as_str().unwrap();
        assert!(words(text) <= LINE_WORDS, "{} words: {text}", words(text));
    }
    let total: usize = st.iter().map(|s| words(s["text"].as_str().unwrap())).sum();
    assert!(total <= 70, "{total} words in all statement lines");
    // Related evidence is grouped: fewer statements than differences, and one cites several.
    assert!(st.len() < Bundle::load(&bundle).differences().len());
    assert!(st.iter().any(|s| s["cites"].as_array().unwrap().iter().filter(|c| c.as_str().unwrap().starts_with('d')).count() >= 3));
}
fn words(s: &str) -> usize {
    s.split_whitespace().count()
}
/// Real output of the short-check prompt on the 2283 bundle copy under tests/data (one live call).
const REAL_2283: &str = "ba0f0038db55c1fe4b16573a1c80d5b5a58adae7159f2849c2ab71f626fc2e3a";
const KEPT_2283: usize = 6;
const DROPPED_2283: usize = 0;

#[test]
fn fixture_2283_response_passes_check() {
    // The contract's fixture recording predates the short-check prompt, so it no longer replays by the
    // key of `review_prompt` (contracts/ owns it). Its response still goes through the same check.
    // Since contracts round 3 the fixture recording is keyed on the frozen prompt again: it replays by key.
    let v = review(&fixtures().join("2283"), &Recorder::replay(fixtures().join("recordings"))).unwrap();
    let key = v["recording"].as_str().expect("fixture recording replays by key");
    assert!(key.starts_with("c979afb6"));
    assert!(fixtures().join(format!("recordings/{key}.json")).exists());
    assert_eq!(v["statements"].as_array().unwrap().len(), 5);
    assert!(v["dropped"].as_array().unwrap().is_empty());
    assert_eq!(v["summaries"].as_object().unwrap().len(), 3);
    assert!(v["statements"].as_array().unwrap().iter().any(|s| s["detail"].is_string()));
    assert_valid(&fixtures().join("2283"), &v);
}

#[test]
fn prompt_keys_to_the_real_recording() {
    // Pins prompt and schema bytes: the 2283 copy under tests/data must key to its recording.
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let key = recording_key(PURPOSE_REVIEW, &review_prompt(&Bundle::load(&data.join("2283/bundle"))), &review_schema());
    assert_eq!(key, REAL_2283);
}

#[test]
fn prompt_asks_for_a_short_check() {
    let p = review_prompt(&Bundle::load(&fixtures().join("2283")));
    assert!(p.contains(&format!("At most {MAX_STATEMENTS} statements")));
    assert!(p.contains(&format!("at most {LINE_WORDS} words")));
    assert!(p.contains("ONE statement that cites all of them"));
    assert!(p.contains("State only what the evidence below shows"));
    let schema = review_schema();
    assert_eq!(schema["properties"]["statements"]["maxItems"], MAX_STATEMENTS);
    assert!(schema["properties"]["summaries"]["properties"]["not-in-description"].is_object());
}

#[test]
fn summaries_and_detail_are_checked_and_kept() {
    let b = Bundle::load(&fixtures().join("2283"));
    let resp = json!({
        "summaries": {"matches": "Crash fixed.", "not-in-description": "This is safe.", "described-not-found": "Nothing cited here."},
        "statements": [
            {"heading": "matches", "text": "Crash fixed.", "detail": "The lookup no longer throws.", "cites": ["d2", "d3"]},
            {"heading": "not-in-description", "text": "Diagnostic dropped.", "detail": "Verified by hand.", "cites": ["d1"]},
            {"heading": "not-in-description", "text": "Value lost.", "cites": ["d1"]},
        ]
    });
    let v = check_review(&resp, &b, None);
    let st = v["statements"].as_array().unwrap();
    assert_eq!(st[0]["detail"], "The lookup no longer throws.");
    // A banned word in `detail` drops the statement; in a summary it drops the summary.
    assert_eq!(v["dropped"].as_array().unwrap().len(), 1);
    assert_eq!(v["summaries"], json!({"matches": "Crash fixed."}));
    assert_valid(&fixtures().join("2283"), &v);
}

#[test]
fn snippet_kept_only_when_outcomes_match() {
    let b = Bundle::load(&fixtures().join("2283"));
    let d1 = &b.differences()[0];
    // d1: old reports MsgPack004 -> new nothing reported.
    let kept = accept_snippet(d1, "// lib ...", "MsgPack004: global::Lib.S<int>.Unkeyed", "0 warnings; round trip Unkeyed 42 -> 0");
    assert_eq!(kept["state"], "seen-live");
    assert_eq!(kept["input"]["source"], "model");
    let gone = accept_snippet(d1, "// lib ...", "0 warnings", "0 warnings");
    assert_eq!(gone["state"], "not-replayed");
    assert!(gone["input"].is_null());
    let d2 = &b.differences()[1];
    assert!(snippet_matches(d2, "CS8785, AD0001: IndexOutOfRangeException", "0 warnings; formatter without Key"));
    assert!(!snippet_matches(d2, "CS8785, AD0001: NullReferenceException", "0 warnings"));
}

#[test]
fn outcome_with_ids_and_throw() {
    // 2234-style row: a diagnostic then a crash.
    let row = "reports MsgPack005, throws InvalidOperationException";
    assert!(outcome_matches(row, "MsgPack005; AD0001: System.InvalidOperationException"));
    assert!(!outcome_matches(row, "MsgPack005"));
    assert!(!outcome_matches(row, "AD0001: InvalidOperationException"));
    assert!(!outcome_matches(row, "MsgPack005; NullReferenceException"));
    // IDs-only row still matches a real outcome that also crashed (2283 d1).
    assert!(outcome_matches("reports MsgPack004", "MsgPack004; CS8785: IndexOutOfRangeException"));
    // Nothing reported needs neither.
    assert!(outcome_matches("nothing reported", "0 warnings"));
    assert!(!outcome_matches("nothing reported", "MsgPack004"));
    assert!(!outcome_matches("nothing reported", "IndexOutOfRangeException"));
    let c = classify("reports MsgPack005, throws X.InvalidOperationException");
    assert_eq!(c.throws.as_deref(), Some("InvalidOperationException"));
    assert!(c.ids.contains("MsgPack005"));
}

#[test]
fn snippet_replay_missing_is_no_recording() {
    let b = Bundle::load(&fixtures().join("2283"));
    let r = Recorder::replay(tmp("snip"));
    assert!(matches!(propose_snippet(&b, &b.differences()[0], &r), Err(ModelError::NoRecording(_))));
}
