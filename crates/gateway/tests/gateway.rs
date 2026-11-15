use contracts::Schemas;
use gateway::{http, standin, Config};
use serde_json::{json, Value};
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const OLD: &str = "32258fdbee4bcd653c79f51b4376b5103c6f4d27";
const REPO: &str = "fork/junction26-messagepack-csharp";
const TOKEN: &str = "tok-reviewer-a";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn tmp(name: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gw-{name}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

struct Env {
    url: String,
    gh: Arc<standin::State>,
    bundles: PathBuf,
    data: PathBuf,
    key: String,
    new: String,
    dir: PathBuf,
}

/// Copy a fixture bundle, set its pull request to 1, start the stand-in and the gateway on port 0.
fn setup(fixture: &str, repo: Option<PathBuf>) -> Env {
    setup_with(fixture, repo, vec![])
}

fn setup_with(fixture: &str, repo: Option<PathBuf>, plugins: Vec<PathBuf>) -> Env {
    let t = tmp(fixture);
    let bundles = t.join("bundles");
    let dir = bundles.join(fixture);
    fs::create_dir_all(&dir).unwrap();
    for e in fs::read_dir(root().join("contracts/fixtures").join(fixture)).unwrap().flatten() {
        if e.path().is_file() {
            fs::copy(e.path(), dir.join(e.file_name())).unwrap();
        }
    }
    let mut b: Value = serde_json::from_slice(&fs::read(dir.join("bundle.json")).unwrap()).unwrap();
    b["pullRequest"] = json!(1);
    fs::write(dir.join("bundle.json"), serde_json::to_vec(&b).unwrap()).unwrap();
    let new = b["newCommit"].as_str().unwrap().to_string();
    let old = b["oldCommit"].as_str().unwrap().to_string();
    let gh = standin::start(TcpListener::bind("127.0.0.1:0").unwrap());
    gh.set_pull(REPO, 1, &new, "agent-author");
    gh.add_token(TOKEN, "reviewer-a");
    gh.add_token("tok-author", "agent-author");
    let ghurl = gh.base.lock().unwrap().clone();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let cfg = Config {
        bundles: bundles.clone(),
        data: t.join("data"),
        repo,
        plugins,
        trees: None,
        plugin_host: vec![env!("CARGO_BIN_EXE_stub-plugin-host").to_string()],
        github: ghurl,
        github_repo: REPO.into(),
        pull_request: None,
        prepare: vec![],
        web: Some(t.join("web")),
        plugin_timeout: Some(std::time::Duration::from_secs(3)),
    };
    fs::create_dir_all(t.join("web/assets")).unwrap();
    fs::write(t.join("web/index.html"), "<!doctype html>page").unwrap();
    fs::write(t.join("web/assets/app.js"), "x()").unwrap();
    fs::write(t.join("secret.txt"), "no").unwrap();
    let g = gateway::start(cfg, l);
    std::mem::forget(g);
    Env { url, gh, bundles, data: t.join("data"), key: format!("{old}-{new}"), new, dir }
}

impl Env {
    fn get(&self, path: &str) -> (u16, Value) {
        let (s, b) = http::call("GET", &format!("{}{path}", self.url), &[], None).unwrap();
        (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }
    fn post(&self, path: &str, body: &Value, token: Option<&str>) -> (u16, Value) {
        let auth = token.map(|t| format!("Bearer {t}"));
        let h: Vec<(&str, &str)> = auth.as_deref().map(|a| vec![("Authorization", a)]).unwrap_or_default();
        let (s, b) = http::call("POST", &format!("{}{path}", self.url), &h, Some(&serde_json::to_vec(body).unwrap())).unwrap();
        (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }
    fn update(&self, u: Value) -> Value {
        let (s, v) = self.post(&format!("/api/sessions/{}/reviewer-a", self.key), &u, None);
        assert_eq!(s, 200, "{v}");
        v
    }
    fn submit(&self, token: Option<&str>) -> (u16, Value) {
        self.post(&format!("/api/sessions/{}/reviewer-a/review", self.key), &json!({"risk": "Metadata types with unkeyed members now build and lose values."}), token)
    }
    fn record(&self, name: &str) -> Value {
        serde_json::from_slice(&fs::read(self.dir.join(format!("{name}.json"))).unwrap()).unwrap()
    }
}

fn schemas() -> Schemas {
    Schemas::contract().unwrap()
}
fn check_def(def: &str, v: &Value) {
    schemas().validate_def("gateway", def, v, def).unwrap_or_else(|e| panic!("{def}: {e}\n{v}"));
}
fn check(name: &str, v: &Value) {
    schemas().validate(name, v, name).unwrap_or_else(|e| panic!("{name}: {e}\n{v}"));
}

#[test]
fn fallbacks_line_one_is_contract_hash() {
    let out = std::process::Command::new("git").args(["log", "-1", "--format=%H", "--", "contracts/"]).current_dir(root()).output();
    let Some(out) = out.ok().filter(|o| o.status.success()) else { return };
    let hash = String::from_utf8(out.stdout).unwrap().trim().to_string();
    let f = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("FALLBACKS.md")).unwrap();
    assert_eq!(f.lines().next().unwrap().trim(), hash);
}

#[test]
fn contract_examples_validate() {
    for def in ["bundles", "fileText", "playground", "sessionUpdate", "submit", "submitted", "error"] {
        let v: Value = serde_json::from_slice(&fs::read(root().join(format!("contracts/messages/examples/gateway.{def}.json"))).unwrap()).unwrap();
        check_def(def, &v);
    }
}

#[test]
fn bundles_records_and_file_text() {
    // A small git repository standing in for the clone.
    let repo = tmp("repo");
    let git = |args: &[&str]| {
        let o = std::process::Command::new("git").args(args).current_dir(&repo)
            .env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t")
            .output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap()
    };
    git(&["init", "-q"]);
    fs::create_dir_all(repo.join("src/A")).unwrap();
    fs::write(repo.join("src/A/TypeCollector.cs"), "line one\nline two\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "c"]);
    let sha = git(&["rev-parse", "HEAD"]).trim().to_string();

    let e = setup("2283", Some(repo));
    let (s, v) = e.get("/api/bundles");
    assert_eq!(s, 200);
    check_def("bundles", &v);
    assert_eq!(v["bundles"][0]["key"], json!(e.key));
    assert_eq!(v["bundles"][0]["requirement"], json!("2283"));
    assert_eq!(v["bundles"][0]["stale"], json!(false));

    let (s, d) = e.get(&format!("/api/bundles/{}/differences.json", e.key));
    assert_eq!(s, 200);
    check("differences", &d);
    assert_eq!(d["differences"].as_array().unwrap().len(), 3);
    assert_eq!(e.get(&format!("/api/bundles/{}/nothing.json", e.key)).0, 404);
    assert_eq!(e.get(&format!("/api/bundles/{}/..%2Fbundle.json", e.key)).0, 404);

    let (s, f) = e.get(&format!("/api/file?commit={sha}&path=src/A/TypeCollector.cs"));
    assert_eq!(s, 200, "{f}");
    check_def("fileText", &f);
    assert_eq!(f["lines"], json!(["line one", "line two"]));
    assert_eq!(e.get(&format!("/api/file?commit={sha}&path=../etc/passwd")).0, 400);
    assert_eq!(e.get(&format!("/api/file?commit={sha}&path=missing.cs")).0, 404);
}

#[test]
fn playground_forwards_over_line_protocol() {
    let e = setup("2283", None);
    let ex: Value = serde_json::from_slice(&fs::read(root().join("contracts/messages/examples/gateway.playground.json")).unwrap()).unwrap();
    let path = format!("/api/bundles/{}/playground", e.key);
    let (s, r1) = e.post(&path, &ex, None);
    assert_eq!(s, 200, "{r1}");
    check("plugin-reply", &r1);
    assert_eq!(r1["id"], json!(7));
    assert_eq!(r1["results"][0]["value"]["plugin"], json!("generator-lab"));
    assert_eq!(r1["results"][0]["value"]["inputs"], json!("{\"roundTrip\":false}"));
    let (_, r2) = e.post(&path, &ex, None);
    assert_eq!(r1["pid"], r2["pid"], "one long-lived host process");
    assert_eq!(e.post(&path, &json!({"plugin": "x"}), None).0, 400);
}

#[test]
fn session_records_per_bundle_and_reviewer() {
    let e = setup("2283", None);
    let (_, s0) = e.get(&format!("/api/sessions/{}/reviewer-a", e.key));
    check("session", &s0);
    let ex: Value = serde_json::from_slice(&fs::read(root().join("contracts/messages/examples/gateway.sessionUpdate.json")).unwrap()).unwrap();
    check_def("sessionUpdate", &ex);
    e.update(ex);
    e.update(json!({"opened": ["b1", "b2"], "shown": ["p1"], "run": {"difference": "d2", "input": "preset 2"}, "spotCheck": "b5"}));
    let s = e.update(json!({"opened": ["b1"], "spotCheck": "b6"}));
    check("session", &s);
    assert_eq!(s["answers"], json!({"q1": "return"}));
    assert_eq!(s["opened"], json!(["b1", "b2"]));
    assert_eq!(s["spotCheck"], json!("b5"), "stable pick");
    let stored: Value = serde_json::from_slice(&fs::read(e.data.join("sessions").join(&e.key).join("reviewer-a/session.json")).unwrap()).unwrap();
    assert_eq!(stored, s);
    let (_, other) = e.get(&format!("/api/sessions/{}/reviewer-b", e.key));
    assert_eq!(other["answers"], json!({}));
}

fn refused(e: &Env, gate: &str) -> Value {
    let (s, v) = e.submit(Some(TOKEN));
    assert_eq!(s, 409, "{v}");
    check_def("error", &v);
    assert_eq!(v["gate"], json!(gate), "{v}");
    assert!(e.gh.reviews().is_empty(), "stand-in received a review");
    v
}

#[test]
fn approve_refused_for_each_unmet_gate() {
    let e = setup("2283", None);
    // One question unanswered.
    e.update(json!({"answer": {"question": "q1", "value": "accept"}}));
    let v = refused(&e, "question-unanswered");
    assert!(v["items"].as_array().unwrap().contains(&json!("q2")));
    e.update(json!({"answer": {"question": "q2", "value": "accept"}}));
    // Spot-check not opened.
    e.update(json!({"spotCheck": "b5", "opened": ["b2"]}));
    let v = refused(&e, "spot-check");
    assert!(v["items"].as_array().unwrap().contains(&json!("b5")));
    // Level-3 filter block b1 not opened.
    e.update(json!({"opened": ["b5"]}));
    let v = refused(&e, "level-3-unopened");
    assert_eq!(v["items"], json!(["b1"]));
    // A section running.
    e.update(json!({"opened": ["b1"]}));
    let mut b = e.record("bundle");
    b["sections"]["tests"] = json!({"state": "running"});
    fs::write(e.dir.join("bundle.json"), serde_json::to_vec(&b).unwrap()).unwrap();
    let v = refused(&e, "section-running");
    assert_eq!(v["items"], json!(["tests"]));
    b["sections"]["tests"] = json!({"state": "ready"});
    fs::write(e.dir.join("bundle.json"), serde_json::to_vec(&b).unwrap()).unwrap();
    let (s, v) = e.submit(Some(TOKEN));
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["review"]["event"], json!("APPROVE"));
}

#[test]
fn no_token_is_refused() {
    let e = setup("2283", None);
    let (s, v) = e.submit(None);
    assert_eq!(s, 401);
    check_def("error", &v);
    assert!(e.gh.received.lock().unwrap().is_empty());
    let (s, _) = e.post(&format!("/api/sessions/{}/reviewer-a/review", e.key), &json!({"risk": " "}), Some(TOKEN));
    assert_eq!(s, 400, "risk sentence required");
}

#[test]
fn return_requests_changes_and_body_is_composed() {
    let e = setup("2283", None);
    e.update(json!({"opened": ["b2", "b3"], "shown": ["p2"], "run": {"difference": "d2", "input": "preset 2"}}));
    e.update(json!({"answer": {"question": "q1", "value": "return"}}));
    e.update(json!({"answer": {"question": "q2", "value": "accept"}}));
    let (s, v) = e.submit(Some(TOKEN));
    assert_eq!(s, 200, "{v}");
    check_def("submitted", &v);
    let review = &v["review"];
    check("review", review);
    assert_eq!(review["event"], json!("REQUEST_CHANGES"));
    let body = review["body"].as_str().unwrap();
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(lines[0], "Requirement: 2283");
    assert_eq!(lines[1], "Looked at: b2, b3");
    assert_eq!(lines[2], "Tried: difference 2");
    assert!(lines[3].starts_with("Relied on machine checks: 3 differences, 3 differences checked by Lean"), "{}", lines[3]);
    assert_eq!(lines[4], "Not opened: b1, b4, b5, b6");
    assert!(lines[5].contains("2 assumptions (index-empty, failed-cast)"), "{}", lines[5]);
    assert_eq!(lines[6], "Answers: q1 return (b1 TypeCollector.cs:239), q2 accept");
    assert_eq!(lines[7], "Risk: Metadata types with unkeyed members now build and lose values.");
    let got = e.gh.reviews();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].authorization.as_deref(), Some(&*format!("Bearer {TOKEN}")));
    assert_eq!(got[0].body["event"], json!("REQUEST_CHANGES"));
    assert_eq!(got[0].body["commit_id"], json!(e.new));
    let stored: Value = serde_json::from_slice(&fs::read(e.data.join("sessions").join(&e.key).join("reviewer-a/review.json")).unwrap()).unwrap();
    assert_eq!(&stored, review);
    assert!(!fs::read_to_string(e.data.join("sessions").join(&e.key).join("reviewer-a/review.json")).unwrap().contains(TOKEN));
}

