//! `differ --old model.old.json --new model.new.json [--out solver-rows.json] [--detail detail.json]
//!         [--disable <contract>]... [--first-row N] [--no-dependency]`

use differ::{detail_json, diff_models, model::Model, solver_rows_json, Options};
use std::fs;

fn main() {
    if let Err(e) = real() {
        eprintln!("differ: {e}");
        std::process::exit(1);
    }
}

fn real() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut old, mut new, mut out, mut detail) = (None, None, None, None);
    let mut opts = Options::default();
    let (mut bridge, mut rows_out, mut scope_in, mut scope_out, mut first_difference) = (None, None, None, None, 1usize);
    while let Some(a) = args.next() {
        let mut val = || args.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--old" => old = Some(val()?),
            "--new" => new = Some(val()?),
            "--out" => out = Some(val()?),
            "--detail" => detail = Some(val()?),
            "--disable" => opts.disabled.push(val()?),
            "--first-row" => opts.first_row = val()?.parse().map_err(|_| "--first-row needs a number")?,
            "--no-dependency" => opts.no_dependency = true,
            "--new-only" => opts.new_only = true,
            "--file" => opts.file = Some(val()?),
            "--changed" => {
                for x in val()?.split(',').filter(|x| !x.is_empty()) {
                    opts.changed.push(x.trim().parse().map_err(|_| format!("--changed: bad line {x}"))?);
                }
            }
            "--bridge" => bridge = Some(val()?),
            "--rows" => rows_out = Some(val()?),
            "--scope-in" => scope_in = Some(val()?),
            "--scope-out" => scope_out = Some(val()?),
            "--first-difference" => first_difference = val()?.parse().map_err(|_| "--first-difference needs a number")?,
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    let read = |p: Option<String>, w: &str| -> Result<Model, String> {
        let p = p.ok_or_else(|| format!("--{w} is required"))?;
        Model::from_str(&fs::read_to_string(&p).map_err(|e| format!("{p}: {e}"))?).map_err(|e| format!("{p}: {e}"))
    };
    let (om, nm) = (read(old, "old")?, read(new, "new")?);
    let r = diff_models(&om, &nm, &opts)?;
    let rows = serde_json::to_string_pretty(&solver_rows_json(&r)).unwrap();
    match out {
        Some(p) => fs::write(&p, rows + "\n").map_err(|e| format!("{p}: {e}"))?,
        None => println!("{rows}"),
    }
    let write = |p: &str, v: &serde_json::Value| fs::write(p, serde_json::to_string_pretty(v).unwrap() + "\n").map_err(|e| format!("{p}: {e}"));
    if let Some(p) = detail {
        write(&p, &detail_json(&r))?;
    }
    if let Some(p) = bridge {
        write(&p, &differ::bridge::bridge_json(&om, &nm, &r))?;
    }
    if let Some(p) = rows_out {
        write(&p, &differ::rows_record(&[&r], first_difference)?)?;
    }
    match (scope_in, scope_out) {
        (Some(i), Some(o)) => {
            let mut s: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&i).map_err(|e| format!("{i}: {e}"))?).map_err(|e| format!("{i}: {e}"))?;
            differ::fill_scope(&mut s, &[&r]);
            write(&o, &s)?;
        }
        (None, None) => {}
        _ => return Err("--scope-in and --scope-out go together".into()),
    }
    Ok(())
}
