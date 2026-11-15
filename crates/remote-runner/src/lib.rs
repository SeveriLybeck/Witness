//! Runner for strangers' code (D45): one interface, a local bwrap implementation, a remote stub.

use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sandbox::{Net, Sandbox, Step, Toolchain};

/// A GitHub link to a public repository, normalised.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoLink {
    pub owner: String,
    pub repo: String,
}

impl RepoLink {
    pub fn url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.repo)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejected {
    NotGithub,
    Malformed,
    Credentials,
    Private,
    Unknown(String),
    RateLimited { retry_after: Duration },
}

/// Answers whether a repository is public.
pub trait Visibility {
    fn is_public(&self, link: &RepoLink) -> Result<bool, String>;
}

/// Unauthenticated GitHub API lookup (a private repository answers 404). Not used by tests.
pub struct GitHubVisibility;

impl Visibility for GitHubVisibility {
    fn is_public(&self, l: &RepoLink) -> Result<bool, String> {
        let out = Command::new("timeout")
            .args(["10", "curl", "-s", "-o", "/dev/null", "-w", "%{http_code}"])
            .arg(format!("https://api.github.com/repos/{}/{}", l.owner, l.repo))
            .output()
            .map_err(|e| e.to_string())?;
        match String::from_utf8_lossy(&out.stdout).trim() {
            "200" => Ok(true),
            "404" => Ok(false),
            c => Err(format!("github: status {c}")),
        }
    }
}

/// Fixed table, for tests and offline use.
pub struct StaticVisibility(pub HashMap<String, bool>);

impl Visibility for StaticVisibility {
    fn is_public(&self, l: &RepoLink) -> Result<bool, String> {
        self.0.get(&format!("{}/{}", l.owner, l.repo).to_lowercase()).copied().ok_or_else(|| "unknown repository".into())
    }
}

fn name_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= 100 && s != "." && s != ".." && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

/// Parse a link: `https://github.com/<owner>/<repo>[.git][/]` only; no credentials, query or fragment.
pub fn parse_link(raw: &str) -> Result<RepoLink, Rejected> {
    let s = raw.trim();
    let rest = s.strip_prefix("https://").ok_or(Rejected::NotGithub)?;
    let (host, path) = rest.split_once('/').ok_or(Rejected::Malformed)?;
    if host.contains('@') {
        return Err(Rejected::Credentials);
    }
    if !host.eq_ignore_ascii_case("github.com") && !host.eq_ignore_ascii_case("www.github.com") {
        return Err(Rejected::NotGithub);
    }
    if path.contains('?') || path.contains('#') {
        return Err(Rejected::Malformed);
    }
    let path = path.trim_end_matches('/');
    let mut parts = path.split('/');
    let (owner, repo) = match (parts.next(), parts.next(), parts.next()) {
        (Some(o), Some(r), None) => (o, r.strip_suffix(".git").unwrap_or(r)),
        _ => return Err(Rejected::Malformed),
    };
    if !name_ok(owner) || !name_ok(repo) {
        return Err(Rejected::Malformed);
    }
    Ok(RepoLink { owner: owner.into(), repo: repo.into() })
}

/// Parse and require a public repository.
pub fn accept_link(raw: &str, vis: &dyn Visibility) -> Result<RepoLink, Rejected> {
    let l = parse_link(raw)?;
    match vis.is_public(&l) {
        Ok(true) => Ok(l),
        Ok(false) => Err(Rejected::Private),
        Err(e) => Err(Rejected::Unknown(e)),
    }
}

/// At most `max` requests per `window` per key (sliding window).
pub struct RateLimiter {
    max: usize,
    window: Duration,
    seen: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new(max: usize, window: Duration) -> RateLimiter {
        RateLimiter { max, window, seen: Mutex::new(HashMap::new()) }
    }
    pub fn check(&self, key: &str) -> Result<(), Rejected> {
        self.check_at(key, Instant::now())
    }
    pub fn check_at(&self, key: &str, now: Instant) -> Result<(), Rejected> {
        let mut m = self.seen.lock().unwrap();
        let v = m.entry(key.to_string()).or_default();
        v.retain(|t| now.saturating_duration_since(*t) < self.window);
        if v.len() >= self.max {
            let retry_after = self.window - now.saturating_duration_since(v[0]);
            return Err(Rejected::RateLimited { retry_after });
        }
        v.push(now);
        Ok(())
    }
}