fn satisfy_all(e: &Env) {
    let blocks: Vec<Value> = e.record("blocks")["blocks"].as_array().unwrap().iter().map(|b| b["id"].clone()).collect();
    let spot = e.record("levels")["levels"].as_array().unwrap().iter().find(|l| l["level"] == json!(1)).map(|l| l["block"].clone());
    e.update(json!({"opened": blocks}));
    if let Some(s) = spot {
        e.update(json!({"spotCheck": s}));
    }
    for q in e.record("questions")["questions"].as_array().unwrap() {
        e.update(json!({"answer": {"question": q["id"], "value": "accept"}}));
    }
}

#[test]
fn approval_for_2297_reaches_stand_in() {
    let e = setup("2297", None);
    satisfy_all(&e);
    let (s, v) = e.submit(Some(TOKEN));
    assert_eq!(s, 200, "{v}");
    check_def("submitted", &v);
    let got = e.gh.reviews();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].authorization.as_deref(), Some(&*format!("Bearer {TOKEN}")));
    assert_eq!(got[0].body["event"], json!("APPROVE"));
    assert_eq!(got[0].body["commit_id"], json!(e.new));
    assert_eq!(v["review"]["result"]["status"], json!(200));
    assert!(v["review"]["result"]["url"].as_str().unwrap().contains("pullrequestreview"));
}

