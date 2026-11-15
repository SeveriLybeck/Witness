//! Sandbox runner (group 1; D4, D42, D43).
//!
//! Everything from the untrusted sample repository runs under `bwrap`: read-only `/nix/store`,
//! one writable root (also the cwd), a stub one-line `/etc/passwd`, `--clearenv`, and no network
//! unless the step asks for it. Each step is recorded with its network state in
//! `<root>/.sb/steps.jsonl`. Heavy steps go through [`Queue`] (at most four at once, each in a
//! `systemd-run --user --scope -p MemoryMax=4G` when available).

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub const SAMPLE_REPO: &str =
    "/home/slopsoftwareagent1/Documents/junction/challenge_material/junction26-messagepack-csharp";
pub const BASE_COMMIT: &str = "32258fdb";

/// Workspace root (two levels above this crate).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace root")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Net {
    Off,
    On,
}

impl Net {
    pub fn as_str(self) -> &'static str {
        match self {
            Net::Off => "off",
            Net::On => "on",
        }
    }
}

/// Store paths the sandbox needs, resolved from the host `PATH` (e.g. a `nix-shell -p` environment).
#[derive(Clone, Debug)]
pub struct Toolchain {
    pub path_dirs: Vec<PathBuf>,
    pub bwrap: PathBuf,
    pub bash: PathBuf,
    pub env: PathBuf,
    pub ca_file: Option<PathBuf>,
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
        .and_then(|p| p.canonicalize().ok())
}

impl Toolchain {
    /// Required: bwrap, bash, env, git, dotnet. Optional: grep, sed, find, curl, cat, ls.
    pub fn from_path() -> Result<Toolchain, String> {
        let need = |n: &str| which(n).ok_or_else(|| format!("sandbox: `{n}` not on PATH"));
        let bwrap = need("bwrap")?;
        let bash = need("bash")?;
        let env = need("env")?;
        let mut dirs: Vec<PathBuf> = Vec::new();
        let mut add = |p: &Path| {
            let d = p.parent().unwrap().to_path_buf();
            if d.starts_with("/nix/store") && !dirs.contains(&d) {
                dirs.push(d);
            }
        };
        for n in ["dotnet", "git"] {
            add(&need(n)?);
        }
        for n in ["env", "bash", "grep", "sed", "find", "curl", "cat", "ls"] {
            if let Some(p) = which(n) {
                add(&p);
            }
        }
        let ca_file = ["SSL_CERT_FILE", "NIX_SSL_CERT_FILE"]
            .iter()
            .filter_map(|v| std::env::var_os(v))
            .map(PathBuf::from)
            .chain([PathBuf::from("/etc/ssl/certs/ca-certificates.crt"), PathBuf::from("/etc/ssl/certs/ca-bundle.crt")])
            .find(|p| p.is_file())
            .and_then(|p| p.canonicalize().ok());
        Ok(Toolchain { path_dirs: dirs, bwrap, bash, env, ca_file })
    }
}

/// One recorded step.
#[derive(Clone, Debug)]
pub struct StepRecord {
    pub step: String,
    pub network: Net,
    pub argv: Vec<String>,
    /// Exit code; `None` when killed (signal or timeout).
    pub exit: Option<i32>,
    pub timed_out: bool,
    pub millis: u128,
    pub stdout: String,
    pub stderr: String,
}

impl StepRecord {
    pub fn ok(&self) -> bool {
        self.exit == Some(0)
    }
    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "step": self.step, "network": self.network.as_str(), "argv": self.argv,
            "exit": self.exit, "timedOut": self.timed_out, "millis": self.millis as u64,
        })
    }
}

/// A step to run inside the sandbox.
#[derive(Clone, Debug)]
pub struct Step {
    pub name: String,
    pub net: Net,
    pub argv: Vec<String>,
    /// Working directory relative to the root.
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    /// Bind the sample repository read-only at `/untrusted`.
    pub src: bool,
    pub timeout: Duration,
}

