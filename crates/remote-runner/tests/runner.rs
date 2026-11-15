use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use remote_runner::*;

fn runner() -> LocalRunner {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("remote-runner");
    LocalRunner::new(base).unwrap()
}

fn lim(cpu: u64, mem: u64, wall: u64) -> Limits {
    Limits { cpu_secs: cpu, memory_mb: mem, wall: Duration::from_secs(wall), processes: 64 }
}

#[test]
fn fallbacks_line1_is_contract_hash() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let out = std::process::Command::new("git").args(["-C", root, "log", "-1", "--format=%H", "--", "contracts/"]).output();
    let first = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/FALLBACKS.md")).unwrap();
    let first = first.lines().next().unwrap().to_string();
    assert_eq!(first.len(), 40);
    if let Ok(o) = out {
        if o.status.success() {
            assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), first);
        }
    }
}

#[test]
fn runs_non_root_offline() {
    let r = runner().run(&Job::sh("id", "id -u; echo hi > /out/x && cat /out/x")).unwrap();
    assert_eq!(r.exit, Some(0), "{}", r.stderr);
    let mut l = r.stdout.lines();
    assert_ne!(l.next().unwrap(), "0");
    assert_eq!(l.next().unwrap(), "hi");
}

#[test]
fn filesystem_read_only_outside_out_and_tmp() {
    let r = runner().run(&Job::sh("ro", "for p in /x /nix/store/x /etc/x \"$HOME/../x\"; do touch $p 2>/dev/null && echo W $p; done; touch /tmp/ok && echo tmp")).unwrap();
    assert_eq!(r.stdout.trim(), "tmp", "{}", r.stderr);
}

#[test]
fn fresh_root_per_run() {
    let rn = runner();
    rn.run(&Job::sh("a", "echo left > /out/mark")).unwrap();
    let r = rn.run(&Job::sh("b", "ls -A /out | wc -l")).unwrap();
    assert_eq!(r.stdout.trim(), "0");
}

#[test]
fn no_environment_leaks() {
    std::env::set_var("WITNESS_SECRET_TEST", "s3cret");
    let r = runner().run(&Job::sh("env", "env")).unwrap();
    assert!(!r.stdout.contains("s3cret"));
}

#[test]
fn wall_time_limit_kills() {
    let t = Instant::now();
    let r = runner().run(&Job::sh("sleep", "sleep 30").limits(lim(60, 512, 1))).unwrap();
    assert!(r.timed_out);
    assert_eq!(r.exit, None);
    assert!(t.elapsed() < Duration::from_secs(10));
}

#[test]
fn cpu_limit_kills() {
    let r = runner().run(&Job::sh("spin", "while :; do :; done").limits(lim(1, 512, 20))).unwrap();
    assert!(!r.timed_out);
    assert_ne!(r.exit, Some(0));
}

#[test]
fn memory_limit_enforced() {
    let r = runner().run(&Job::sh("mem", "x=$(head -c 300000000 /dev/zero | tr '\\0' a); echo ${#x}").limits(lim(30, 128, 30))).unwrap();
    assert_ne!(r.stdout.trim(), "300000000");
}

#[test]
fn remote_is_unavailable() {
    let r: &dyn Runner = &RemoteRunner { endpoint: None };
    assert!(matches!(r.run(&Job::sh("x", "true")), Err(RunError::Unavailable(_))));
}

fn vis() -> StaticVisibility {
    StaticVisibility(HashMap::from([("open/repo".to_string(), true), ("acme/secret".to_string(), false)]))
}

#[test]
fn links() {
    assert_eq!(accept_link("https://github.com/open/repo.git/", &vis()).unwrap().url(), "https://github.com/open/repo");
    assert_eq!(accept_link("https://github.com/acme/secret", &vis()), Err(Rejected::Private));
    assert_eq!(accept_link("https://x:tok@github.com/open/repo", &vis()), Err(Rejected::Credentials));
    assert_eq!(accept_link("https://gitlab.com/open/repo", &vis()), Err(Rejected::NotGithub));
    assert_eq!(accept_link("git@github.com:open/repo", &vis()), Err(Rejected::NotGithub));
    assert_eq!(accept_link("http://github.com/open/repo", &vis()), Err(Rejected::NotGithub));
    for bad in ["https://github.com/open", "https://github.com/open/repo?token=1", "https://github.com/../etc", "https://github.com/a/b/c", "https://github.com/<script>/x"] {
        assert_eq!(parse_link(bad), Err(Rejected::Malformed), "{bad}");
    }
}

#[test]
fn rate_limit() {
    let rl = RateLimiter::new(2, Duration::from_secs(60));
    let t = Instant::now();
    assert!(rl.check_at("k", t).is_ok());
    assert!(rl.check_at("k", t).is_ok());
    assert!(matches!(rl.check_at("k", t), Err(Rejected::RateLimited { .. })));
    assert!(rl.check_at("other", t).is_ok());
    assert!(rl.check_at("k", t + Duration::from_secs(61)).is_ok());
}

#[test]
fn script_tag_escaped() {
    let r = runner().run(&Job::sh("xss", "echo '<script>alert(\"x\")</script>' '&'")).unwrap();
    let h = r.stdout_html();
    assert!(!h.contains('<') && !h.contains('>') && !h.contains('"'));
    assert!(h.contains("&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp;"));
    assert_eq!(escape_html("a'`\u{7}"), "a&#39;&#96;&#7;");
}
