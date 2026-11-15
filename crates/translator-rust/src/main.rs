//! translator-rust lower --src FILE --function NAME --version old|new [--file PATH] [--work DIR]
//! Uses Charon when installed (tools/setup.sh), else the syn fallback; stderr names which.
use translator_rust::{lower_file, Options};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    if a.first().map(String::as_str) != Some("lower") {
        eprintln!("usage: translator-rust lower --src FILE --function NAME --version old|new [--file PATH] [--work DIR]");
        std::process::exit(2);
    }
    let src = get("--src").expect("--src");
    let o = Options {
        function: get("--function").expect("--function"),
        version: get("--version").unwrap_or_else(|| "new".into()),
        file: get("--file").unwrap_or_else(|| src.clone()),
    };
    let work = std::path::PathBuf::from(get("--work").unwrap_or_else(|| "work/translator-rust".into()));
    match lower_file(std::path::Path::new(&src), &o, &work) {
        Ok((m, p)) => {
            eprintln!("translator-rust: model from {}", p.label());
            println!("{}", serde_json::to_string_pretty(&m).unwrap());
        }
        Err(e) => {
            eprintln!("translator-rust: {e}");
            std::process::exit(1);
        }
    }
}
