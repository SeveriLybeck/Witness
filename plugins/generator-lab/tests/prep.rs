mod common;

#[test]
fn prepare_builds() {
    let p = common::prep();
    assert!(p.root.join("worker/src/Witness.Worker/bin/Release/net8.0/witness-worker.dll").exists());
}
