//! `generator-lab [--plugin]`: the plugin's line protocol on stdin/stdout.
//! `generator-lab replay <bundle dir> [--proposed <file>] [--out <file>] [--no-test-candidates]`: replays every difference (D23)
//! and writes `replays.json` (default stdout); `--proposed` holds `[{difference, text}]` model snippets.

use generator_lab::{serve, Config, Lab};
use serde_json::Value;
use std::path::Path;

fn fail(m: String) -> ! {
    eprintln!("generator-lab: {m}");
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cfg = Config::load().unwrap_or_else(|e| fail(e));
    match args.first().map(String::as_str) {
        None | Some("--plugin") => serve(Lab::new(cfg)),
        Some("replay") => {
            let mut bundle = None;
            let (mut proposed, mut out, mut tests) = (None, None, None);
            let mut it = args.iter().skip(1);
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--proposed" => proposed = it.next().cloned(),
                    "--out" => out = it.next().cloned(),
                    "--test-candidates" => tests = Some(true),
                    "--no-test-candidates" => tests = Some(false),
                    _ => bundle = Some(a.clone()),
                }
            }
            let bundle = bundle.unwrap_or_else(|| fail("usage: generator-lab replay <bundle dir> [--proposed <file>] [--out <file>]".into()));
            let expected = proposed.is_some();
            let proposed: Vec<Value> = proposed
                .map(|p| std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or_else(|| fail(format!("{p} unreadable"))))
                .and_then(|v| v.as_array().cloned()).unwrap_or_default();
            let mut lab = Lab::for_bundle(cfg, Path::new(&bundle)).unwrap_or_else(|e| fail(e));
            if let Some(t) = tests { lab.test_candidates = t; }
            lab.snippets_expected = expected;
            let r = lab.replay(&proposed).unwrap_or_else(|e| fail(e));
            let text = serde_json::to_string_pretty(&r).unwrap() + "\n";
            match out {
                Some(o) => std::fs::write(&o, text).unwrap_or_else(|e| fail(e.to_string())),
                None => print!("{text}"),
            }
        }
        Some(a) => fail(format!("unknown argument {a}")),
    }
}