#[test]
fn self_approval_refusal_is_recorded() {
    let e = setup("2297", None);
    satisfy_all(&e);
    let (s, v) = e.submit(Some("tok-author"));
    assert_eq!(s, 422, "{v}");
    check_def("error", &v);
    assert_eq!(v["gate"], json!("self-approval"));
    let stored: Value = serde_json::from_slice(&fs::read(e.data.join("sessions").join(&e.key).join("reviewer-a/review.json")).unwrap()).unwrap();
    check("review", &stored);
    assert_eq!(stored["result"]["status"], json!(422));
}

#[test]
fn moved_head_marks_answers_stale() {
    let e = setup("2283", None);
    let moved = "1111111111111111111111111111111111111111";
    e.gh.set_pull(REPO, 1, moved, "agent-author");
    e.update(json!({"answer": {"question": "q1", "value": "return"}}));
    let (s, v) = e.submit(Some(TOKEN));
    assert_eq!(s, 409, "{v}");
    check_def("error", &v);
    assert_eq!(v["gate"], json!("head-moved"));
    let new_key = format!("{OLD}-{moved}");
    assert_eq!(v["items"], json!([new_key]));
    assert!(e.gh.reviews().is_empty());
    assert!(e.bundles.join(&new_key).join("bundle.json").is_file());
    let (_, list) = e.get("/api/bundles");
    check_def("bundles", &list);
    let by = |k: &str| list["bundles"].as_array().unwrap().iter().find(|b| b["key"] == json!(k)).cloned().unwrap();
    assert_eq!(by(&e.key)["stale"], json!(true));
    assert_eq!(by(&new_key)["stale"], json!(false));
    assert_eq!(by(&new_key)["prepared"], json!(false));
    // Earlier answers stay with the earlier bundle; the new bundle's session starts empty.
    let (_, old_s) = e.get(&format!("/api/sessions/{}/reviewer-a", e.key));
    assert_eq!(old_s["answers"], json!({"q1": "return"}));
    let (_, new_s) = e.get(&format!("/api/sessions/{new_key}/reviewer-a"));
    check("session", &new_s);
    assert_eq!(new_s["answers"], json!({}));
}

