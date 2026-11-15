//! Module form: `rules <stageDir> <bundleDir> <repo> <old> <new>` reads records from `<bundleDir>`
//! (else `<stageDir>`), writes levels.json, mode.json, questions.json to `<stageDir>`.
//! Short form: `rules <bundle-dir> [<out-dir>]`. Either takes `--config rules.json`.
use std::path::PathBuf;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut cfg = rules::Config::default();
    if let Some(i) = args.iter().position(|a| a == "--config") {
        let p = args.get(i + 1).cloned().unwrap_or_default();
        cfg = rules::Config::load(&PathBuf::from(&p)).unwrap_or_else(|e| fail(&e));
        args.drain(i..(i + 2).min(args.len()));
    }
    let (inp, out) = match args.len() {
        5 => {
            let (stage, bundle) = (PathBuf::from(&args[0]), PathBuf::from(&args[1]));
            (rules::Inputs::read_dirs(&[&bundle, &stage]), stage)
        }
        1 | 2 => {
            let dir = PathBuf::from(&args[0]);
            let out = args.get(1).map(PathBuf::from).unwrap_or_else(|| dir.clone());
            (rules::Inputs::read_dir(&dir), out)
        }
        _ => fail("usage: rules <stageDir> <bundleDir> <repo> <old> <new> | rules <bundle-dir> [<out-dir>] [--config rules.json]"),
    };
    let inp = inp.unwrap_or_else(|e| fail(&e));
    std::fs::create_dir_all(&out).unwrap_or_else(|e| fail(&format!("{}: {e}", out.display())));
    let (l, m, q) = rules::run(&inp, &cfg);
    for (n, v) in [("levels", l), ("mode", m), ("questions", q)] {
        let p = out.join(format!("{n}.json"));
        std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap() + "\n").unwrap_or_else(|e| fail(&format!("{}: {e}", p.display())));
    }
}

fn fail(m: &str) -> ! { eprintln!("{m}"); std::process::exit(1) }
