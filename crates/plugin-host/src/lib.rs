//! Plugin host (D36, D38): manifests, deterministic detection, the line protocol, fallback viewers.

use serde_json::{json, Value};
use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// The closed list of eight result types.
pub const RESULT_TYPES: [&str; 8] = ["diagnostics", "text-diff", "table", "facts", "bytes", "image", "image-pair", "page"];

#[derive(Debug, Clone, PartialEq)]
pub struct Error(pub String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
fn err<T>(m: impl Into<String>) -> Result<T, Error> {
    Err(Error(m.into()))
}

// ---------- manifests ----------

#[derive(Debug, Clone)]
pub struct Manifest {
    /// Folder holding `plugin.json`; `executable` and `config` resolve against it.
    pub dir: PathBuf,
    pub json: Value,
}

impl Manifest {
    pub fn name(&self) -> &str {
        self.json["name"].as_str().unwrap_or("")
    }
    pub fn executable(&self) -> PathBuf {
        self.dir.join(self.json["executable"].as_str().unwrap_or(""))
    }
    pub fn args(&self) -> Vec<String> {
        strings(&self.json["args"])
    }
    pub fn outputs(&self) -> Vec<(String, String)> {
        self.json["outputs"].as_array().into_iter().flatten()
            .map(|o| (o["name"].as_str().unwrap_or("").into(), o["type"].as_str().unwrap_or("").into()))
            .collect()
    }
    pub fn config(&self) -> Option<PathBuf> {
        self.json["config"].as_str().map(|c| self.dir.join(c))
    }

    /// Parse and check a manifest. An output outside the eight result types is rejected by name.
    pub fn parse(json: Value, dir: &Path) -> Result<Manifest, Error> {
        if let Some(outs) = json["outputs"].as_array() {
            for o in outs {
                let t = o["type"].as_str().unwrap_or("");
                if !RESULT_TYPES.contains(&t) {
                    let n = o["name"].as_str().unwrap_or("?");
                    return err(format!("output {n}: unknown result type {t:?}"));
                }
            }
        }
        let s = contracts::Schemas::contract().map_err(|e| Error(e.to_string()))?;
        s.validate("plugin-manifest", &json, "plugin.json").map_err(|e| Error(e.to_string()))?;
        Ok(Manifest { dir: dir.to_path_buf(), json })
    }

    pub fn load(path: &Path) -> Result<Manifest, Error> {
        let text = fs::read_to_string(path).map_err(|e| Error(format!("{}: {e}", path.display())))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| Error(format!("{}: {e}", path.display())))?;
        Manifest::parse(v, path.parent().unwrap_or(Path::new(".")))
    }

    /// True when every detection rule holds on `repo` (rules are ANDed; deterministic).
    pub fn detect(&self, repo: &Path) -> bool {
        let files = repo_files(repo);
        self.json["detect"].as_array().map_or(false, |rules| !rules.is_empty() && rules.iter().all(|r| rule_holds(r, repo, &files)))
    }
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|s| s.as_str().map(String::from)).collect()
}

/// Load every `*/plugin.json` directly under `dir`, sorted by folder name.
pub fn load_plugins(dir: &Path) -> Result<Vec<Manifest>, Error> {
    let mut v = vec![];
    let mut entries: Vec<PathBuf> = fs::read_dir(dir).map_err(|e| Error(e.to_string()))?.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let m = p.join("plugin.json");
        if m.is_file() {
            v.push(Manifest::load(&m)?);
        }
    }
    Ok(v)
}

// ---------- detection ----------

