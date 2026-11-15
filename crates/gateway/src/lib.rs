//! Witness gateway (D13, D36, D41, D47, D48): the HTTP API of `contracts/messages/gateway-api.md`.
pub mod http;
pub mod plugin;
pub mod review;
pub mod standin;

use http::{Request, Response};
use review::Records;
use serde_json::{json, Value};
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Folder of bundle folders (each holds `bundle.json` and the other records).
    pub bundles: PathBuf,
    /// Data folder: `sessions/<key>/<reviewer>/{session,review}.json`, `stale/<key>`.
    pub data: PathBuf,
    /// A clone of the repository, for `/api/file` (`git show <commit>:<path>`).
    pub repo: Option<PathBuf>,
    /// Plugin folders (`--plugins`, repeatable), each holding `<plugin>/plugin.json`.
    pub plugins: Vec<PathBuf>,
    /// Checkout per commit for plugin detection, `{commit}` replaced (e.g. `work/int/sb/wt/{commit}`).
    pub trees: Option<String>,
    /// Plugin host command; the gateway appends `[--plugins <dir>] --work <data>/plugins/<key> <plugin> <bundle dir>`.
    pub plugin_host: Vec<String>,
    /// GitHub API base, e.g. `https://api.github.com` or the stand-in `http://127.0.0.1:9261`.
    pub github: String,
    /// `owner/repo` of the fork holding the pull requests.
    pub github_repo: String,
    /// Pull request used when `bundle.json` has `pullRequest: null`.
    pub pull_request: Option<u64>,
    /// Command run as `<cmd...> <oldCommit> <newHead> <dir>` to prepare a new bundle when the head moved.
    /// Without it a placeholder `bundle.json` with every section `not-run` is written.
    pub prepare: Vec<String>,
    /// Built page folder (e.g. `web/dist`): non-`/api` GETs are served from it, unknown paths get `index.html`.
    pub web: Option<PathBuf>,
    /// Plugin request timeout; `None` means 90 s.
    pub plugin_timeout: Option<std::time::Duration>,
}

pub struct Gateway {
    pub cfg: Config,
    hosts: plugin::Hosts,
    lock: Mutex<()>,
}

fn err(status: u16, msg: impl Into<String>, gate: Option<&str>, items: Vec<String>) -> Response {
    let mut v = json!({"error": msg.into(), "gate": gate});
    if !items.is_empty() {
        v["items"] = json!(items);
    }
    Response::json(status, &v)
}

/// Content type by extension; anything unknown is `application/octet-stream` (never sniffed as a page).
fn content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        "txt" | "md" | "log" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn is_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn is_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 100 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.') && !s.starts_with('.')
}
fn read(p: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(p).ok()?).ok()
}
fn write(p: &Path, v: &Value) -> std::io::Result<()> {
    fs::create_dir_all(p.parent().unwrap())?;
    let tmp = p.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(v).unwrap())?;
    fs::rename(tmp, p)
}

impl Gateway {
    pub fn new(cfg: Config) -> Gateway {
        Gateway { cfg, hosts: plugin::Hosts::default(), lock: Mutex::new(()) }
    }

