use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let r = match a.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["lower", file, func, version] => translator_ts::lower(Path::new(file), func, version, None),
        ["lower", file, func, version, unit] => translator_ts::lower(Path::new(file), func, version, Some(unit)),
        ["render", file] => translator_ts::render_causes(Path::new(file)),
        _ => {
            eprintln!("usage: translator-ts lower <file> <function> <old|new> [unit] | render <file>");
            std::process::exit(2);
        }
    };
    match r {
        Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
