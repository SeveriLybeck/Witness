//! Charon, Aeneas and Aeneas's Lean library, as installed by `tools/setup.sh`
//! under `<repo>/work/` (override with CHARON, AENEAS, AENEAS_LEAN).

use std::path::{Path, PathBuf};
use std::process::Command;

pub const LEAN_TOOLCHAIN: &str = "leanprover--lean4---v4.36.0-rc2";

fn work() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../work")
}

fn pick(var: &str, default: PathBuf) -> Option<PathBuf> {
    let p = std::env::var_os(var).map(PathBuf::from).unwrap_or(default);
    p.exists().then_some(p)
}

pub fn charon() -> Option<PathBuf> {
    pick("CHARON", work().join("charon/bin/charon"))
}

pub fn aeneas() -> Option<PathBuf> {
    pick("AENEAS", work().join("aeneas/bin/aeneas"))
}

/// The Aeneas Lean project (built `Aeneas` library) and the `lake` of its toolchain.
pub fn aeneas_lean() -> Option<(PathBuf, PathBuf)> {
    let dir = pick("AENEAS_LEAN", work().join("aeneas-lean"))?;
    let home = std::env::var_os("ELAN_HOME").map(PathBuf::from).unwrap_or(work().join("elan"));
    let lake = home.join("toolchains").join(LEAN_TOOLCHAIN).join("bin/lake");
    (lake.exists() && dir.join(".lake/build/lib/lean/Aeneas.olean").exists()).then_some((dir, lake))
}

fn run(c: &mut Command) -> Result<String, String> {
    let o = c.output().map_err(|e| format!("{c:?}: {e}"))?;
    let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
    if o.status.success() {
        Ok(text)
    } else {
        Err(format!("{c:?} failed: {text}"))
    }
}

pub fn version(tool: &Path, arg: &str) -> Result<String, String> {
    Ok(run(Command::new(tool).arg(arg))?.trim().to_string())
}

/// `charon rustc --preset=aeneas` on one source file; `dedup` false gives the readable form.
pub fn charon_llbc(src: &Path, crate_name: &str, out: &Path, dedup: bool) -> Result<(), String> {
    let charon = charon().ok_or("charon not installed (run crates/translator-rust/tools/setup.sh)")?;
    let mut c = Command::new(charon);
    c.args(["rustc", "--preset=aeneas"]);
    if !dedup {
        c.arg("--no-dedup-serialized-ast");
    }
    c.arg("--dest-file").arg(out).args(["--", "--crate-type=lib", "--crate-name", crate_name]).arg(src);
    run(&mut c).map(|_| ())
}

/// `aeneas -backend lean` on an LLBC file; returns the generated Lean text.
pub fn aeneas_to_lean(llbc: &Path, dir: &Path) -> Result<String, String> {
    let aeneas = aeneas().ok_or("aeneas not installed (run crates/translator-rust/tools/setup.sh)")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    run(Command::new(aeneas).args(["-backend", "lean", "-use-lean-modules", "false", "-dest"]).arg(dir).arg(llbc))?;
    let f = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "lean"))
        .ok_or("aeneas wrote no .lean file")?;
    std::fs::read_to_string(f).map_err(|e| e.to_string())
}

/// `lake env lean <file>` in the Aeneas Lean project; returns (success, output).
pub fn check_lean(file: &Path) -> Result<(bool, String), String> {
    let (dir, lake) = aeneas_lean().ok_or("Aeneas Lean library not built (run crates/translator-rust/tools/setup.sh)")?;
    let o = Command::new(lake).args(["env", "lean"]).arg(file).current_dir(dir).output().map_err(|e| e.to_string())?;
    Ok((o.status.success(), String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr)))
}
