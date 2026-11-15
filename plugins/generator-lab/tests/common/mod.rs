//! Test preparation: builds of old, 2283, 2234, the reverted 2283 filter block and the C# worker,
//! all inside the sandbox (`plugins/generator-lab/work/sb`), heavy steps through the shared queue.
#![allow(dead_code)]

use sandbox::{Net, Queue, Sandbox, Step};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const OLD: &str = "32258fdbee4bcd653c79f51b4376b5103c6f4d27";
pub const NEW_2283: &str = "9df83c38409c8bfad0517aebffe48211eda4a5b4";
pub const NEW_2234: &str = "9007cee1163c4a274fd60c3cc20c21ae63622786";
const MP: &str = "src/MessagePack/MessagePack.csproj";

pub fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
pub fn repo() -> PathBuf {
    here().join("../..").canonicalize().unwrap()
}
pub fn fixture(n: &str) -> PathBuf {
    repo().join("contracts/fixtures").join(n)
}
pub fn read(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

fn which(n: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(n)).find(|p| p.is_file())
}

/// `git` is not in the verify shell; take it from the nix store.
pub fn ensure_git() {
    if which("git").is_some() {
        return;
    }
    let o = std::process::Command::new("nix-build").args(["<nixpkgs>", "-A", "git", "--no-out-link"]).output().expect("nix-build");
    let p = String::from_utf8_lossy(&o.stdout).trim().lines().last().unwrap_or("").to_string();
    assert!(!p.is_empty(), "nix-build git failed");
    let path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{p}/bin:{path}"));
}

pub struct Prep {
    pub sb: Sandbox,
    pub root: PathBuf,
    pub nuget: PathBuf,
}

fn ok(r: sandbox::StepRecord) {
    assert!(r.ok(), "step {} exit {:?}\n{}\n{}", r.step, r.exit, tail(&r.stdout), tail(&r.stderr));
}
fn tail(s: &str) -> String {
    let l: Vec<&str> = s.lines().collect();
    l[l.len().saturating_sub(30)..].join("\n")
}

fn built(dir: &Path) -> bool {
    dir.join("bin/MessagePack.SourceGenerator/Release/netstandard2.0/MessagePack.SourceGenerator.dll").exists()
        && dir.join("bin/MessagePack/Release/net8.0/MessagePack.dll").exists()
}

fn build_tree(sb: &Sandbox, q: &Queue, wc: &sandbox::WorkingCopy) {
    if built(&wc.abs) {
        return;
    }
    let mut r = sb.restore(q, wc, MP);
    for _ in 0..2 {
        if r.ok() {
            break;
        }
        r = sb.restore(q, wc, MP);
    }
    ok(r);
    ok(sb.build(q, wc, MP, &["-c", "Release", "-f", "net8.0", "-v", "q", "-nologo"]));
}

pub fn prep() -> &'static Prep {
    static P: OnceLock<Prep> = OnceLock::new();
    P.get_or_init(|| {
        ensure_git();
        let main_locks = repo().join("../../../work/locks");
        if std::env::var_os("LOCKS").is_none() && main_locks.is_dir() {
            std::env::set_var("LOCKS", &main_locks);
        }
        let root = here().join("work/sb");
        let nuget = here().join("work/nuget");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&nuget).unwrap();
        let sb = Sandbox::new(&root, &nuget).expect("sandbox");
        let q = Queue::shared();
        for rev in [OLD, "origin/ju26-issue-2283", "origin/ju26-issue-2234"] {
            let wc = sb.working_copy(rev).expect("working copy");
            build_tree(&sb, &q, &wc);
        }
        // reverted build of 2283 b1 (parts p1, p5), as the test contract keeps it (D27)
        let rel = format!("rev/{}/b1", &NEW_2283[..12]);
        if !built(&root.join(&rel)) {
            let wc = testcontract::worktree(&sb, &rel, NEW_2283).expect("revert worktree");
            let d = sb.run(&Step::new("diff", Net::Off, &["git", "diff", "-U0", "--no-color", OLD, NEW_2283]).cwd("repo"));
            assert!(d.ok(), "{}", d.stderr);
            let hunks = testcontract::revert::parse_hunks(&d.stdout);
            let parts = read(&fixture("2283/parts.json"));
            let f = "src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs";
            let (mut a, mut del) = (BTreeSet::new(), BTreeSet::new());
            for p in parts["parts"].as_array().unwrap().iter().filter(|p| p["id"] == "p1" || p["id"] == "p5") {
                a.extend(p["addedLines"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize));
                del.extend(p["deletedLines"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize));
            }
            let hs: Vec<_> = hunks.iter().filter(|(hf, _)| hf == f).map(|(_, h)| h.clone()).collect();
            let o = std::fs::read_to_string(root.join("wt").join(OLD).join(f)).unwrap();
            let n = std::fs::read_to_string(root.join("wt").join(NEW_2283).join(f)).unwrap();
            let (t, touched) = testcontract::revert::revert_file(&o, &n, &hs, &a, &del);
            assert!(touched);
            std::fs::write(wc.abs.join(f), t).unwrap();
            build_tree(&sb, &q, &wc);
        }
        // the C# worker, from worker/src
        let dll = root.join("worker/src/Witness.Worker/bin/Release/net8.0/witness-worker.dll");
        if !dll.exists() {
            let dst = root.join("worker");
            let _ = std::fs::remove_dir_all(&dst);
            std::fs::create_dir_all(&dst).unwrap();
            let st = std::process::Command::new("cp").args(["-r"]).arg(repo().join("worker/src")).arg(repo().join("worker/nuget.config")).arg(&dst).status().unwrap();
            assert!(st.success());
            let proj = "worker/src/Witness.Worker/Witness.Worker.csproj";
            ok(sb.run_heavy(&q, &Step::new("worker-restore", Net::On, &["dotnet", "restore", proj, "--configfile", "worker/nuget.config", "-v", "q"])));
            ok(sb.run_heavy(&q, &Step::new("worker-build", Net::Off,
                &["dotnet", "build", proj, "-c", "Release", "--no-restore", "-m:2", "-p:UseSharedCompilation=false", "-v", "q", "-nologo"])));
        }
        std::env::set_var("NUGET_PACKAGES", &nuget);
        std::env::set_var("GENERATOR_LAB_BIN", env!("CARGO_BIN_EXE_generator-lab"));
        Prep { sb, root, nuget }
    })
}
