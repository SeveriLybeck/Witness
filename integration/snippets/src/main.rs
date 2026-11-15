//! `witness-snippets --recordings DIR [--live CLI] [--check] [--not-replayed replays.json] <bundle dir> <out.json>`
//! For every difference of the bundle, asks the model crate for one proposed replay snippet (D23, purpose
//! `replay-snippet`) and writes `[{difference, text}]` for `generator-lab replay --proposed`.
//! Replay mode (default) never spawns a process: a missing recording means no snippet for that difference.
//! `--check` prints each prompt's size and refuses (exit 2) when a prompt holds a host path or a secret-like word.

use model::{snippet_prompt, propose_snippet, Bundle, ModelError, Recorder};
use serde_json::{json, Value};
use std::path::PathBuf;

fn suspicious(p: &str) -> Option<&'static str> {
    for w in ["/home/", "/nix/store", "/tmp/", "token", "TOKEN", "password", "secret", "ANTHROPIC", "Bearer"] {
        if p.contains(w) {
            return Some(w);
        }
    }
    None
}

fn main() {
    let mut a: Vec<String> = std::env::args().skip(1).collect();
    let (mut rec_dir, mut live, mut check, mut skip, mut review) = (None, None, false, None::<PathBuf>, false);
    while a.first().is_some_and(|x| x.starts_with("--")) {
        match a.remove(0).as_str() {
            "--recordings" => rec_dir = Some(PathBuf::from(a.remove(0))),
            "--live" => live = Some(PathBuf::from(a.remove(0))),
            "--check" => check = true,
            "--review-prompt" => review = true,
            "--not-replayed" => skip = Some(PathBuf::from(a.remove(0))),
            x => { eprintln!("unknown option {x}"); std::process::exit(2) }
        }
    }
    let [bundle, out] = <[String; 2]>::try_from(a).unwrap_or_else(|_| { eprintln!("usage: witness-snippets --recordings DIR [--live CLI] [--check] <bundle dir> <out.json>"); std::process::exit(2) });
    let dir = rec_dir.unwrap_or_else(|| { eprintln!("--recordings missing"); std::process::exit(2) });
    let rec = match &live { Some(cli) => Recorder::live(&dir, cli), None => Recorder::replay(&dir) };
    let b = Bundle::load(std::path::Path::new(&bundle));
    if review {
        // the AI review prompt model-review will send: size and the same refusal check, nothing sent
        let p = model::review_prompt(&b);
        eprintln!("witness-snippets: review prompt {} bytes", p.len());
        if let Some(w) = suspicious(&p) { eprintln!("witness-snippets: review prompt contains {w:?}; refusing"); std::process::exit(2); }
        let _ = std::fs::write(&out, &p);
        return;
    }
    let diffs: Vec<Value> = b.differences.get("differences").and_then(Value::as_array).cloned().unwrap_or_default();
    // only differences a first replay left not-replayed (`--not-replayed replays.json`)
    let keep: Option<Vec<String>> = skip.map(|p| {
        let r: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap_or_default()).unwrap_or(Value::Null);
        r["replays"].as_array().into_iter().flatten().filter(|x| x["state"] == "not-replayed").filter_map(|x| x["difference"].as_str().map(String::from)).collect()
    });
    let diffs: Vec<Value> = diffs.into_iter().filter(|d| keep.as_ref().is_none_or(|k| k.iter().any(|x| d["id"] == x.as_str()))).collect();
    let mut outv = vec![];
    for d in &diffs {
        let id = d["id"].as_str().unwrap_or("?");
        let prompt = snippet_prompt(&b, d);
        if check || live.is_some() {
            eprintln!("witness-snippets: {id} prompt {} bytes", prompt.len());
            if let Some(w) = suspicious(&prompt) {
                eprintln!("witness-snippets: {id} prompt contains {w:?}; refusing");
                std::process::exit(2);
            }
        }
        if check {
            continue;
        }
        match propose_snippet(&b, d, &rec) {
            Ok(t) => outv.push(json!({"difference": id, "text": t})),
            Err(ModelError::NoRecording(_)) => {}
            Err(e) => eprintln!("witness-snippets: {id}: {e:?}"),
        }
    }
    std::fs::write(&out, serde_json::to_string_pretty(&Value::Array(outv)).unwrap() + "\n").expect("write out");
}
