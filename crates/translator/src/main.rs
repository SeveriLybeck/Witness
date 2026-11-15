//! `translator lower --cfg <worker-reply.json> --unit <graph> --version old|new [--unit-id ID]
//!     [--inline SYM]... [--delegate FIELD=GRAPH]... [--derive-delegates LINES] [--part L,L,..]... [--disable CONTRACT]...`
//!   prints a model record (model.schema.json).
//! `translator scope <input.json>` with `{"changedLines": {...}, "production": [{"side","file","line"}], "models": [{"file", "model"}]}`
//!   prints scope.json.

use std::collections::BTreeSet;
use translator::{derive_delegates, lower, scope, Line, Options};

fn lines(s: &str) -> Vec<u64> {
    s.split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

fn read(p: &str) -> serde_json::Value {
    let t = std::fs::read_to_string(p).unwrap_or_else(|e| die(&format!("{p}: {e}")));
    serde_json::from_str(&t).unwrap_or_else(|e| die(&format!("{p}: {e}")))
}

fn die(m: &str) -> ! {
    eprintln!("translator: {m}");
    std::process::exit(2)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("lower") => {
            let mut o = Options { version: "new".into(), ..Default::default() };
            let mut cfg = None;
            let mut derive: Option<BTreeSet<u64>> = None;
            let mut i = 1;
            while i < a.len() {
                if a[i] == "--exact" {
                    o.exact = true;
                    i += 1;
                    continue;
                }
                if a[i] == "--per-iteration" {
                    o.per_iteration = true;
                    i += 1;
                    continue;
                }
                let v =a.get(i + 1).cloned().unwrap_or_else(|| die(&format!("{} needs a value", a[i])));
                match a[i].as_str() {
                    "--cfg" => cfg = Some(v),
                    "--unit" => o.unit = v,
                    "--unit-id" => o.unit_id = Some(v),
                    "--version" => o.version = v,
                    "--inline" => {
                        o.inline.insert(v);
                    }
                    "--delegate" => {
                        let (f, g) = v.split_once('=').unwrap_or_else(|| die("--delegate FIELD=GRAPH"));
                        o.delegates.insert(f.into(), g.into());
                    }
                    "--derive-delegates" => derive = Some(lines(&v).into_iter().collect()),
                    "--part" => o.parts.push(lines(&v)),
                    "--site" => o.region.extend(lines(&v)),
                    "--per-iteration" => {
                        o.per_iteration = true;
                        i += 1;
                        continue;
                    }
                    "--disable" => {
                        o.disabled.insert(v);
                    }
                    x => die(&format!("unknown option {x}")),
                }
                i += 2;
            }
            let reply = read(&cfg.unwrap_or_else(|| die("--cfg missing")));
            if let Some(ch) = derive {
                if let Ok(r) = translator::cfg_result(&reply) {
                    o.delegates.extend(derive_delegates(r, &ch));
                }
            }
            match lower(&reply, &o) {
                Ok(m) => println!("{}", serde_json::to_string_pretty(&m).unwrap()),
                Err(e) => die(&e.0),
            }
        }
        Some("region") => {
            // translator region <reply.json> <graph> <line,line>: prints the block ordinals of the site unit
            let reply = read(a.get(1).unwrap_or_else(|| die("region <reply> <graph> <lines>")));
            let cfg = translator::cfg_result(&reply).unwrap_or_else(|e| die(&e.0));
            let g = cfg["methods"].as_array().unwrap().iter().find(|m| m["name"] == a[2].as_str()).unwrap_or_else(|| die("no graph"));
            println!("{:?}", translator::region_blocks(g, &lines(&a[3])));
        }
        Some("scope") => {
            let inp = read(a.get(1).unwrap_or_else(|| die("scope <input.json>")));
            let prod: Vec<Line> = inp["production"].as_array().into_iter().flatten()
                .map(|l| Line { side: l["side"].as_str().unwrap_or("new").into(), file: l["file"].as_str().unwrap_or("").into(), line: l["line"].as_u64().unwrap_or(0) })
                .collect();
            let models: Vec<(String, serde_json::Value)> = inp["models"].as_array().into_iter().flatten()
                .map(|m| (m["file"].as_str().unwrap_or("").to_string(), m["model"].clone()))
                .collect();
            // optional `reasons`: [{side, file, line, endLine, reason}] for changed lines no unit covers
            let reasons: Vec<(Line, u64, String)> = inp["reasons"].as_array().into_iter().flatten()
                .map(|r| (Line { side: r["side"].as_str().unwrap_or("new").into(), file: r["file"].as_str().unwrap_or("").into(), line: r["line"].as_u64().unwrap_or(0) },
                    r["endLine"].as_u64().unwrap_or(0), r["reason"].as_str().unwrap_or("").to_string()))
                .collect();
            let _ = scope;
            println!("{}", serde_json::to_string_pretty(&translator::scope_with(&inp["changedLines"], &prod, &models, &reasons)).unwrap());
        }
        _ => die("usage: translator lower ... | translator scope <input.json>"),
    }
}
