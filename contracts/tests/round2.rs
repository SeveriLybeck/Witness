use contracts::{cite_resolves, validate_dir, Bundle, Options, Schemas};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

fn fx(n: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(n)
}
fn rec(n: &str, r: &str) -> Value {
    contracts::read_json(&fx(n).join(format!("{r}.json"))).unwrap()
}
fn copy_dir(from: &Path, to: &Path) {
    let _ = fs::remove_dir_all(to);
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        if e.path().is_file() {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}
fn tmp(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}
fn write(p: &Path, v: &Value) {
    fs::write(p, serde_json::to_string_pretty(v).unwrap()).unwrap();
}

#[test]
fn review_cites_resolve_like_the_page() {
    let b = Bundle::load(&fx("2283")).unwrap();
    for c in ["d1", "b3", "q2", "p5", "preset:d1", "reproduction", "declared",
              "GenericsFormatterTests.ClosedGenericStruct_FromMetadataReference", "TypeCollector.cs:985"] {
        assert!(cite_resolves(&b, c), "{c}");
    }
    for c in ["d9", "preset:d9", "hanging", "b0", "Nope.cs:1", "TypeCollector.cs:0", ""] {
        assert!(!cite_resolves(&b, c), "{c}");
    }
    assert!(cite_resolves(&Bundle::load(&fx("2234")).unwrap(), "hanging"));
    assert!(contracts::is_sha("32258fdbee4bcd653c79f51b4376b5103c6f4d27") && !contracts::is_sha("32258fdb"));
    assert_eq!(contracts::DIFFERENCE_KINDS.len(), 8);
}

#[test]
fn library_fact_needs_listing_on_a_checked_unit() {
    let d = tmp("library-fact");
    copy_dir(&fx("2283"), &d);
    let mut sc = rec("2283", "scope");
    sc["assumptions"].as_array_mut().unwrap().push(json!({"kind": "library-fact", "key": "FirstOrDefault",
        "text": "FirstOrDefault(a) = a.Length > 0 ? a[0] : null", "rows": ["r2"]}));
    write(&d.join("scope.json"), &sc);
    let e = validate_dir(&d, &Options::default()).unwrap_err();
    assert!(e.message.contains("lists no fact `FirstOrDefault`"), "{e}");
    let mut l = rec("2283", "lean");
    l["coverage"][0]["facts"] = json!([{"key": "FirstOrDefault", "text": "FirstOrDefault(a) = a.Length > 0 ? a[0] : null"}]);
    write(&d.join("lean.json"), &l);
    validate_dir(&d, &Options::default()).unwrap();
}

#[test]
fn optional_records_are_validated_when_present() {
    for (n, sub) in [("2283", "tests-extra"), ("2297", "tests-extra"), ("2129", "map-extra")] {
        assert!(fx(n).join(sub).is_dir(), "{n}/{sub}");
    }
    let d = tmp("bad-extra");
    copy_dir(&fx("2283"), &d);
    fs::create_dir_all(d.join("tests-extra")).unwrap();
    write(&d.join("tests-extra/reverted.json"), &json!({"reverted": [{"block": "x1"}]}));
    let e = validate_dir(&d, &Options::default()).unwrap_err();
    assert!(e.path.starts_with("tests-extra/reverted.json"), "{e}");
    let mut r = rec("2283", "replays");
    r["replays"][0]["input"]["source"] = json!("test");
    r["replays"][1] = json!({"difference": "d2", "input": null, "old": null, "new": null, "state": "not-replayed",
        "roundTrip": null, "reason": "no-match", "detail": "2 candidates"});
    Schemas::contract().unwrap().validate("replays", &r, "replays").unwrap();
}

#[test]
fn real_bundles_validate() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../replays");
    for n in ["2283", "2234", "2297", "2129"] {
        let d = root.join(n).join("bundle");
        if d.exists() {
            validate_dir(&d, &Options::default()).unwrap_or_else(|f| panic!("{n}: {f}"));
        }
    }
}
