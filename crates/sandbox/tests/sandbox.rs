use sandbox::{workspace_root, Net, Queue, Sandbox, Step, BASE_COMMIT};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn sb(name: &str) -> Sandbox {
    let ws = workspace_root();
    Sandbox::new(ws.join("work/sandbox-test").join(name), ws.join("work/nuget")).expect("sandbox")
}

// 1.1
#[test]
fn fallbacks_line_one_is_contract_hash_and_fixtures_validate() {
    let ws = workspace_root();
    let out = Command::new("git").args(["log", "-1", "--format=%H", "--", "contracts/"]).current_dir(&ws).output().unwrap();
    let hash = String::from_utf8(out.stdout).unwrap().trim().to_string();
    assert_eq!(hash.len(), 40);
    let text = std::fs::read_to_string(ws.join("crates/sandbox/FALLBACKS.md")).unwrap();
    assert_eq!(text.lines().next().unwrap().trim(), hash);
    for c in ["2283", "2234", "2129", "2297"] {
        let d = ws.join("contracts/fixtures").join(c);
        contracts::validate_dir(&d, &contracts::Options { repo: None })
            .unwrap_or_else(|f| panic!("{}: {}: {}", d.display(), f.path, f.message));
    }
}

// 1.2
#[test]
fn dotnet_runs_inside_and_network_is_off() {
    let s = sb("basic");
    let v = s.run(&Step::new("dotnet-version", Net::Off, &["dotnet", "--version"]));
    assert!(v.ok(), "{v:?}");
    assert!(v.stdout.trim().starts_with("8."), "{}", v.stdout);

    let p = s.run(&Step::sh("passwd", Net::Off, "while read -r l; do echo \"$l\"; done < /etc/passwd"));
    assert_eq!(p.stdout.lines().count(), 1, "{p:?}");

    let e = s.run(&Step::sh("env", Net::Off, "echo \"${USER:-}|${SSH_AUTH_SOCK:-}|$HOME\""));
    assert_eq!(e.stdout.trim(), format!("||{}/home", s.root.display()));

    let w = s.run(&Step::sh("ro-store", Net::Off, "echo x > /nix/store/sandbox-probe"));
    assert!(!w.ok());

    let n = s.run(&Step::sh("net-probe", Net::Off, "exec 3<>/dev/tcp/1.1.1.1/53"));
    assert!(!n.ok(), "network call succeeded offline: {n:?}");

    let steps = s.steps();
    let probe = steps.iter().rev().find(|x| x["step"] == "net-probe").unwrap();
    assert_eq!(probe["network"], "off");
}

// 1.3
#[test]
fn checks_out_base_and_2283_head() {
    let s = sb("checkout");
    let base = s.working_copy(BASE_COMMIT).expect("base");
    assert!(base.commit.starts_with(BASE_COMMIT));
    assert!(base.abs.join("src/MessagePack/MessagePack.csproj").is_file());
    let head = s.working_copy("origin/ju26-issue-2283").expect("2283 head");
    assert_eq!(head.commit.len(), 40);
    assert_ne!(head.commit, base.commit);
    let parent = s.resolve(&format!("{}^", head.commit)).unwrap();
    assert_eq!(parent, base.commit);
    assert!(head.abs.join(".git").exists());
}

// 1.4
#[test]
fn restore_then_offline_build_of_2283_head() {
    let s = sb("build");
    let q = Queue::shared();
    let wc = s.working_copy("origin/ju26-issue-2283").expect("2283 head");
    let proj = "src/MessagePack/MessagePack.csproj";
    let r = s.restore(&q, &wc, proj);
    assert!(r.ok(), "restore: {}\n{}", r.stdout, r.stderr);
    let b = s.build(&q, &wc, proj, &["-c", "Release", "-f", "net8.0"]);
    assert!(b.ok(), "build: {}\n{}", b.stdout, b.stderr);
    assert!(wc.abs.join("bin/MessagePack/Release/net8.0/MessagePack.dll").is_file());
    let steps = s.steps();
    let net_of = |n: &str| steps.iter().rev().find(|x| x["step"] == n).unwrap()["network"].clone();
    assert_eq!(net_of("restore"), "on");
    assert_eq!(net_of("build"), "off");
}