#[test]
fn gates_and_body_need_no_model() {
    // Gates and body are pure functions of records and session: identical across calls.
    let e = setup("2283", None);
    let mut r = gateway::review::Records::new();
    for n in ["bundle", "blocks", "parts", "differences", "scope", "lean", "levels", "questions", "mode"] {
        r.insert(n.into(), e.record(n));
    }
    let s: Value = serde_json::from_slice(&fs::read(root().join(format!("contracts/fixtures/sessions/{}/reviewer-a/session.json", e.key))).unwrap()).unwrap();
    let a = gateway::review::compose_body(&r, &s, "x");
    assert_eq!(a, gateway::review::compose_body(&r, &s, "x"));
    assert!(a.contains("Looked at: b1, b2, b3, b5 (spot-check)"));
    assert!(a.contains("Tried: difference 1, free input"));
    assert!(a.contains("Not opened: b4, b6"));
}

fn add_bundle(e: &Env, fixture: &str) -> String {
    let dir = e.bundles.join(fixture);
    fs::create_dir_all(&dir).unwrap();
    for f in fs::read_dir(root().join("contracts/fixtures").join(fixture)).unwrap().flatten() {
        if f.path().is_file() {
            fs::copy(f.path(), dir.join(f.file_name())).unwrap();
        }
    }
    let b: Value = serde_json::from_slice(&fs::read(dir.join("bundle.json")).unwrap()).unwrap();
    format!("{}-{}", b["oldCommit"].as_str().unwrap(), b["newCommit"].as_str().unwrap())
}