/// Escape text from a repository for HTML display (element content and quoted attributes).
pub fn escape_html(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            '`' => o.push_str("&#96;"),
            c if c.is_control() && c != '\n' && c != '\t' => o.push_str(&format!("&#{};", c as u32)),
            c => o.push(c),
        }
    }
    o
}

#[derive(Clone, Debug)]
pub struct Limits {
    pub cpu_secs: u64,
    pub memory_mb: u64,
    pub wall: Duration,
    pub processes: u64,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits { cpu_secs: 60, memory_mb: 2048, wall: Duration::from_secs(120), processes: 256 }
    }
}

#[derive(Clone, Debug)]
pub struct Job {
    pub name: String,
    pub argv: Vec<String>,
    pub limits: Limits,
    /// Files placed read-only at `/job` (e.g. a checkout), copied by the caller beforehand.
    pub input: Option<PathBuf>,
}

impl Job {
    pub fn new(name: &str, argv: &[&str]) -> Job {
        Job { name: name.into(), argv: argv.iter().map(|s| s.to_string()).collect(), limits: Limits::default(), input: None }
    }
    pub fn sh(name: &str, script: &str) -> Job {
        Job::new(name, &["bash", "-c", script])
    }
    pub fn limits(mut self, l: Limits) -> Job {
        self.limits = l;
        self
    }
}

#[derive(Clone, Debug)]
pub struct RunRecord {
    pub name: String,
    pub exit: Option<i32>,
    pub timed_out: bool,
    pub millis: u128,
    /// Raw output: never display unescaped.
    pub stdout: String,
    pub stderr: String,
}

