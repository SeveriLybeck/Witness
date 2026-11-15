# Blocked

Nothing is blocked at 13:10.

## Resolved
- **Group 25, Rust front end.** Charon 0.1.315 and Aeneas (rev 0d21a9e) are installed from their nix flakes by `crates/translator-rust/tools/setup.sh`, with Aeneas's Lean library and mathlib (Lean 4.36.0-rc2 under `work/elan`). On `main`: Charon's output is lowered to the model, the differ finds the planted difference, and one row reads "checked by Lean (Aeneas)". Needs about 12 GB free disk for the first setup.
- **Group 11, replays from the pull request's own tests.** The contract now allows replay source `test` (contracts round 2); test-derived candidates are on by default.
- **Group 7, Lean on contract inputs.** Lean checks the differ's real rows; the contract fixture was corrected in round 2.
