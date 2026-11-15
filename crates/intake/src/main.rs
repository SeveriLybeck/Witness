//! `witness-intake [--work DIR] [--prepare CMD...] [--playground] <link>`
//! Prints progress lines on stderr and the plan JSON on stdout.
use std::path::PathBuf;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut work = PathBuf::from("work/intake");
    let mut prepare: Vec<String> = Vec::new();
    let mut click = false;
    let mut link = None;
    while !args.is_empty() {
        let a = args.remove(0);
        match a.as_str() {
            "--work" if !args.is_empty() => work = PathBuf::from(args.remove(0)),
            "--playground" => click = true,
            "--prepare" => {
                prepare = std::mem::take(&mut args);
            }
            _ => link = Some(a),
        }
    }
    let Some(link) = link else {
        eprintln!("usage: witness-intake [--work DIR] [--playground] <link> [--prepare CMD...]");
        std::process::exit(2);
    };
    let mut progress = |s: &str| eprintln!("{s}");
    let mut run = || -> Result<i32, String> {
        let l = intake::parse_link(&link)?;
        let f = intake::fetch(&l, &work, &mut progress)?;
        let mut plan = intake::plan(&link, &f)?;
        if click {
            intake::start_playground(&mut plan, true)?;
        }
        println!("{}", serde_json::to_string_pretty(&plan).unwrap());
        if prepare.is_empty() {
            return Ok(0);
        }
        intake::run_prepare(&prepare, &f, &plan, &mut progress)
    };
    match run() {
        Ok(c) => std::process::exit(c),
        Err(e) => {
            eprintln!("intake: {e}");
            std::process::exit(1);
        }
    }
}