/// Repository files as sorted `/`-separated relative paths, skipping `.git`.
pub fn repo_files(repo: &Path) -> Vec<String> {
    fn walk(root: &Path, d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if e.file_name() == ".git" {
                continue;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                walk(root, &p, out);
            } else if ft.is_file() {
                if let Ok(r) = p.strip_prefix(root) {
                    out.push(r.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    let mut out = vec![];
    walk(repo, repo, &mut out);
    out.sort();
    out
}

/// Glob to regex: `**/` any directories, `**` anything, `*` within a segment, `?` one char.
pub fn glob_regex(glob: &str) -> regex::Regex {
    let mut r = String::from("^");
    let c: Vec<char> = glob.chars().collect();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '*' if c.get(i + 1) == Some(&'*') => {
                if c.get(i + 2) == Some(&'/') {
                    r.push_str("(?:.*/)?");
                    i += 3;
                } else {
                    r.push_str(".*");
                    i += 2;
                }
                continue;
            }
            '*' => r.push_str("[^/]*"),
            '?' => r.push_str("[^/]"),
            ch => r.push_str(&regex::escape(&ch.to_string())),
        }
        i += 1;
    }
    r.push('$');
    regex::Regex::new(&r).expect("glob regex")
}

fn rule_holds(rule: &Value, repo: &Path, files: &[String]) -> bool {
    let re = glob_regex(rule["path"].as_str().unwrap_or(""));
    let pat = rule["pattern"].as_str().unwrap_or("");
    let mut hits = files.iter().filter(|f| re.is_match(f));
    match rule["rule"].as_str().unwrap_or("") {
        "file-exists" => hits.next().is_some(),
        "file-contains" => hits.any(|f| fs::read_to_string(repo.join(f)).map_or(false, |t| t.contains(pat))),
        "package-reference" => {
            let needles = [
                format!("Include=\"{pat}\""),
                format!("\"{pat}\":"),
                format!("\n{pat} ="),
                format!("\n{pat}="),
            ];
            hits.any(|f| fs::read_to_string(repo.join(f)).map_or(false, |t| {
                let t = format!("\n{t}");
                needles.iter().any(|n| t.contains(n.as_str()))
            }))
        }
        _ => false,
    }
}

// ---------- line protocol ----------

/// How plugin processes are kept off the network. The contract names no sandbox (D36);
/// the host chooses one here.
fn isolated_command(exe: &Path, args: &[String]) -> Result<Command, Error> {
    let have = |t: &str| Command::new(t).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok();
    if Command::new("unshare").args(["-rn", "true"]).stdout(Stdio::null()).stderr(Stdio::null()).status().map_or(false, |s| s.success()) {
        let mut c = Command::new("unshare");
        c.arg("-rn");
        // Loopback inside the plugin's own namespace is not outside network: bring `lo` up.
        if Command::new("ip").arg("-V").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() {
            c.args(["sh", "-c", "ip link set lo up; exec \"$0\" \"$@\""]);
        }
        c.arg(exe).args(args);
        Ok(c)
    } else if have("bwrap") {
        let mut c = Command::new("bwrap");
        c.args(["--bind", "/", "/", "--dev", "/dev", "--proc", "/proc", "--unshare-net", "--die-with-parent", "--"]).arg(exe).args(args);
        Ok(c)
    } else {
        err("no network isolation available (unshare -rn, bwrap)")
    }
}

/// A long-lived plugin process: one JSON message per line on stdin and stdout.
pub struct PluginProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    pub workdir: PathBuf,
}

impl PluginProcess {
    /// Start `exe args` in `workdir` (created if missing) with no network.
    pub fn spawn(exe: &Path, args: &[String], workdir: &Path) -> Result<PluginProcess, Error> {
        PluginProcess::spawn_env(exe, args, workdir, &[])
    }

    /// Like `spawn`, with extra environment variables for the plugin.
    pub fn spawn_env(exe: &Path, args: &[String], workdir: &Path, env: &[(String, String)]) -> Result<PluginProcess, Error> {
        fs::create_dir_all(workdir).map_err(|e| Error(e.to_string()))?;
        let exe = fs::canonicalize(exe).map_err(|e| Error(format!("{}: {e}", exe.display())))?;
        let mut c = isolated_command(&exe, args)?;
        c.envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        let mut child = c.current_dir(workdir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
            .spawn().map_err(|e| Error(format!("spawn {}: {e}", exe.display())))?;
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Ok(PluginProcess { child, stdin, stdout, next_id: 1, workdir: workdir.to_path_buf() })
    }

    pub fn start(m: &Manifest, workdir: &Path) -> Result<PluginProcess, Error> {
        PluginProcess::spawn(&m.executable(), &m.args(), workdir)
    }

    /// Like `start`, with `PLUGIN_BUNDLE_DIR=<absolute bundle_dir>` in the plugin's environment.
    pub fn start_with_bundle(m: &Manifest, workdir: &Path, bundle_dir: &Path) -> Result<PluginProcess, Error> {
        let b = fs::canonicalize(bundle_dir).map_err(|e| Error(format!("{}: {e}", bundle_dir.display())))?;
        let env = [("PLUGIN_BUNDLE_DIR".to_string(), b.to_string_lossy().to_string())];
        PluginProcess::spawn_env(&m.executable(), &m.args(), workdir, &env)
    }

    pub fn os_pid(&self) -> u32 {
        self.child.id()
    }

    /// Send one request (an `id` is assigned) and read its one-line reply.
    /// Replies are checked against `plugin-reply.schema.json`, and every result type against the eight.
    pub fn request(&mut self, mut req: Value) -> Result<Value, Error> {
        let id = self.next_id;
        self.next_id += 1;
        req["id"] = json!(id);
        self.relay(req)
    }

    /// Send a request with the caller's own `id` unchanged and read the reply with that id.
    pub fn relay(&mut self, req: Value) -> Result<Value, Error> {
        let id = req["id"].as_u64().ok_or_else(|| Error("request has no integer id".into()))?;
        let line = serde_json::to_string(&req).unwrap();
        writeln!(self.stdin, "{line}").and_then(|_| self.stdin.flush()).map_err(|e| Error(format!("write: {e}")))?;
        let mut buf = String::new();
        let n = self.stdout.read_line(&mut buf).map_err(|e| Error(format!("read: {e}")))?;
        if n == 0 {
            return err("plugin closed its output");
        }
        let reply: Value = serde_json::from_str(buf.trim_end()).map_err(|e| Error(format!("reply is not one JSON message: {e}")))?;
        if reply["id"].as_u64() != Some(id) {
            return err(format!("reply id {} for request {id}", reply["id"]));
        }
        let s = contracts::Schemas::contract().map_err(|e| Error(e.to_string()))?;
        s.validate("plugin-reply", &reply, "reply").map_err(|e| Error(e.to_string()))?;
        Ok(reply)
    }

    pub fn describe(&mut self, old_commit: &str, new_commit: &str) -> Result<Value, Error> {
        let wd = self.workdir.to_string_lossy().to_string();
        self.request(json!({"op": "describe", "workdir": wd, "bundle": {"oldCommit": old_commit, "newCommit": new_commit}}))
    }

    pub fn run(&mut self, subject: &str, inputs: Value, sides: &[&str], preset: Value) -> Result<Value, Error> {
        self.request(json!({"op": "run", "subject": subject, "inputs": inputs, "sides": sides, "preset": preset}))
    }

    /// Ask the plugin to stop and wait for it.
    pub fn shutdown(mut self) -> Result<(), Error> {
        let r = self.request(json!({"op": "shutdown"}));
        drop(self.stdin);
        let _ = self.child.wait();
        r.map(|_| ())
    }
}

// ---------- viewers (D38) ----------

#[derive(Debug, Clone, PartialEq)]
pub enum ViewerKind {
    /// A plugin whose detection matched.
    Plugin(String),
    /// Run the changed program on old and new and compare.
    RunAndCompare,
    /// A caller generated from the changed functions' signatures.
    SignatureCaller,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Viewer {
    pub kind: ViewerKind,
    pub name: String,
    pub outputs: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Viewers {
    pub viewers: Vec<Viewer>,
    /// Tabs only when more than one viewer applies.
    pub tabs: bool,
}

/// Build files that make a repository runnable by run-and-compare.
pub const RUNNABLE: [&str; 8] = ["Cargo.toml", "*.csproj", "*.sln", "package.json", "go.mod", "Makefile", "pyproject.toml", "pom.xml"];

pub fn runnable(repo: &Path) -> bool {
    let res: Vec<regex::Regex> = RUNNABLE.iter().map(|g| glob_regex(&format!("**/{g}"))).collect();
    repo_files(repo).iter().any(|f| res.iter().any(|r| r.is_match(f)))
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.into(), b.into())
}

/// The viewer ladder: matching plugins; else run-and-compare when the code can be run;
/// else a caller from signatures. The pull request's tests are never a viewer.
pub fn choose_viewers(repo: &Path, plugins: &[Manifest]) -> Viewers {
    let mut viewers: Vec<Viewer> = plugins.iter().filter(|m| m.detect(repo))
        .map(|m| Viewer { kind: ViewerKind::Plugin(m.name().into()), name: m.name().into(), outputs: m.outputs() })
        .collect();
    if viewers.is_empty() {
        viewers.push(if runnable(repo) {
            Viewer { kind: ViewerKind::RunAndCompare, name: "run-and-compare".into(),
                outputs: vec![pair("output", "text-diff"), pair("exit", "facts")] }
        } else {
            Viewer { kind: ViewerKind::SignatureCaller, name: "signature-caller".into(),
                outputs: vec![pair("result", "facts"), pair("diagnostics", "diagnostics")] }
        });
    }
    let tabs = viewers.len() > 1;
    Viewers { viewers, tabs }
}

/// Generated-form field for a parameter type (D35): `None` marks it not constructible.
pub fn form_field(ty: &str) -> Option<&'static str> {
    match ty.trim() {
        "int" | "long" | "short" | "byte" | "uint" | "ulong" | "i32" | "i64" | "u32" | "u64" | "usize" | "number" => Some("int"),
        "float" | "double" | "decimal" | "f32" | "f64" => Some("number"),
        "string" | "String" | "&str" | "char" => Some("string"),
        "bool" | "boolean" => Some("bool"),
        _ => None,
    }
}

/// Slots for a signature's parameters `(name, type)`, shaped as `plugin-reply` slots.
pub fn signature_slots(params: &[(&str, &str)]) -> Vec<Value> {
    params.iter().map(|(n, t)| {
        let f = form_field(t);
        json!({"name": n, "role": n, "type": t, "constructible": f.is_some(), "field": f})
    }).collect()
}