impl Step {
    pub fn new(name: &str, net: Net, argv: &[&str]) -> Step {
        Step {
            name: name.into(),
            net,
            argv: argv.iter().map(|s| s.to_string()).collect(),
            cwd: None,
            env: Vec::new(),
            src: false,
            timeout: Duration::from_secs(900),
        }
    }
    /// `bash -c script`.
    pub fn sh(name: &str, net: Net, script: &str) -> Step {
        Step::new(name, net, &["bash", "-c", script])
    }
    pub fn cwd(mut self, d: &str) -> Step {
        self.cwd = Some(d.into());
        self
    }
    pub fn src(mut self) -> Step {
        self.src = true;
        self
    }
    pub fn timeout(mut self, t: Duration) -> Step {
        self.timeout = t;
        self
    }
}

/// A sandbox: one writable root plus a shared NuGet package folder.
#[derive(Clone, Debug)]
pub struct Sandbox {
    pub root: PathBuf,
    pub nuget: PathBuf,
    pub sample: PathBuf,
    pub toolchain: Toolchain,
    /// Memory cap for the systemd scope (heavy steps only).
    pub memory_max: String,
}

impl Sandbox {
    /// `nuget` is shared between sandboxes (filled once by restore, then `--no-restore`).
    pub fn new(root: impl Into<PathBuf>, nuget: impl Into<PathBuf>) -> Result<Sandbox, String> {
        let root = root.into();
        let nuget = nuget.into();
        // `/tmp` and `/dev/shm` are fresh tmpfs mounts inside; a path below them is bound on top
        // (later mount wins), but the mount points themselves would be hidden.
        for p in [&root, &nuget] {
            let c = p.canonicalize().unwrap_or_else(|_| p.clone());
            if ["/", "/tmp", "/dev/shm", "/nix", "/nix/store", "/proc", "/dev"].iter().any(|m| c == Path::new(m)) {
                return Err(format!("sandbox: {} cannot be a sandbox root or package folder", c.display()));
            }
        }
        for d in [root.join("home"), root.join(".sb"), nuget.clone()] {
            fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let nuget = nuget.canonicalize().map_err(|e| e.to_string())?;
        let uid = unsafe_uid();
        fs::write(root.join(".sb/passwd"), format!("u:x:{uid}:100:u:{}:/bin/sh\n", root.join("home").display()))
            .map_err(|e| e.to_string())?;
        fs::write(root.join(".sb/group"), "users:x:100:\n").map_err(|e| e.to_string())?;
        let sample = PathBuf::from(std::env::var("SAMPLE_REPO").unwrap_or_else(|_| SAMPLE_REPO.into()));
        Ok(Sandbox { root, nuget, sample, toolchain: Toolchain::from_path()?, memory_max: "4G".into() })
    }

    /// Defaults: root `$SB_ROOT` or `<workspace>/work/sandbox/<name>`, NuGet `$SB_NUGET` or `<workspace>/work/nuget`.
    pub fn named(name: &str) -> Result<Sandbox, String> {
        let ws = workspace_root();
        let root = std::env::var_os("SB_ROOT").map(PathBuf::from).unwrap_or_else(|| ws.join("work/sandbox").join(name));
        let nuget = std::env::var_os("SB_NUGET").map(PathBuf::from).unwrap_or_else(|| ws.join("work/nuget"));
        Sandbox::new(root, nuget)
    }

    /// The full `bwrap` argv for a step (useful for inspection; [`Sandbox::run`] executes it).
    pub fn bwrap_argv(&self, step: &Step) -> Vec<String> {
        let tc = &self.toolchain;
        let r = self.root.display().to_string();
        let s = |x: &str| x.to_string();
        let mut a: Vec<String> = vec![tc.bwrap.display().to_string(), s("--unshare-all")];
        match step.net {
            Net::Off => a.push(s("--unshare-net")),
            Net::On => {
                a.push(s("--share-net"));
                if Path::new("/etc/resolv.conf").exists() {
                    a.extend([s("--ro-bind"), s("/etc/resolv.conf"), s("/etc/resolv.conf")]);
                }
            }
        }
        a.extend(["--die-with-parent", "--new-session", "--clearenv", "--ro-bind", "/nix/store", "/nix/store",
            "--proc", "/proc", "--dev", "/dev", "--perms", "1777", "--tmpfs", "/tmp", "--perms", "1777", "--tmpfs", "/dev/shm"]
            .map(s));
        a.extend([s("--bind"), r.clone(), r.clone()]);
        if !self.nuget.starts_with(&self.root) {
            let n = self.nuget.display().to_string();
            a.extend([s("--bind"), n.clone(), n]);
        }
        a.extend([s("--ro-bind"), format!("{r}/.sb/passwd"), s("/etc/passwd"), s("--ro-bind"), format!("{r}/.sb/group"), s("/etc/group")]);
        let mut ssl = None;
        if let Some(ca) = &tc.ca_file {
            if ca.starts_with("/nix/store") {
                ssl = Some(ca.display().to_string());
            } else {
                a.extend([s("--ro-bind"), ca.display().to_string(), s("/etc/ssl/certs/ca-bundle.crt")]);
                ssl = Some(s("/etc/ssl/certs/ca-bundle.crt"));
            }
        }
        if step.src {
            a.extend([s("--ro-bind"), self.sample.display().to_string(), s("/untrusted")]);
        }
        a.extend([s("--symlink"), tc.bash.display().to_string(), s("/bin/sh")]);
        a.extend([s("--symlink"), tc.env.display().to_string(), s("/usr/bin/env")]);
        let path = tc.path_dirs.iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join(":");
        let home = format!("{r}/home");
        let mut env: Vec<(String, String)> = vec![
            (s("HOME"), home.clone()),
            (s("DOTNET_CLI_HOME"), home),
            (s("NUGET_PACKAGES"), self.nuget.display().to_string()),
            (s("DOTNET_CLI_TELEMETRY_OPTOUT"), s("1")),
            (s("DOTNET_NOLOGO"), s("1")),
            (s("DOTNET_SKIP_FIRST_TIME_EXPERIENCE"), s("1")),
            (s("MSBUILDDISABLENODEREUSE"), s("1")),
            (s("TMPDIR"), s("/tmp")),
            (s("LANG"), s("C.UTF-8")),
            (s("GIT_PAGER"), s("cat")),
            (s("GIT_EDITOR"), s("true")),
            (s("PATH"), path),
        ];
        if let Some(c) = ssl {
            env.push((s("SSL_CERT_FILE"), c));
        }
        env.extend(step.env.iter().cloned());
        for (k, v) in env {
            a.extend([s("--setenv"), k, v]);
        }
        let cwd = match &step.cwd {
            Some(c) => format!("{r}/{c}"),
            None => r,
        };
        a.extend([s("--chdir"), cwd]);
        a.extend(step.argv.iter().cloned());
        a
    }

    /// Run a step directly (light work: git, `--version`).
    pub fn run(&self, step: &Step) -> StepRecord {
        self.exec(step, self.bwrap_argv(step))
    }

    /// Run a heavy step (restore, build, test): waits for a queue slot, then runs in a memory-capped scope.
    pub fn run_heavy(&self, queue: &Queue, step: &Step) -> StepRecord {
        let _slot = queue.acquire();
        let mut argv = self.bwrap_argv(step);
        if scope_available() {
            let mut w: Vec<String> = ["systemd-run", "--user", "--scope", "-q", "-p"].map(String::from).to_vec();
            w.push(format!("MemoryMax={}", self.memory_max));
            w.push("--".into());
            w.append(&mut argv);
            argv = w;
        }
        self.exec(step, argv)
    }

    fn exec(&self, step: &Step, argv: Vec<String>) -> StepRecord {
        let start = Instant::now();
        let out_p = self.root.join(".sb").join(format!("out.{}.{}", std::process::id(), unique()));
        let err_p = out_p.with_extension("err");
        let (exit, timed_out) = match (File::create(&out_p), File::create(&err_p)) {
            (Ok(o), Ok(e)) => match Command::new(&argv[0]).args(&argv[1..]).stdin(Stdio::null()).stdout(o).stderr(e).spawn() {
                Ok(mut child) => loop {
                    if let Ok(Some(st)) = child.try_wait() {
                        break (st.code(), false);
                    }
                    if start.elapsed() > step.timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        break (None, true);
                    }
                    std::thread::sleep(Duration::from_millis(20));
                },
                Err(e) => {
                    let _ = fs::write(&err_p, format!("sandbox: spawn {}: {e}", argv[0]));
                    (None, false)
                }
            },
            _ => (None, false),
        };
        let stdout = fs::read_to_string(&out_p).unwrap_or_default();
        let stderr = fs::read_to_string(&err_p).unwrap_or_default();
        let _ = fs::remove_file(&out_p);
        let _ = fs::remove_file(&err_p);
        let rec = StepRecord {
            step: step.name.clone(),
            network: step.net,
            argv: step.argv.clone(),
            exit,
            timed_out,
            millis: start.elapsed().as_millis(),
            stdout,
            stderr,
        };
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(self.root.join(".sb/steps.jsonl")) {
            let _ = writeln!(f, "{}", rec.json());
        }
        rec
    }