fn run_req() -> Value {
    json!({"id": 3, "op": "run", "subject": "s", "sides": ["new"], "inputs": {}})
}

#[test]
fn slow_plugin_does_not_block_other_bundle() {
    let e = Arc::new(setup("2283", None));
    let other = add_bundle(&e, "2297");
    let e2 = e.clone();
    let slow = std::thread::spawn(move || e2.post(&format!("/api/bundles/{}/playground", e2.key), &json!({"plugin": "slow2", "request": run_req()}), None));
    std::thread::sleep(std::time::Duration::from_millis(300));
    let t = std::time::Instant::now();
    let (s, v) = e.post(&format!("/api/bundles/{other}/playground"), &json!({"plugin": "generator-lab", "request": run_req()}), None);
    assert_eq!(s, 200, "{v}");
    assert!(t.elapsed() < std::time::Duration::from_millis(1000), "blocked for {:?}", t.elapsed());
    let (s, v) = slow.join().unwrap();
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["id"], json!(3));
}

#[test]
fn plugin_timeout_kills_host() {
    let e = setup("2283", None);
    let path = format!("/api/bundles/{}/playground", e.key);
    let t = std::time::Instant::now();
    let (s, v) = e.post(&path, &json!({"plugin": "slow30", "request": run_req()}), None);
    assert_eq!(s, 502, "{v}");
    check_def("error", &v);
    assert!(v["error"].as_str().unwrap().contains("timed out"));
    assert!(t.elapsed() < std::time::Duration::from_secs(10));
    // The other plugin of the bundle still answers.
    assert_eq!(e.post(&path, &json!({"plugin": "generator-lab", "request": run_req()}), None).0, 200);
}

#[test]
fn files_record_carries_folder_history() {
    let e = setup("2283", None);
    // A second bundle touching the same folders: 2283 with another new commit.
    let other_new = "2222222222222222222222222222222222222222";
    let odir = e.bundles.join("2283-next");
    fs::create_dir_all(&odir).unwrap();
    for f in fs::read_dir(&e.dir).unwrap().flatten() {
        fs::copy(f.path(), odir.join(f.file_name())).unwrap();
    }
    let mut b: Value = serde_json::from_slice(&fs::read(odir.join("bundle.json")).unwrap()).unwrap();
    b["newCommit"] = json!(other_new);
    fs::write(odir.join("bundle.json"), serde_json::to_vec(&b).unwrap()).unwrap();
    let other = format!("{OLD}-{other_new}");
    let folder = "src/MessagePack.SourceGenerator/CodeAnalysis";
    let hist = |key: &str| {
        let (s, v) = e.get(&format!("/api/bundles/{key}/files.json"));
        assert_eq!(s, 200);
        check("files", &v);
        v["files"].as_array().unwrap().iter().find(|f| f["history"]["folder"] == json!(folder)).unwrap()["history"].clone()
    };
    assert_eq!(hist(&other)["reviews"], json!(0));
    // Sign 2283.
    satisfy_all(&e);
    let (s, v) = e.submit(Some(TOKEN));
    assert_eq!(s, 200, "{v}");
    let h = hist(&other);
    assert_eq!(h["reviews"], json!(1));
    assert!(h["undeclared"].as_u64().unwrap() >= 1, "{h}");
    // Its own review is not counted.
    assert_eq!(hist(&e.key)["reviews"], json!(0));
}

