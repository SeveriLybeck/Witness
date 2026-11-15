# rules — status

Works: importance levels (D33), review mode (D47), decide list (D40). 2283 levels, mode and questions equal the fixture; 2234 mode (full: level 3, hanging, not run) and questions equal the fixture; level numbers, mode and questions equal the fixture on all four; a bundle with no level 3 gives light and an empty list; outputs validate against the contract schemas.
Run: `nix-shell -p cargo rustc gcc pkg-config --run 'cargo test -p rules'` (14 tests).
Module: `rules <stageDir> <bundleDir> <repo> <old> <new>` reads each record from `<bundleDir>`, else `<stageDir>`; writes `levels.json`, `mode.json`, `questions.json` (provenance `pipeline`) to `<stageDir>`, created if missing; repo/old/new ignored. Short form: `rules <bundle-dir> [<out-dir>]`. Both take `--config rules.json`.

Interface
- Reads `blocks parts differences links tests scope dependencies` and optional `facts` (`Inputs::read_dir`). Thresholds and kind lists: `crates/rules/rules.json` only (`Config::default()` embeds it).
- `rules::{run, levels, mode, questions, fired, level_of, RULE_TABLE, Config, Inputs}`; `run` returns the three records as JSON.
- Level rules, table order (all firing rules are recorded; level = max, floor 1): `undeclared-difference` 3, `severe-difference` 3, `architectural` 3, `not-modelled-line` 3, `difference` 2, `reach` 2, `more-differences-than-tests` 2, `large-comment` 2.
- Undeclared: block has a difference and its link is not `declared` (or no link). Severe kinds: crash, silent-failure, data-loss. `difference` fires only for a difference that is neither undeclared nor severe.
- Mode rules: `level-3-block`, `hanging-tests`, `withdrawn-tests`, `not-run-tests`; none means light.
- Questions: per difference (undeclared, else declared severe; tests = the block's linked tests), then `level-3-block` for each level-3 block without a question, then one `test-group` per category hanging, withdrawn, not-run. Ids q1… in that order. Answers (accept/return) are the page's and gateway's, not stored here.
- Section states (running, unavailable) are the bundle builder's: run rules only when tests and links are ready.

Deviations and fallbacks
- `architectural` fires from worker facts (not a contract record): `facts.json` or `map-extra/facts.json` in `<bundleDir>` (else `<stageDir>`), reply `{result: {facts}}` or `{facts}`. A fact of a kind in `factKinds` fires on every block with a production part whose `new` range overlaps the fact's location; each kind is written after it in `levels.json` as `{"rule": "architectural", "fact": "<kind>"}` (in Rust, `BlockLevel.rules` holds `architectural:<kind>`; level_of reads the part before `:`). Also fires on a public-api/new-dependency difference or a new file part. Without facts, results are as before.
- `reach` = sites of edges `from` the block (edge A→B: B uses A) + `reach[].callSites` when present; threshold `reachSites` 5 (no number in design). 2283 b1 with 28 call sites fires.
- Fixtures 2234, 2297, 2129 are hand-written and differ in rule names only: 2234 b2 lists `more-differences-than-tests` (1 difference, 2 linked tests: rule does not fire here); 2129 lists `architectural` on b2–b6 and omits `large-comment` where parts set `largeComment: true`. Tests compare level numbers there, full records on 2283.
- Playbook §14 checks: contract hash on line 1 of `FALLBACKS.md` (tested); no ports, no heavy builds, no toolchain beyond cargo.