    /// All recorded steps of this root, one JSON object per step.
    pub fn steps(&self) -> Vec<serde_json::Value> {
        fs::read_to_string(self.root.join(".sb/steps.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    // ---- working copies (D42) ----

    /// Clone the sample repository into `<root>/repo` (offline, hooks off) and fetch the change
    /// branches, which are remote-tracking refs in the download. Idempotent.
    pub fn ensure_clone(&self) -> Result<(), String> {
        let _g = lock_file(&self.root.join(".sb/sandbox-clone.lock"))?;
        self.ensure_clone_unlocked()
    }

    fn ensure_clone_unlocked(&self) -> Result<(), String> {
        let healthy = self.root.join("repo/.git").exists()
            && self.run(&Step::new("clone-check", Net::Off, &["git", "rev-parse", "-q", "--verify", "HEAD"]).cwd("repo")).ok();
        if !healthy {
            // missing or broken clone: start over (its worktrees depend on it)
            let _ = fs::remove_dir_all(self.root.join("repo"));
            let _ = fs::remove_dir_all(self.root.join("wt"));
            let r = self.run(&Step::sh("clone", Net::Off,
                "git -c core.hooksPath=/dev/null clone --no-hardlinks -q /untrusted repo").src());
            check(&r)?;
        }
        let r = self.run(&Step::sh("fetch-changes", Net::Off,
            "git -c core.hooksPath=/dev/null fetch -q /untrusted 'refs/remotes/origin/ju26-*:refs/remotes/origin/ju26-*' && git config gc.auto 0")
            .cwd("repo").src());
        check(&r)
    }

    /// Full 40-character commit id of a revision in the clone.
    pub fn resolve(&self, rev: &str) -> Result<String, String> {
        let r = self.run(&Step::new("rev-parse", Net::Off, &["git", "rev-parse", "--verify", "-q", &format!("{rev}^{{commit}}")]).cwd("repo"));
        check(&r)?;
        Ok(r.stdout.trim().to_string())
    }

    /// Working copy for one commit at `<root>/wt/<commit>` (relative path returned). Idempotent.
    pub fn working_copy(&self, rev: &str) -> Result<WorkingCopy, String> {
        let _g = lock_file(&self.root.join(".sb/sandbox-clone.lock"))?;
        self.ensure_clone_unlocked()?;
        let commit = self.resolve(rev)?;
        let rel = format!("wt/{commit}");
        if !self.root.join(&rel).join(".git").exists() {
            // stale registration or half-made directory
            let _ = self.run(&Step::new("worktree-prune", Net::Off, &["git", "worktree", "prune"]).cwd("repo"));
            let _ = fs::remove_dir_all(self.root.join(&rel));
            let r = self.run(&Step::new("worktree", Net::Off,
                &["git", "-c", "core.hooksPath=/dev/null", "worktree", "add", "-q", "-f", "--detach", &format!("../{rel}"), &commit]).cwd("repo"));
            check(&r)?;
        }
        let abs = self.root.join(&rel);
        Ok(WorkingCopy { commit, rel, abs })
    }

    // ---- .NET steps (D43, T4) ----

    /// `dotnet restore <project>` with network on.
    pub fn restore(&self, q: &Queue, wc: &WorkingCopy, project: &str) -> StepRecord {
        // one restore at a time per package folder (T4); taken before the queue slot
        let _g = lock_file(&self.nuget.join(".restore.lock"));
        self.run_heavy(q, &Step::new("restore", Net::On, &["dotnet", "restore", project]).cwd(&wc.rel))
    }

    /// `dotnet build <project> --no-restore` with network off.
    pub fn build(&self, q: &Queue, wc: &WorkingCopy, project: &str, extra: &[&str]) -> StepRecord {
        let mut a = vec!["dotnet", "build", project, "--no-restore", "-m:2", "-p:UseSharedCompilation=false"];
        a.extend_from_slice(extra);
        self.run_heavy(q, &Step::new("build", Net::Off, &a).cwd(&wc.rel))
    }

    /// `dotnet test <project> --no-build`, network on (the tests download reference assemblies, S6).
    pub fn test(&self, q: &Queue, wc: &WorkingCopy, project: &str, extra: &[&str]) -> StepRecord {
        let mut a = vec!["dotnet", "test", project, "--no-build"];
        a.extend_from_slice(extra);
        self.run_heavy(q, &Step::new("test", Net::On, &a).cwd(&wc.rel))
    }
}

#[derive(Clone, Debug)]
pub struct WorkingCopy {
    /// 40-character commit id.
    pub commit: String,
    /// Path relative to the sandbox root (`wt/<commit>`).
    pub rel: String,
    pub abs: PathBuf,
}

fn check(r: &StepRecord) -> Result<(), String> {
    if r.ok() {
        Ok(())
    } else {
        Err(format!("sandbox step `{}` exit {:?}: {}", r.step, r.exit, r.stderr.trim()))
    }
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

fn unsafe_uid() -> String {
    Command::new("id").arg("-u").output().ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "1000".into())
}

/// Whether `systemd-run --user --scope` works here (checked once; `SB_NO_SCOPE=1` disables).
pub fn scope_available() -> bool {
    static S: OnceLock<bool> = OnceLock::new();
    *S.get_or_init(|| {
        std::env::var_os("SB_NO_SCOPE").is_none()
            && Command::new("systemd-run")
                .args(["--user", "--scope", "-q", "-p", "MemoryMax=64M", "true"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
    })
}

// ---- build queue (D4, D42, M1/M2) ----

/// Cross-process build queue: `slots` lock files `heavy.N` in one directory (default `$LOCKS` or
/// `<workspace>/work/locks`, four slots). A job holds one slot for its whole run.
#[derive(Clone, Debug)]
pub struct Queue {
    pub dir: PathBuf,
    pub slots: usize,
}

pub struct Slot {
    _file: File,
    pub index: usize,
}

impl Queue {
    pub fn new(dir: impl Into<PathBuf>, slots: usize) -> Queue {
        let dir = dir.into();
        let _ = fs::create_dir_all(&dir);
        Queue { dir, slots: slots.max(1) }
    }

    pub fn shared() -> Queue {
        let dir = std::env::var_os("LOCKS").map(PathBuf::from).unwrap_or_else(|| workspace_root().join("work/locks"));
        Queue::new(dir, 4)
    }

    fn open(&self, i: usize) -> File {
        OpenOptions::new().create(true).truncate(false).write(true).open(self.dir.join(format!("heavy.{i}"))).expect("lock file")
    }

    /// Take a free slot, or wait on one.
    pub fn acquire(&self) -> Slot {
        for i in 0..self.slots {
            let f = self.open(i);
            if f.try_lock().is_ok() {
                return Slot { _file: f, index: i };
            }
        }
        let i = (std::process::id() as usize + unique() as usize) % self.slots;
        let f = self.open(i);
        f.lock().expect("flock");
        Slot { _file: f, index: i }
    }
}

/// Exclusive cross-process lock on `path` (flock; also excludes other threads, each opens its own file).
/// Not re-entrant: a holder must not take the same file again.
pub fn lock_file(path: &Path) -> Result<File, String> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    let f = OpenOptions::new().create(true).truncate(false).write(true).open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    f.lock().map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(f)
}
