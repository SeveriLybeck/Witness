use plugin_host::*;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}
fn example_manifest() -> Value {
    let p = root().join("contracts/messages/examples/plugin-manifest.generator-lab.json");
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}
fn write(p: &Path, s: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, s).unwrap();
}

// 10.1
#[test]
fn fallbacks_line_one_is_contract_hash() {
    let out = std::process::Command::new("git").args(["log", "-1", "--format=%H", "--", "contracts/"])
        .current_dir(root()).output().unwrap();
    let hash = String::from_utf8(out.stdout).unwrap().trim().to_string();
    let f = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("FALLBACKS.md")).unwrap();
    assert_eq!(f.lines().next().unwrap(), hash);
}

// 10.2
#[test]
fn example_manifest_loads() {
    let m = Manifest::parse(example_manifest(), &root().join("plugins/generator-lab")).unwrap();
    assert_eq!(m.name(), "generator-lab");
    assert_eq!(m.outputs().len(), 4);
}

#[test]
fn unknown_result_type_rejected_by_name() {
    let mut v = example_manifest();
    v["outputs"].as_array_mut().unwrap().push(json!({"name": "trend", "type": "chart"}));
    let e = Manifest::parse(v, Path::new(".")).unwrap_err();
    assert!(e.0.contains("trend") && e.0.contains("chart"), "{e}");
}

#[test]
fn sandbox_field_rejected() {
    let mut v = example_manifest();
    v["sandbox"] = json!("bwrap");
    assert!(Manifest::parse(v, Path::new(".")).is_err());
}

#[test]
fn detection_deterministic() {
    let gen = tmp("repo-gen");
    write(&gen.join("src/Gen/Gen.csproj"), "<PackageReference Include=\"Microsoft.CodeAnalysis.CSharp\" />");
    write(&gen.join("src/Gen/G.cs"), "class G : IIncrementalGenerator {}");
    let shop = tmp("repo-shop");
    write(&shop.join("package.json"), "{\"dependencies\": {\"express\": \"4\"}}");
    let m = Manifest::parse(example_manifest(), Path::new(".")).unwrap();
    assert!(m.detect(&gen) && m.detect(&gen));
    assert!(!m.detect(&shop));
    let mut v = example_manifest();
    v["detect"] = json!([{"rule": "package-reference", "path": "**/package.json", "pattern": "express"}]);
    assert!(Manifest::parse(v, Path::new(".")).unwrap().detect(&shop));
}

#[test]
fn globs() {
    let r = glob_regex("src/**/*.cs");
    assert!(r.is_match("src/a.cs") && r.is_match("src/x/y/a.cs") && !r.is_match("lib/a.cs") && !r.is_match("src/a.csx"));
}

// 10.3
fn echo_manifest(dir: &Path) -> Manifest {
    let exe = env!("CARGO_BIN_EXE_echo-plugin");
    let v = json!({"name": "echo", "version": "0", "executable": exe, "args": [],
        "detect": [{"rule": "file-exists", "path": "echo.txt"}],
        "inputs": [{"name": "n", "role": "n", "type": "int"}, {"name": "s", "role": "s", "type": "string"}],
        "outputs": [{"name": "echo", "type": "facts"}]});
    write(&dir.join("echo/plugin.json"), &v.to_string());
    Manifest::load(&dir.join("echo/plugin.json")).unwrap()
}

#[test]
fn echo_plugin_process_reused_no_network() {
    let d = tmp("echo");
    let m = echo_manifest(&d);
    let wd = d.join("work");
    let mut p = PluginProcess::start(&m, &wd).unwrap();
    let desc = p.describe(&"a".repeat(40), &"b".repeat(40)).unwrap();
    assert_eq!(desc["subjects"][0]["inputs"][0]["field"], "int");
    let r1 = p.run("echo", json!({"n": 1, "s": "x"}), &["old", "new"], Value::Null).unwrap();
    let r2 = p.run("echo", json!({"n": 2, "s": "y"}), &["old", "new"], Value::Null).unwrap();
    assert_eq!(r1["pid"], r2["pid"]);
    assert_eq!(r1["pid"], desc["pid"]);
    assert_eq!(r2["results"].as_array().unwrap().len(), 2);
    let facts = &r1["results"][0]["value"];
    assert!(facts["network"].as_str().unwrap().starts_with("failed"), "{facts}");
    assert_eq!(facts["loopback"], "open", "{facts}");
    assert_eq!(fs::canonicalize(facts["cwd"].as_str().unwrap()).unwrap(), fs::canonicalize(&wd).unwrap());
    p.shutdown().unwrap();
}

