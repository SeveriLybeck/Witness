//! Two forms, both writing files.json, parts.json, blocks.json, groups.json:
//! `cutter --repo <git dir> --old <rev> --new <rev> [--references <reply.json>] --out <dir>`
//! `cutter [--references <reply.json>] [--worker "<cmd>"] <stageDir> <bundleDir> <repo> <old> <new>` (bundle builder)
use std::collections::BTreeMap;
use std::path::PathBuf;

const VALUED: &[&str] = &["--repo", "--old", "--new", "--out", "--references", "--worker", "--comment-run", "--comment-ratio", "--wt-old", "--wt-new"];

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut opts: BTreeMap<String, String> = BTreeMap::new();
    let mut pos: Vec<String> = vec![];
    let mut i = 0;
    while i < a.len() {
        if VALUED.contains(&a[i].as_str()) && i + 1 < a.len() {
            opts.insert(a[i].clone(), a[i + 1].clone());
            i += 2;
        } else {
            pos.push(a[i].clone());
            i += 1;
        }
    }
    let (repo, old, new, out) = match (opts.get("--repo"), opts.get("--old"), opts.get("--new"), opts.get("--out")) {
        (Some(r), Some(o), Some(n), Some(d)) => (r.clone(), o.clone(), n.clone(), d.clone()),
        _ if pos.len() == 5 => (pos[2].clone(), pos[3].clone(), pos[4].clone(), pos[0].clone()),
        _ => {
            eprintln!("usage: cutter --repo <dir> --old <rev> --new <rev> [--references <file>] [--worker <cmd>] --out <dir>\n       cutter [options] <stageDir> <bundleDir> <repo> <old> <new>");
            std::process::exit(2);
        }
    };
    let mut o = cutter::Options::default();
    if let Some(x) = opts.get("--comment-run") { o.comment_run = x.parse().expect("number"); }
    if let Some(x) = opts.get("--comment-ratio") { o.comment_ratio_den = x.parse().expect("number"); }
    let references = opts.get("--references").cloned().or_else(|| std::env::var("WITNESS_REFERENCES").ok().filter(|s| !s.is_empty()));
    let worker = opts.get("--worker").cloned().or_else(|| std::env::var("WITNESS_WORKER").ok().filter(|s| !s.is_empty()));
    let repo = PathBuf::from(repo);
    let fail = |e: String| -> ! {
        eprintln!("cutter: {e}");
        std::process::exit(1);
    };
    let c = if let Some(p) = references {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| fail(format!("{p}: {e}"))))
            .unwrap_or_else(|e| fail(format!("{p}: {e}")));
        cutter::cut_repo(&repo, &old, &new, &cutter::references_from_reply(&v), &o)
    } else if let Some(w) = worker {
        {
        let wt = |k: &str, e: &str| opts.get(k).cloned().or_else(|| std::env::var(e).ok().filter(|s| !s.is_empty())).map(PathBuf::from);
        cutter::cut_repo_with_worker(&repo, &old, &new, &w, (wt("--wt-old", "WITNESS_WT_OLD"), wt("--wt-new", "WITNESS_WT_NEW")), &o)
    }
    } else {
        cutter::cut_repo(&repo, &old, &new, &Default::default(), &o)
    }
    .unwrap_or_else(|e| fail(e));
    c.write(&PathBuf::from(&out)).unwrap_or_else(|e| fail(e.to_string()));
}