impl RunRecord {
    pub fn stdout_html(&self) -> String {
        escape_html(&self.stdout)
    }
    pub fn stderr_html(&self) -> String {
        escape_html(&self.stderr)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunError {
    Unavailable(String),
    Setup(String),
}

/// The one runner interface; Verda and this machine both implement it.
pub trait Runner {
    fn run(&self, job: &Job) -> Result<RunRecord, RunError>;
}

/// Placeholder until access to the remote machine exists.
pub struct RemoteRunner {
    pub endpoint: Option<String>,
}

impl Runner for RemoteRunner {
    fn run(&self, _job: &Job) -> Result<RunRecord, RunError> {
        Err(RunError::Unavailable("remote runner: no access".into()))
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(name)).find(|p| p.is_file()).and_then(|p| p.canonicalize().ok())
}

/// Local implementation: fresh root per run under `base`, bwrap, no network, no environment, read-only root.
pub struct LocalRunner {
    pub base: PathBuf,
    toolchain: Toolchain,
}

static SEQ: AtomicU64 = AtomicU64::new(0);

impl LocalRunner {
    pub fn new(base: impl Into<PathBuf>) -> Result<LocalRunner, RunError> {
        let base = base.into();
        fs::create_dir_all(&base).map_err(|e| RunError::Setup(e.to_string()))?;
        let need = |n: &str| which(n).ok_or_else(|| RunError::Setup(format!("`{n}` not on PATH")));
        let bwrap = need("bwrap")?;
        let bash = need("bash")?;
        let env = need("env")?;
        let mut dirs: Vec<PathBuf> = Vec::new();
        for n in ["bash", "env", "cat", "grep", "sed", "git"] {
            if let Some(p) = which(n) {
                let d = p.parent().unwrap().to_path_buf();
                if d.starts_with("/nix/store") && !dirs.contains(&d) {
                    dirs.push(d);
                }
            }
        }
        let base = base.canonicalize().map_err(|e| RunError::Setup(e.to_string()))?;
        Ok(LocalRunner { base, toolchain: Toolchain { path_dirs: dirs, bwrap, bash, env, ca_file: None } })
    }

    /// The bwrap argv for a job in `root` (exposed for inspection).
    pub fn argv(&self, job: &Job, root: &Path) -> Vec<String> {
        let sb = Sandbox {
            root: root.to_path_buf(),
            nuget: root.join("nuget"),
            sample: PathBuf::from("/nonexistent"),
            toolchain: self.toolchain.clone(),
            memory_max: format!("{}M", job.limits.memory_mb),
        };
        let l = &job.limits;
        let script = format!(
            "ulimit -t {} -v {} -u {} -c 0 || exit 125; exec \"$@\"",
            l.cpu_secs,
            l.memory_mb * 1024,
            l.processes
        );
        let mut inner: Vec<&str> = vec!["bash", "-c", &script, "job"];
        inner.extend(job.argv.iter().map(String::as_str));
        let mut step = Step::new(&job.name, Net::Off, &inner);
        step.env.push(("HOME".into(), "/tmp".into()));
        let mut a = sb.bwrap_argv(&step);
        let r = root.display().to_string();
        // Root read-only; only /out writable.
        for i in 0..a.len().saturating_sub(2) {
            if a[i] == "--bind" && a[i + 1] == r && a[i + 2] == r {
                a[i] = "--ro-bind".into();
            }
        }
        let ci = a.iter().position(|x| x == "--chdir").unwrap();
        let mut extra = vec!["--bind".to_string(), format!("{r}/out"), "/out".into()];
        if let Some(inp) = &job.input {
            extra.extend(["--ro-bind".to_string(), inp.display().to_string(), "/job".into()]);
        }
        extra.extend(["--remount-ro".to_string(), "/".into()]);
        a.splice(ci..ci + 2, extra.into_iter().chain(["--chdir".to_string(), "/out".into()]));
        a
    }
}

impl Runner for LocalRunner {
    fn run(&self, job: &Job) -> Result<RunRecord, RunError> {
        let id = format!("run.{}.{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst));
        let root = self.base.join(&id);
        for d in ["out", "home", ".sb", "nuget"] {
            fs::create_dir_all(root.join(d)).map_err(|e| RunError::Setup(e.to_string()))?;
        }
        fs::write(root.join(".sb/passwd"), "u:x:1000:100:u:/tmp:/bin/sh\n").map_err(|e| RunError::Setup(e.to_string()))?;
        fs::write(root.join(".sb/group"), "users:x:100:\n").map_err(|e| RunError::Setup(e.to_string()))?;
        let argv = self.argv(job, &root);
        let out_p = self.base.join(format!("{id}.out"));
        let err_p = self.base.join(format!("{id}.err"));
        let o = File::create(&out_p).map_err(|e| RunError::Setup(e.to_string()))?;
        let e = File::create(&err_p).map_err(|e| RunError::Setup(e.to_string()))?;
        let start = Instant::now();
        let mut child = Command::new(&argv[0])
            .args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(o)
            .stderr(e)
            .spawn()
            .map_err(|e| RunError::Setup(e.to_string()))?;
        let (exit, timed_out) = loop {
            if let Ok(Some(st)) = child.try_wait() {
                break (st.code(), false);
            }
            if start.elapsed() > job.limits.wall {
                let _ = child.kill();
                let _ = child.wait();
                break (None, true);
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let cap = |p: &Path| {
            let s = fs::read(p).unwrap_or_default();
            String::from_utf8_lossy(&s[..s.len().min(1 << 20)]).into_owned()
        };
        let rec = RunRecord { name: job.name.clone(), exit, timed_out, millis: start.elapsed().as_millis(), stdout: cap(&out_p), stderr: cap(&err_p) };
        let _ = fs::remove_file(&out_p);
        let _ = fs::remove_file(&err_p);
        let _ = fs::remove_dir_all(&root);
        Ok(rec)
    }
}