// 10.4
#[test]
fn viewer_ladder() {
    let d = tmp("ladder");
    let echo = echo_manifest(&d.join("plugins"));
    let gen = Manifest::parse(example_manifest(), Path::new(".")).unwrap();
    let plugins = vec![echo, gen];

    let one = tmp("ladder-one");
    write(&one.join("echo.txt"), "");
    let v = choose_viewers(&one, &plugins);
    assert_eq!(v.viewers.len(), 1);
    assert!(!v.tabs);

    let both = tmp("ladder-both");
    write(&both.join("echo.txt"), "");
    write(&both.join("src/G.csproj"), "Microsoft.CodeAnalysis.CSharp");
    write(&both.join("src/G.cs"), "IIncrementalGenerator");
    assert!(choose_viewers(&both, &plugins).tabs);

    let run = tmp("ladder-run");
    write(&run.join("Cargo.toml"), "[package]");
    let v = choose_viewers(&run, &plugins);
    assert_eq!(v.viewers[0].kind, ViewerKind::RunAndCompare);
    assert!(!v.tabs);

    let none = tmp("ladder-none");
    write(&none.join("lib.rs"), "pub fn f(n: i32) {}");
    let v = choose_viewers(&none, &[]);
    assert_eq!(v.viewers[0].kind, ViewerKind::SignatureCaller);
    for v in [&v.viewers[0]] {
        assert!(v.outputs.iter().all(|(_, t)| RESULT_TYPES.contains(&t.as_str())));
    }
}

#[test]
fn signature_form() {
    let s = signature_slots(&[("n", "int"), ("s", "string"), ("o", "MessagePackSerializerOptions")]);
    assert_eq!(s[0]["field"], "int");
    assert_eq!(s[1]["field"], "string");
    assert_eq!(s[2]["constructible"], false);
    assert!(s[2]["field"].is_null());
}

// Gateway's command line: `plugin-host --plugins <dir> <plugin> <bundle dir>`, caller ids kept.
#[test]
fn host_binary_relays_like_gateway() {
    use std::io::{BufRead, BufReader, Write};
    let d = tmp("bin");
    echo_manifest(&d.join("plugins"));
    let bundle = root().join("contracts/fixtures/2283");
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_plugin-host"))
        .arg("--plugins").arg(d.join("plugins")).arg("--work").arg(d.join("work"))
        .arg("echo").arg(&bundle)
        .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    let mut i = c.stdin.take().unwrap();
    let mut o = BufReader::new(c.stdout.take().unwrap());
    let mut ask = |v: Value| -> Value {
        writeln!(i, "{v}").unwrap();
        let mut s = String::new();
        o.read_line(&mut s).unwrap();
        serde_json::from_str(&s).unwrap()
    };
    let r = ask(json!({"id": 7, "op": "describe"}));
    assert_eq!(r["id"], 7);
    assert!(r["ok"].as_bool().unwrap(), "{r}");
    let r = ask(json!({"id": 3, "op": "run", "subject": "echo", "inputs": {"n": 1}, "sides": ["new"]}));
    assert_eq!(r["id"], 3);
    let net = r["results"][0]["value"]["network"].as_str().unwrap();
    assert!(net.starts_with("failed"), "{net}");
    let cwd = r["results"][0]["value"]["cwd"].as_str().unwrap();
    assert!(cwd.ends_with("echo/9df83c38409c8bfad0517aebffe48211eda4a5b4"), "{cwd}");
    let bd = r["results"][0]["value"]["bundleDir"].as_str().unwrap();
    assert_eq!(Path::new(bd), fs::canonicalize(&bundle).unwrap());
    assert_eq!(ask(json!({"id": 9, "op": "shutdown"}))["id"], 9);
    assert!(c.wait().unwrap().success());
}
