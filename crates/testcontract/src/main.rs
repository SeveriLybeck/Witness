//! `testcontract --records <dir> --old <rev> --new <rev> --out <dir> [--root NAME]`
//! or, as a bundle module, `testcontract [--root NAME] <stageDir> <bundleDir> <repo> <old> <new>`.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let args = match testcontract::parse_args(&a) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let run = || -> Result<(), String> {
        let root = match &args.root {
            Some(r) => r.clone(),
            None if args.new.len() == 40 && args.new.chars().all(|c| c.is_ascii_hexdigit()) => testcontract::default_root(&args.new),
            None => {
                let probe = sandbox::Sandbox::named("testcontract-resolve")?;
                probe.ensure_clone()?;
                testcontract::default_root(&probe.resolve(&args.new)?)
            }
        };
        let sb = sandbox::Sandbox::named(&root)?;
        let q = sandbox::Queue::shared();
        let inputs = testcontract::Inputs { records: args.records.clone(), old: args.old.clone(), new: args.new.clone() };
        let o = testcontract::prepare(&sb, &q, &inputs, "tc")?;
        testcontract::write(&o, &args.out)?;
        if let Some(x) = &args.extra {
            testcontract::write_extra(&o, x)?;
        }
        Ok(())
    };
    if let Err(e) = run() {
        eprintln!("testcontract: {e}");
        std::process::exit(1);
    }
    println!("ok {}", args.out.display());
}