fn manifest(dir: &Path, name: &str, marker: &str, outputs: Value) {
    fs::create_dir_all(dir.join(name)).unwrap();
    let m = json!({"name": name, "version": "0.1.0", "executable": env!("CARGO_BIN_EXE_stub-plugin-host"), "args": [],
        "detect": [{"rule": "file-exists", "path": marker}], "inputs": [], "outputs": outputs});
    fs::write(dir.join(name).join("plugin.json"), serde_json::to_vec(&m).unwrap()).unwrap();
}

#[test]
fn viewers_from_plugin_folders() {
    let t = tmp("plugins");
    let repo = t.join("repo");
    fs::create_dir_all(&repo).unwrap();
    fs::write(repo.join("marker.txt"), "x").unwrap();
    let (p1, p2) = (t.join("plugins"), t.join("plugins-web"));
    manifest(&p1, "echo", "marker.txt", json!([{"name": "a", "type": "facts"}, {"name": "b", "type": "facts"}, {"name": "c", "type": "text-diff"}]));
    manifest(&p2, "absent", "absent.txt", json!([{"name": "a", "type": "facts"}]));
    manifest(&p2, "site-preview", "marker.txt", json!([{"name": "page", "type": "page"}]));
    let e = setup_with("2283", Some(repo), vec![p1.clone(), p2]);
    let (s, v) = e.get(&format!("/api/bundles/{}/viewers", e.key));
    assert_eq!(s, 200, "{v}");
    check_def("viewers", &v);
    assert_eq!(v, json!({"viewers": [
        {"name": "echo", "title": "Echo", "outputs": ["facts", "text-diff"]},
        {"name": "site-preview", "title": "Site preview", "outputs": ["page"]}], "tabs": true}));
    // The host is started with the plugin's folder and the bundle's work folder.
    let (s, r) = e.post(&format!("/api/bundles/{}/playground", e.key), &json!({"plugin": "echo", "request": run_req()}), None);
    assert_eq!(s, 200, "{r}");
    assert_eq!(r["results"][0]["value"]["work"], json!(e.data.join("plugins").join(&e.key).to_string_lossy()));
    // No plugins configured: empty list.
    let e2 = setup("2283", None);
    let (_, v) = e2.get(&format!("/api/bundles/{}/viewers", e2.key));
    assert_eq!(v, json!({"viewers": [], "tabs": false}));
}

/// A tree shaped like the sample: generator csproj referencing Roslyn and an IIncrementalGenerator.
fn sample_tree(repo: &Path) {
    let g = repo.join("src/MessagePack.SourceGenerator");
    fs::create_dir_all(&g).unwrap();
    fs::write(g.join("MessagePack.SourceGenerator.csproj"), "<Project><ItemGroup><PackageReference Include=\"Microsoft.CodeAnalysis.CSharp\" /></ItemGroup></Project>\n").unwrap();
    fs::write(g.join("MessagePackGenerator.cs"), "public partial class MessagePackGenerator : IIncrementalGenerator { }\n").unwrap();
    fs::write(repo.join("README.md"), "x").unwrap();
}

#[test]
fn real_generator_lab_manifest_detects_sample_shape() {
    let t = tmp("realplug");
    let repo = t.join("repo");
    sample_tree(&repo);
    let e = setup_with("2283", Some(repo), vec![root().join("plugins"), root().join("plugins/web")]);
    let (s, v) = e.get(&format!("/api/bundles/{}/viewers", e.key));
    assert_eq!(s, 200, "{v}");
    check_def("viewers", &v);
    let names: Vec<&str> = v["viewers"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"generator-lab"), "{v}");
}

