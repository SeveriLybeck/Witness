//! Fit-check fixes without the worker.
use generator_lab::{outcome, outcome_matches, split_replay, Config, RunInput};
use serde_json::json;
use std::path::PathBuf;

#[test]
fn bundle_dir_from_plugin_host_env_first() {
    let fx = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts/fixtures/2283").canonicalize().unwrap();
    std::env::set_var("PLUGIN_BUNDLE_DIR", &fx);
    let cfg = Config::load().unwrap();
    let d = cfg.bundle_dir("32258fdbee4bcd653c79f51b4376b5103c6f4d27", "9df83c38409c8bfad0517aebffe48211eda4a5b4").unwrap();
    assert_eq!(d, fx);
}

#[test]
fn errors_keep_the_crash_exception() {
    let side = json!({"diagnostics": [
        {"id": "MsgPack005", "severity": "error", "message": "x", "location": null},
        {"id": "CS8785", "severity": "warning", "message": "Exception was of type 'InvalidOperationException' with", "location": null}]});
    let o = outcome(&side);
    assert_eq!(o, "errors MsgPack005; warnings CS8785: InvalidOperationException");
    assert!(outcome_matches("reports MsgPack005, throws InvalidOperationException", &o));
    assert!(!outcome_matches("reports MsgPack005, throws ArgumentException", &o));
    // 2283 d1: the row names the ID only
    assert!(outcome_matches("reports MsgPack004", "errors MsgPack004; warnings CS8785, AD0001: IndexOutOfRangeException"));
    assert!(!outcome_matches("nothing reported", "warnings CS8785: IndexOutOfRangeException"));
    assert!(outcome_matches("nothing reported", "0 warnings; round trip preserved"));
}

#[test]
fn reproduction_by_bare_or_hash_id() {
    let cfg = Config::load().unwrap();
    assert!(cfg.reproduction("2283").is_some());
    assert_eq!(cfg.reproduction("2283"), cfg.reproduction("#2283"));
    assert!(cfg.reproduction("9999").is_none());
}

#[test]
fn build_properties_from_entry_and_text() {
    let i = RunInput::from_entry("// NoWarn=MsgPack005\nclass X {}\n", &json!({}));
    assert_eq!(i.build_properties["NoWarn"], "MsgPack005");
    let i = RunInput::from_entry("class X {}\n", &json!({"NoWarn": "MsgPack005"}));
    assert_eq!(i.build_properties["NoWarn"], "MsgPack005");
    assert_eq!(generator_lab::strip_properties("// NoWarn=MsgPack005\n// lib\nclass X {}\n"), "// lib\nclass X {}\n");
}

#[test]
fn host_paths_become_relative() {
    let sha = "32258fdbee4bcd653c79f51b4376b5103c6f4d27";
    let msg = format!("Exception was of type 'IndexOutOfRangeException'.\r\n   at TypeCollector.GetObjectInfo(INamedTypeSymbol formattedType) in /srv/any/root/.claude/worktrees/x/plugins/generator-lab/work/sb/wt/{sha}/src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs:line 1075\n");
    let v = generator_lab::scrub_value(&json!({"value": [{"id": "CS8785", "message": msg}]}), &[]);
    let m = v["value"][0]["message"].as_str().unwrap();
    assert!(m.contains(" in src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs:line 1075"), "{m}");
    assert!(!m.contains("/srv/"));
    assert_eq!(generator_lab::scrub("at /work/sandbox/tc/rev/9df83c38409c/b1/src/A.cs:12", &[]), "at src/A.cs:12");
    assert_eq!(generator_lab::scrub("error in /tmp/witness-run-abc/App.dll", &[]), "error in consumer/App.dll");
    assert_eq!(generator_lab::scrub("see /opt/tree/src/X.cs", &["/opt/tree".into()]), "see src/X.cs");
    assert_eq!(generator_lab::scrub("a // comment / b", &[]), "a // comment / b");
}

#[test]
fn round_trip_only_with_probe() {
    assert!(!RunInput::from_text("// lib\nnamespace Lib;\n// app\nnamespace App;\n").round_trip);
    assert!(RunInput::from_text("public static object Make() => 1;").round_trip);
}

#[test]
fn preset_build_properties_are_lines_and_unmarked_text_is_app() {
    let i = RunInput::from_text("// NoWarn=MsgPack005\nusing MessagePack;\n");
    assert_eq!(i.preset_inputs()["buildProperties"], "NoWarn=MsgPack005");
    assert_eq!(generator_lab::build_properties(&i.preset_inputs()["buildProperties"]), json!({"NoWarn": "MsgPack005"}));
    let (lib, app) = split_replay("using MessagePack;\n");
    assert!(lib.is_empty() && app.contains("MessagePack"));
}
