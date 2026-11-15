//! `witness prepare [--data D] [--modules F] [--issues D] [--description T] [--pr N] [--wait] <repo> <old> <new>`
//! `witness finish  [--data D] [--modules F] <repo> <old> <new>` (the test contract; `prepare` starts it in the background)

use bundle::{load_modules, prepare, Builder, Config};
use std::path::PathBuf;
use std::process::{exit, Command, Stdio};

fn usage() -> ! {
    eprintln!("usage: witness prepare|finish [--data D] [--modules F] [--issues D] [--description T] [--pr N] [--wait] <repo> <old> <new>");
    exit(64)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().cloned() else { usage() };
    let mut cfg = Config {
        data: std::env::var_os("WITNESS_DATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("data")),
        issues: std::env::var_os("WITNESS_ISSUES").map(PathBuf::from),
        ..Default::default()
    };
    let mut modules = std::env::var_os("WITNESS_MODULES").map(PathBuf::from);
    let (mut wait, mut pos) = (false, Vec::new());
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().unwrap_or_else(|| usage());
        match a.as_str() {
            "--data" => cfg.data = val().into(),
            "--modules" => modules = Some(val().into()),
            "--issues" => cfg.issues = Some(val().into()),
            "--description" => cfg.description = Some(val()),
            "--pr" => cfg.pull_request = Some(val().parse().unwrap_or_else(|_| usage())),
            "--wait" => wait = true,
            _ => pos.push(a.clone()),
        }
    }
    let [repo, old, new] = <[String; 3]>::try_from(pos).unwrap_or_else(|_| usage());
    cfg.repo = repo.into();
    cfg.old = old;
    cfg.new = new;
    if let Some(m) = &modules {
        cfg.modules = load_modules(m).unwrap_or_else(|e| { eprintln!("witness: {e}"); exit(64) });
    }
    let fail = |e: String| -> ! { eprintln!("witness: {e}"); exit(1) };
    match cmd.as_str() {
        "prepare" => {
            let mut b = prepare(cfg.clone()).unwrap_or_else(|e| fail(e));
            if wait {
                b.finish().unwrap_or_else(|e| fail(e));
            } else {
                let exe = std::env::current_exe().unwrap_or_else(|e| fail(e.to_string()));
                let mut c = Command::new(exe);
                c.arg("finish").arg("--data").arg(&cfg.data);
                if let Some(m) = &modules {
                    c.arg("--modules").arg(m);
                }
                c.arg(&cfg.repo).arg(&b.bundle_old()).arg(&b.bundle_new());
                c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
                c.spawn().unwrap_or_else(|e| fail(e.to_string()));
            }
            println!("{}", b.dir.display());
            exit(if b.prepared() { 0 } else { 2 })
        }
        "finish" => {
            let mut b = Builder::reopen(cfg).unwrap_or_else(|e| fail(e));
            b.finish().unwrap_or_else(|e| fail(e));
            if let Err(e) = b.validate() {
                b.log(&format!("validate: {e}"));
            }
            println!("{}", b.dir.display());
        }
        _ => usage(),
    }
}