#[test]
fn detection_runs_on_the_new_commit_not_head() {
    let t = tmp("detect-commit");
    let repo = t.join("repo");
    fs::create_dir_all(&repo).unwrap();
    let git = |args: &[&str]| {
        let o = std::process::Command::new("git").args(args).current_dir(&repo)
            .env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t")
            .output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim().to_string()
    };
    git(&["init", "-q"]);
    fs::write(repo.join("README.md"), "x").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "plain"]);
    let plain = git(&["rev-parse", "HEAD"]);
    sample_tree(&repo);
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "generator"]);
    let with_gen = git(&["rev-parse", "HEAD"]);
    git(&["checkout", "-q", &plain]); // HEAD and working tree have no generator.
    let e = setup_with("2283", Some(repo.clone()), vec![root().join("plugins")]);
    let add = |new: &str| {
        let d = e.bundles.join(format!("b-{new}"));
        fs::create_dir_all(&d).unwrap();
        let mut b = e.record("bundle");
        b["newCommit"] = json!(new);
        fs::write(d.join("bundle.json"), serde_json::to_vec(&b).unwrap()).unwrap();
        format!("{OLD}-{new}")
    };
    let names = |key: &str| {
        let (_, v) = e.get(&format!("/api/bundles/{key}/viewers"));
        v["viewers"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap().to_string()).collect::<Vec<_>>()
    };
    let (kg, kp) = (add(&with_gen), add(&plain));
    assert_eq!(names(&kg), vec!["generator-lab"]);
    assert!(names(&kp).is_empty());
    assert_eq!(git(&["rev-parse", "HEAD"]), plain, "clone HEAD untouched");
    assert!(git(&["status", "--porcelain"]).is_empty(), "clone working tree untouched");
}

#[test]
fn pages_from_plugin_workdir() {
    let e = setup("2283", None);
    let wd = e.data.join("plugins").join(&e.key).join("site-preview").join(&e.new);
    fs::create_dir_all(wd.join("assets")).unwrap();
    fs::write(wd.join("index.html"), "<p>page</p>").unwrap();
    fs::write(wd.join("assets/app.js"), "y()").unwrap();
    fs::write(e.data.join("secret.txt"), "no").unwrap();
    let base = format!("/api/bundles/{}/pages/site-preview", e.key);
    let get = |p: &str| {
        let (s, b) = http::call("GET", &format!("{}{p}", e.url), &[], None).unwrap();
        (s, String::from_utf8_lossy(&b).to_string())
    };
    assert_eq!(get(&format!("{base}/index.html")), (200, "<p>page</p>".into()));
    assert_eq!(get(&format!("{base}/assets/app.js")), (200, "y()".into()));
    assert_eq!(get(&format!("{base}/missing.html")).0, 404);
    assert_eq!(get(&format!("{base}/%2e%2e/%2e%2e/%2e%2e/%2e%2e/secret.txt")).0, 404);
    assert_eq!(get(&format!("{base}/../../secret.txt")).0, 404);
    assert_eq!(get(&format!("/api/bundles/{}/pages/other/index.html", e.key)).0, 404);
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(e.url.trim_start_matches("http://")).unwrap();
    s.write_all(format!("GET {base}/assets/app.js HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    assert!(out.contains("Content-Type: text/javascript"), "{out}");
}

#[test]
fn serves_built_page() {
    let e = setup("2283", None);
    let get = |p: &str| {
        let (s, b) = http::call("GET", &format!("{}{p}", e.url), &[], None).unwrap();
        (s, String::from_utf8_lossy(&b).to_string())
    };
    assert_eq!(get("/"), (200, "<!doctype html>page".into()));
    assert_eq!(get("/assets/app.js"), (200, "x()".into()));
    assert_eq!(get("/review/some-key"), (200, "<!doctype html>page".into()));
    assert_eq!(get("/../secret.txt").0, 400);
    assert_eq!(get("/assets/%2e%2e/%2e%2e/secret.txt").0, 400);
    assert_eq!(get("/api/nothing").0, 404);
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(e.url.trim_start_matches("http://")).unwrap();
    s.write_all(b"GET /assets/app.js HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    assert!(out.contains("Content-Type: text/javascript"), "{out}");
}
