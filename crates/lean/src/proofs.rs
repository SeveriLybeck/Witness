//! Model-written proofs (D25): a proof source proposes a proof for a theorem the fixed
//! script did not close; Lean judges it under the same checks, for at most
//! `MAX_ATTEMPTS` attempts. Recordings only: a missing recording means no call (D43).

use serde_json::{json, Value};
use std::path::PathBuf;

pub const MAX_ATTEMPTS: u32 = 3;
pub const PURPOSE: &str = "lean-proof";
pub const FORBIDDEN: &[&str] = &["sorry", "admit", "axiom", "native_decide", "implemented_by", "set_option", "unsafe", "opaque", "extern"];

pub trait ProofSource {
    /// A proof term or tactic block (text after `:=`), or None when there is nothing to try.
    fn propose(&self, statement: &str, last_error: &str, attempt: u32) -> Option<String>;
}

pub fn schema() -> Value {
    json!({"type": "object", "required": ["proof"], "properties": {"proof": {"type": "string"}}})
}

pub fn prompt(statement: &str, last_error: &str, attempt: u32) -> String {
    format!(
        "Write a Lean 4 (core only, 4.29.1) proof for the theorem below. Reply with the text after `:=` only.\nAttempt {attempt}.\nTheorem:\n{statement}\nLast Lean error:\n{last_error}\n"
    )
}

/// Reads `<dir>/<recording_key>.json` and returns `response.proof`.
pub struct Recorded {
    pub dir: PathBuf,
}

impl ProofSource for Recorded {
    fn propose(&self, statement: &str, last_error: &str, attempt: u32) -> Option<String> {
        let key = contracts::recording_key(PURPOSE, &prompt(statement, last_error, attempt), &schema());
        let text = std::fs::read_to_string(self.dir.join(format!("{key}.json"))).ok()?;
        let v: Value = serde_json::from_str(&text).ok()?;
        v.pointer("/response/proof").and_then(Value::as_str).map(str::to_string)
    }
}

pub fn forbidden(proof: &str) -> Option<&'static str> {
    FORBIDDEN.iter().copied().find(|w| proof.contains(w))
}