// 1.5
#[test]
fn queued_builds_never_overlap() {
    let ws = workspace_root();
    let dir = ws.join("work/sandbox-test/queue-locks");
    let _ = std::fs::remove_dir_all(&dir);
    let q = Queue::new(&dir, 1);
    let s = sb("queue");
    let spans = Arc::new(Mutex::new(Vec::new()));
    let hs: Vec<_> = (0..2)
        .map(|i| {
            let (q, s, spans) = (q.clone(), s.clone(), spans.clone());
            std::thread::spawn(move || {
                let t = Instant::now();
                let _slot = q.acquire();
                let a = t.elapsed();
                let r = s.run(&Step::sh(&format!("job{i}"), Net::Off, "sleep 0.6"));
                assert!(r.ok(), "{r:?}");
                spans.lock().unwrap().push((a, t.elapsed()));
            })
        })
        .collect();
    let t0 = Instant::now();
    for h in hs {
        h.join().unwrap();
    }
    // Both threads started at about t0; with one slot the jobs run back to back.
    let mut v = spans.lock().unwrap().clone();
    v.sort();
    assert!(v[1].0 >= v[0].1 - std::time::Duration::from_millis(50), "overlap: {v:?}");
    assert!(t0.elapsed().as_millis() >= 1200);

    // run_heavy goes through the same slots.
    let r = s.run_heavy(&q, &Step::new("heavy-true", Net::Off, &["true"]));
    assert!(r.ok(), "{r:?}");
}

#[test]
fn root_under_tmp_is_visible_inside() {
    let base = std::env::temp_dir().canonicalize().unwrap();
    let d = base.join(format!("sandbox-tmp-root-{}", std::process::id()));
    let n = base.join(format!("sandbox-tmp-nuget-{}", std::process::id()));
    let s = Sandbox::new(&d, &n).expect("tmp root");
    let r = s.run(&Step::sh("tmp-root", Net::Off, "echo hi > probe && echo x > \"$NUGET_PACKAGES/probe\""));
    assert!(r.ok(), "{r:?}");
    assert_eq!(std::fs::read_to_string(d.join("probe")).unwrap().trim(), "hi");
    assert!(n.join("probe").is_file());
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&n);
    assert!(Sandbox::new("/tmp", base.join("sandbox-x")).is_err());
}

#[test]
fn two_threads_clone_one_cold_root() {
    let ws = workspace_root();
    let root = ws.join("work/sandbox-test/race");
    let _ = std::fs::remove_dir_all(&root);
    let hs: Vec<_> = ["origin/ju26-issue-2283", BASE_COMMIT, BASE_COMMIT]
        .into_iter()
        .map(|rev| {
            let s = Sandbox::new(&root, ws.join("work/nuget")).unwrap();
            std::thread::spawn(move || s.working_copy(rev).map(|w| w.abs))
        })
        .collect();
    for h in hs {
        let p = h.join().unwrap().expect("working copy");
        assert!(p.join("src/MessagePack/MessagePack.csproj").is_file());
    }
}

#[test]
fn restores_into_one_folder_are_serialised() {
    let ws = workspace_root();
    let nuget = ws.join("work/sandbox-test/restore-nuget");
    let s = Sandbox::new(ws.join("work/sandbox-test/restore-lock"), &nuget).unwrap();
    let wc = s.working_copy(BASE_COMMIT).unwrap();
    // While the restore lock is held, a restore must wait.
    let g = sandbox::lock_file(&nuget.join(".restore.lock")).unwrap();
    let q = Queue::new(ws.join("work/sandbox-test/restore-locks"), 2);
    let done = Arc::new(Mutex::new(None));
    let d2 = done.clone();
    let h = std::thread::spawn(move || {
        let r = s.restore(&q, &wc, "src/MessagePack.Annotations/MessagePack.Annotations.csproj");
        *d2.lock().unwrap() = Some(Instant::now());
        r
    });
    std::thread::sleep(std::time::Duration::from_millis(1500));
    assert!(done.lock().unwrap().is_none(), "restore ran while the lock was held");
    let released = Instant::now();
    drop(g);
    let r = h.join().unwrap();
    assert!(r.ok(), "{}\n{}", r.stdout, r.stderr);
    assert!(done.lock().unwrap().unwrap() >= released);
}

#[test]
fn recovers_from_missing_clone_and_missing_working_copy() {
    let ws = workspace_root();
    let root = ws.join("work/sandbox-test/recover");
    let _ = std::fs::remove_dir_all(&root);
    let s = Sandbox::new(&root, ws.join("work/nuget")).unwrap();
    let wc = s.working_copy(BASE_COMMIT).unwrap();
    // working copy directory gone but still registered
    std::fs::remove_dir_all(&wc.abs).unwrap();
    let wc = s.working_copy(BASE_COMMIT).expect("re-create working copy");
    assert!(wc.abs.join("src/MessagePack/MessagePack.csproj").is_file());
    // clone gone, only a stray home left
    std::fs::remove_dir_all(root.join("repo")).unwrap();
    let wc = s.working_copy(BASE_COMMIT).expect("re-clone");
    assert!(wc.abs.join("src/MessagePack/MessagePack.csproj").is_file());
    // broken clone (empty .git)
    std::fs::remove_dir_all(root.join("repo/.git")).unwrap();
    std::fs::create_dir(root.join("repo/.git")).unwrap();
    let wc = s.working_copy("origin/ju26-issue-2283").expect("re-clone broken");
    assert!(wc.abs.join("src/MessagePack/MessagePack.csproj").is_file());
    let _ = std::fs::remove_dir_all(&root);
}
