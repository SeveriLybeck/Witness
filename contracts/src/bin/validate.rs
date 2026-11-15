//! `validate [--repo <clone>] <dir>...`: validates bundle, session or recordings directories.
//! Prints `ok <dir>` per directory, or the first failing path and exits 1.
use std::path::PathBuf;

fn main() {
    let mut opts = contracts::Options::default();
    let mut dirs = vec![];
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--repo" {
            opts.repo = args.next().map(PathBuf::from);
        } else {
            dirs.push(PathBuf::from(a));
        }
    }
    if dirs.is_empty() {
        eprintln!("usage: validate [--repo <clone>] <bundle dir>...");
        std::process::exit(2);
    }
    for d in dirs {
        match contracts::validate_dir(&d, &opts) {
            Ok(()) => println!("ok {}", d.display()),
            Err(f) => {
                println!("FAIL {}: {}", d.display(), f);
                std::process::exit(1);
            }
        }
    }
}