    /// Bundle folders by key `<old>-<new>`.
    pub fn bundles(&self) -> Vec<(String, PathBuf, Value)> {
        let mut out = Vec::new();
        let Ok(rd) = fs::read_dir(&self.cfg.bundles) else { return out };
        let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.join("bundle.json").is_file()).collect();
        dirs.sort();
        for d in dirs {
            let Some(b) = read(&d.join("bundle.json")) else { continue };
            let (o, n) = (b["oldCommit"].as_str().unwrap_or(""), b["newCommit"].as_str().unwrap_or(""));
            if is_sha(o) && is_sha(n) {
                out.push((format!("{o}-{n}"), d, b));
            }
        }
        out
    }
    fn bundle(&self, key: &str) -> Option<(PathBuf, Value)> {
        self.bundles().into_iter().find(|(k, _, _)| k == key).map(|(_, d, b)| (d, b))
    }
    fn records(dir: &Path) -> Records {
        let mut r = Records::new();
        for name in ["bundle", "blocks", "parts", "differences", "scope", "lean", "tests", "levels", "questions", "mode"] {
            if let Some(v) = read(&dir.join(format!("{name}.json"))) {
                r.insert(name.into(), v);
            }
        }
        r
    }
    fn stale_marker(&self, key: &str) -> PathBuf {
        self.cfg.data.join("stale").join(key)
    }
    fn session_path(&self, key: &str, reviewer: &str) -> PathBuf {
        self.cfg.data.join("sessions").join(key).join(reviewer).join("session.json")
    }
    /// The stored session, or an empty one.
    pub fn session(&self, key: &str, reviewer: &str) -> Value {
        read(&self.session_path(key, reviewer)).unwrap_or_else(|| {
            let (o, n) = key.split_once('-').unwrap_or(("", ""));
            json!({"oldCommit": o, "newCommit": n, "reviewer": reviewer, "opened": [], "shown": [], "runs": [], "answers": {}, "spotCheck": null})
        })
    }

    pub fn handle(&self, req: &Request) -> Response {
        let seg: Vec<&str> = req.path.trim_matches('/').split('/').collect();
        match (req.method.as_str(), seg.as_slice()) {
            ("GET", ["api", "bundles"]) => self.list(),
            ("GET", ["api", "bundles", key, "viewers"]) => self.viewers(key),
            ("GET", ["api", "bundles", key, "pages", plugin, rest @ ..]) => self.page(key, plugin, rest),
            ("GET", ["api", "bundles", key, record]) => self.record(key, record),
            ("POST", ["api", "bundles", key, "playground"]) => self.playground(key, req),
            ("GET", ["api", "file"]) => self.file(req),
            ("GET", ["api", "sessions", key, reviewer]) if is_name(reviewer) && self.bundle(key).is_some() => {
                Response::json(200, &self.session(key, reviewer))
            }
            ("POST", ["api", "sessions", key, reviewer]) if is_name(reviewer) && self.bundle(key).is_some() => self.update(key, reviewer, req),
            ("POST", ["api", "sessions", key, reviewer, "review"]) if is_name(reviewer) && self.bundle(key).is_some() => {
                self.submit(key, reviewer, req)
            }
            ("GET", _) if seg[0] != "api" && self.cfg.web.is_some() => self.static_file(&req.path),
            _ => err(404, "not found", None, vec![]),
        }
    }

    fn static_file(&self, path: &str) -> Response {
        let web = self.cfg.web.as_ref().unwrap();
        let rel = path.trim_start_matches('/');
        if rel.split('/').any(|p| p == ".." || p.contains('\\')) || rel.contains('\0') {
            return err(400, "bad path", None, vec![]);
        }
        let mut p = web.join(rel);
        if rel.is_empty() || !p.is_file() {
            p = web.join("index.html");
        }
        match fs::read(&p) {
            Ok(body) => Response { status: 200, body, content_type: content_type(&p) },
            Err(_) => err(404, "not found", None, vec![]),
        }
    }

    /// Plugin manifests from every `--plugins` folder; a name seen first wins.
    fn plugins(&self) -> Vec<plugin_host::Manifest> {
        let mut out: Vec<plugin_host::Manifest> = Vec::new();
        for d in &self.cfg.plugins {
            // One bad manifest skips only itself (load_plugins would drop the whole folder).
            let mut dirs: Vec<PathBuf> = fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path().join("plugin.json")).filter(|p| p.is_file()).collect();
            dirs.sort();
            for p in dirs {
                let m = match plugin_host::Manifest::load(&p) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("gateway: skip {}: {}", p.display(), e.0);
                        continue;
                    }
                };
                if !out.iter().any(|o| o.name() == m.name()) {
                    out.push(m);
                }
            }
        }
        out
    }

    /// The `--plugins` folder holding `<plugin>/plugin.json`.
    pub fn plugin_dir(&self, plugin: &str) -> Option<PathBuf> {
        self.cfg.plugins.iter().find(|d| d.join(plugin).join("plugin.json").is_file()).cloned()
    }

    /// The `--work` given to the host for a bundle: `<data>/plugins/<key>`.
    pub fn plugin_work(&self, key: &str) -> PathBuf {
        self.cfg.data.join("plugins").join(key)
    }

    /// The plugin's working folder for a bundle, as the host chooses it: `<work>/<plugin>/<newCommit>`.
    pub fn plugin_workdir(&self, key: &str, plugin: &str) -> PathBuf {
        let new = key.split_once('-').map(|(_, n)| n).unwrap_or("unknown");
        self.plugin_work(key).join(plugin).join(new)
    }

    /// A checkout of `commit` for detection: `--trees` template (`{commit}` replaced) when that folder
    /// exists; else `<data>/trees/<commit>`, written once from `--repo` with `git checkout` into a separate
    /// work tree and index (the clone's HEAD, index and refs are not touched); else `--repo` itself when
    /// it is not a git repository.
    pub fn tree(&self, commit: &str) -> Option<PathBuf> {
        if !is_sha(commit) {
            return None;
        }
        if let Some(t) = &self.cfg.trees {
            let p = PathBuf::from(t.replace("{commit}", commit));
            if p.is_dir() {
                return Some(p);
            }
        }
        let repo = self.cfg.repo.as_ref()?;
        // The folder itself must be a clone (`.git`) or a bare repository, not a folder inside another repository.
        let is_git = repo.join(".git").exists() || (repo.join("HEAD").is_file() && repo.join("objects").is_dir());
        if !is_git {
            return Some(repo.clone());
        }
        let dest = self.cfg.data.join("trees").join(commit);
        if dest.join(".witness-done").is_file() {
            return Some(dest);
        }
        let _g = self.lock.lock().unwrap();
        if dest.join(".witness-done").is_file() {
            return Some(dest);
        }
        let _ = fs::remove_dir_all(&dest);
        fs::create_dir_all(&dest).ok()?;
        let index = self.cfg.data.join("trees").join(format!("{commit}.index"));
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .arg("--work-tree")
            .arg(&dest)
            .env("GIT_INDEX_FILE", &index)
            .args(["-c", "core.hooksPath=/dev/null", "checkout", "-q", "-f", commit, "--", "."])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        let _ = fs::remove_file(&index);
        if !ok {
            let _ = fs::remove_dir_all(&dest);
            return None;
        }
        fs::write(dest.join(".witness-done"), commit).ok()?;
        Some(dest)
    }

    fn viewers(&self, key: &str) -> Response {
        if self.bundle(key).is_none() {
            return err(404, "no bundle", None, vec![]);
        }
        let new = key.split_once('-').map(|(_, n)| n).unwrap_or("");
        let list: Vec<Value> = match self.tree(new) {
            Some(repo) => self
                .plugins()
                .into_iter()
                .filter(|m| m.detect(&repo))
                .map(|m| {
                    let name = m.name().to_string();
                    let title = m.json["title"].as_str().map(String::from).unwrap_or_else(|| {
                        let t = name.replace(['-', '_'], " ");
                        let mut c = t.chars();
                        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
                    });
                    let mut outputs: Vec<String> = Vec::new();
                    for (_, t) in m.outputs() {
                        if !outputs.contains(&t) {
                            outputs.push(t);
                        }
                    }
                    json!({"name": name, "title": title, "outputs": outputs})
                })
                .collect(),
            None => vec![],
        };
        let tabs = list.len() > 1;
        Response::json(200, &json!({"viewers": list, "tabs": tabs}))
    }

    fn page(&self, key: &str, plugin: &str, rest: &[&str]) -> Response {
        let not_found = || err(404, "not found", None, vec![]);
        if self.bundle(key).is_none() || !is_name(plugin) || rest.is_empty() {
            return not_found();
        }
        if rest.iter().any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains('\\') || p.contains('\0')) {
            return not_found();
        }
        let base = self.plugin_workdir(key, plugin);
        let p = rest.iter().fold(base.clone(), |acc, s| acc.join(s));
        // Refuse symlinks leading out of the working folder.
        match (p.canonicalize(), base.canonicalize()) {
            (Ok(c), Ok(b)) if c.starts_with(&b) && c.is_file() => match fs::read(&c) {
                Ok(body) => Response { status: 200, body, content_type: content_type(&c) },
                Err(_) => not_found(),
            },
            _ => not_found(),
        }
    }

    fn list(&self) -> Response {
        let list: Vec<Value> = self
            .bundles()
            .into_iter()
            .map(|(key, _, b)| {
                let prepared = b["sections"].as_object().map(|m| m.values().all(|v| !matches!(v["state"].as_str(), Some("running") | Some("not-run")))).unwrap_or(false);
                json!({"key": key, "oldCommit": b["oldCommit"], "newCommit": b["newCommit"], "pullRequest": b["pullRequest"],
                       "requirement": b["requirement"]["id"], "prepared": prepared, "stale": self.stale_marker(&key).exists()})
            })
            .collect();
        Response::json(200, &json!({"bundles": list}))
    }

    fn record(&self, key: &str, record: &str) -> Response {
        let Some((dir, _)) = self.bundle(key) else { return err(404, "no bundle", None, vec![]) };
        let ok = record.strip_suffix(".json").map(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')).unwrap_or(false);
        if ok && record == "files.json" {
            // Folder history (D44): counted per request from signed reviews of the other known bundles.
            if let Some(files) = read(&dir.join(record)) {
                let dirs: Vec<PathBuf> = self.bundles().into_iter().map(|(_, d, _)| d).collect();
                let counts = history::count(&[self.cfg.data.join("sessions")], &dirs, Some(key));
                return Response::json(200, &history::annotate(&files, &counts));
            }
        }
        match fs::read(dir.join(record)) {
            Ok(bytes) if ok => Response::raw_json(200, bytes),
            _ => err(404, "no record", None, vec![]),
        }
    }

    fn file(&self, req: &Request) -> Response {
        let (Some(commit), Some(path)) = (req.param("commit"), req.param("path")) else { return err(400, "commit and path required", None, vec![]) };
        if !is_sha(commit) || path.is_empty() || path.starts_with('/') || path.split('/').any(|p| p == ".." || p.is_empty()) {
            return err(400, "bad commit or path", None, vec![]);
        }
        let Some(repo) = &self.cfg.repo else { return err(404, "no repository", None, vec![]) };
        let out = std::process::Command::new("git").arg("-C").arg(repo).arg("show").arg(format!("{commit}:{path}")).output();
        match out {
            Ok(o) if o.status.success() => {
                let text = String::from_utf8_lossy(&o.stdout);
                let text = text.strip_suffix('\n').unwrap_or(&text);
                let lines: Vec<&str> = if text.is_empty() { vec![] } else { text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect() };
                Response::json(200, &json!({"commit": commit, "file": path, "lines": lines}))
            }
            _ => err(404, "no such file at commit", None, vec![]),
        }
    }

    fn playground(&self, key: &str, req: &Request) -> Response {
        let Some((dir, _)) = self.bundle(key) else { return err(404, "no bundle", None, vec![]) };
        let Some(body) = req.json() else { return err(400, "bad json", None, vec![]) };
        let (Some(plugin), Some(request)) = (body["plugin"].as_str(), body.get("request").filter(|r| r.is_object())) else {
            return err(400, "plugin and request required", None, vec![]);
        };
        if !is_name(plugin) {
            return err(400, "bad plugin name", None, vec![]);
        }
        let mut cmd = self.cfg.plugin_host.clone();
        if !cmd.is_empty() {
            if let Some(pd) = self.plugin_dir(plugin) {
                cmd.extend(["--plugins".to_string(), pd.to_string_lossy().into_owned()]);
            }
            cmd.extend(["--work".to_string(), self.plugin_work(key).to_string_lossy().into_owned()]);
        }
        match self.hosts.forward(&cmd, plugin, &dir, key, request.clone(), self.cfg.plugin_timeout.unwrap_or(plugin::DEFAULT_TIMEOUT)) {
            Ok(v) => Response::json(200, &v),
            Err(e) => err(502, e, None, vec![]),
        }
    }

    fn update(&self, key: &str, reviewer: &str, req: &Request) -> Response {
        let Some(u) = req.json().filter(|v| v.as_object().map(|m| !m.is_empty()).unwrap_or(false)) else {
            return err(400, "sessionUpdate required", None, vec![]);
        };
        let _g = self.lock.lock().unwrap();
        let mut s = self.session(key, reviewer);
        for field in ["opened", "shown"] {
            if let Some(a) = u.get(field) {
                let Some(a) = a.as_array().filter(|a| a.iter().all(Value::is_string)) else { return err(400, format!("bad {field}"), None, vec![]) };
                let cur = s[field].as_array_mut().unwrap();
                for x in a {
                    if !cur.contains(x) {
                        cur.push(x.clone());
                    }
                }
            }
        }
        if let Some(run) = u.get("run") {
            if !run["input"].is_string() || !(run["difference"].is_null() || run["difference"].is_string()) {
                return err(400, "bad run", None, vec![]);
            }
            s["runs"].as_array_mut().unwrap().push(json!({"difference": run["difference"], "input": run["input"]}));
        }
        if let Some(a) = u.get("answer") {
            let (Some(q), Some(v)) = (a["question"].as_str(), a["value"].as_str()) else { return err(400, "bad answer", None, vec![]) };
            if !matches!(v, "accept" | "return") || !q.starts_with('q') || q[1..].parse::<u32>().map(|n| n == 0).unwrap_or(true) {
                return err(400, "bad answer", None, vec![]);
            }
            s["answers"][q] = json!(v);
        }
        if let Some(b) = u.get("spotCheck") {
            let Some(b) = b.as_str() else { return err(400, "bad spotCheck", None, vec![]) };
            // The pick is stable: the first stored block stays.
            if s["spotCheck"].is_null() {
                s["spotCheck"] = json!(b);
            }
        }
        if let Err(e) = write(&self.session_path(key, reviewer), &s) {
            return err(500, e.to_string(), None, vec![]);
        }
        Response::json(200, &s)
    }

    fn prepare_new(&self, dir: &Path, bundle: &Value, head: &str) -> String {
        let old = bundle["oldCommit"].as_str().unwrap_or("");
        let key = format!("{old}-{head}");
        let target = self.cfg.bundles.join(&key);
        if self.bundle(&key).is_some() {
            return key;
        }
        if !self.cfg.prepare.is_empty() {
            let _ = std::process::Command::new(&self.cfg.prepare[0]).args(&self.cfg.prepare[1..]).arg(old).arg(head).arg(&target).status();
        }
        if !target.join("bundle.json").exists() {
            let mut b = bundle.clone();
            b["newCommit"] = json!(head);
            if let Some(m) = b["sections"].as_object_mut() {
                for v in m.values_mut() {
                    *v = json!({"state": "not-run", "detail": "head moved"});
                }
            }
            let _ = write(&target.join("bundle.json"), &b);
        }
        let _ = dir;
        key
    }

    fn submit(&self, key: &str, reviewer: &str, req: &Request) -> Response {
        let Some(token) = req.header("authorization").map(str::trim).filter(|t| !t.is_empty()) else {
            return err(401, "reviewer token required", None, vec![]);
        };
        let token = token.strip_prefix("Bearer ").or_else(|| token.strip_prefix("token ")).unwrap_or(token).trim().to_string();
        if token.is_empty() {
            return err(401, "reviewer token required", None, vec![]);
        }
        let risk = req.json().and_then(|v| v["risk"].as_str().map(|s| s.trim().to_string())).unwrap_or_default();
        if risk.is_empty() {
            return err(400, "risk sentence required", None, vec![]);
        }
        let (dir, bundle) = self.bundle(key).unwrap();
        let r = Self::records(&dir);
        let session = self.session(key, reviewer);
        let returned = review::returned(&r, &session);
        let event = if returned.is_empty() { "APPROVE" } else { "REQUEST_CHANGES" };
        if event == "APPROVE" {
            let unmet = review::approval_gates(&r, &session);
            if let Some(first) = unmet.first() {
                let gates: Vec<&str> = unmet.iter().map(|u| u.gate).collect();
                let items: Vec<String> = unmet.iter().flat_map(|u| u.items.clone()).collect();
                return err(409, format!("unmet: {}", gates.join(", ")), Some(first.gate), items);
            }
        }
        let Some(pr) = bundle["pullRequest"].as_u64().or(self.cfg.pull_request) else {
            return err(400, "no pull request", None, vec![]);
        };
        let base = self.cfg.github.trim_end_matches('/');
        let repo = &self.cfg.github_repo;
        let auth = format!("Bearer {token}");
        let hdrs = [("Authorization", auth.as_str()), ("Accept", "application/vnd.github+json"), ("User-Agent", "witness-gateway")];
        let new_commit = bundle["newCommit"].as_str().unwrap_or("").to_string();
        let head = match http::call("GET", &format!("{base}/repos/{repo}/pulls/{pr}"), &hdrs, None) {
            Ok((200, b)) => serde_json::from_slice::<Value>(&b).ok().and_then(|v| v["head"]["sha"].as_str().map(String::from)),
            _ => None,
        };
        let Some(head) = head else { return err(502, "pull request head unavailable", None, vec![]) };
        if head != new_commit {
            let _g = self.lock.lock().unwrap();
            let new_key = self.prepare_new(&dir, &bundle, &head);
            let _ = fs::create_dir_all(self.cfg.data.join("stale"));
            let _ = fs::write(self.stale_marker(key), &new_key);
            return err(409, "head moved", Some("head-moved"), vec![new_key]);
        }
        let body = review::compose_body(&r, &session, &risk);
        let payload = json!({"commit_id": new_commit, "body": body, "event": event});
        let res = http::call("POST", &format!("{base}/repos/{repo}/pulls/{pr}/reviews"), &hdrs, Some(&serde_json::to_vec(&payload).unwrap()));
        let (status, url) = match &res {
            Ok((st, b)) => (*st, serde_json::from_slice::<Value>(b).ok().and_then(|v| v["html_url"].as_str().map(String::from))),
            Err(_) => (0, None),
        };
        let result = if res.is_ok() { json!({"status": status, "url": url, "commit": new_commit}) } else { Value::Null };
        let review = json!({"oldCommit": bundle["oldCommit"], "newCommit": new_commit, "reviewer": reviewer, "event": event, "body": body, "result": result});
        let _ = write(&self.session_path(key, reviewer).with_file_name("review.json"), &review);
        match status {
            200..=299 => Response::json(200, &json!({"review": review})),
            422 => err(422, "review refused by GitHub", Some("self-approval"), vec![]),
            0 => err(502, "GitHub unreachable", None, vec![]),
            s => err(502, format!("GitHub answered {s}"), None, vec![]),
        }
    }
}

/// Serve the gateway on `listener` in a background thread.
pub fn start(cfg: Config, listener: TcpListener) -> Arc<Gateway> {
    let g = Arc::new(Gateway::new(cfg));
    let g2 = g.clone();
    std::thread::spawn(move || http::serve(listener, move |r| g2.handle(r)));
    g
}
