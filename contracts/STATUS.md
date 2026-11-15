# contracts — status

Works: 24 record schemas, 8 message schemas, four fixture bundles, a 2283 session, review and recording, the validator, the sandbox.
Run: `nix-shell -p cargo rustc gcc pkg-config --run 'cargo test -p contracts'` (18 tests, also validates `replays/*/bundle`);
`cargo run -p contracts --bin validate -- [--repo <clone>] <dir>...` prints `ok <dir>` or `FAIL <dir>: <path>: <message>` (exit 1).
`source contracts/toolchain.env` prints the pinned versions; `contracts/spike/tools/sb [--root DIR] net|nonet [--src] cmd` is the bwrap sandbox.

Interface
- Records: `schemas/<record>.schema.json`, file `<record>.json`; list records are `{provenance, <record>: [...]}`. Shared defs in `common.schema.json`. Section state `{state, detail?}`.
- Optional records, validated when present: `map-extra/facts.json`, `tests-extra/{snapshots,suggested-tests,reverted}.json`.
- Messages: `messages/*.schema.json`, endpoints in `messages/gateway-api.md`, examples `messages/examples/`; 2283 module inputs in `fixtures/2283/inputs/`.
- Rust: `validate_dir`, `validate_bundle`, `Schemas`, `Bundle::{load, from_records}`, `cite_resolves`, `recording_key`, `canonical_json`, `is_sha`, `DIFFERENCE_KINDS`, `SOLVER_KINDS`, `TEST_CATEGORIES`, `EXTRA_RECORDS`.

Changed in round 2
- Replays: `input.source` may be `test`; optional `input.buildProperties` (NoWarn moved out of the 2234 input text); optional `reason` (`no-candidate|no-match|no-recording|not-replayable`) and `detail`.
- AI-review cites also resolve `preset:d<n>`, `reproduction`, a test category present in `tests.json`, and `File.cs:<line>` of a changed file (as the page does), via `cite_resolves`.
- Gateway: `GET /api/bundles/<key>/viewers` (`viewers` def) and `GET /api/bundles/<key>/pages/<plugin>/<path>`.
- Optional fields: `parts[].largeCommentLines`, `dependencies.reach`, `files[].history`, `lean.coverage[].facts [{key, text, lean?}]`; `levels[].rules[]` may be `{rule, fact}`; worker `fact` gains `endLine`, `container`, `diagnostic`.
- Scope assumptions: either a contract `{contract, symbol, rows}` or `{kind: library-fact, key, text, rows}`. Lean states unchanged: a `checked` row resting on a library fact requires its unit's `lean.coverage[].facts` to list that key (validator rule), so `checked` always comes with its hypotheses.
- Fixtures: requirement ids bare numbers; 2283 b3 `GetObjectInfo local`; 2283 dependencies from the real worker (`both`, site 1080, reach 28/0/0); levels regenerated with the rules crate; 2283 solver rows limited to the :985 models and complete; the recording re-keyed on `model::review_prompt` of `fixtures/2283`; new optional-record fixtures; `contracts/LOCKS/.keep`.
- Removed `src/bin/cut_fixture.rs` and the cut specs (the cutter owns cutting).

Changed in round 3
- Optional `ai-review.summaries {<heading>: line}` and `statements[].detail`; `tests[].failureSummary`; `differences[].group`.
- Fixture recording re-keyed on the current `model::review_prompt`/`review_schema` with a conforming response; `fixtures/2283/ai-review.json` is `model::check_review` of it.

Deviations and fallbacks
- `crates/.gitkeep` outside `contracts/` (cargo needs `crates/` for the member glob); `!contracts/src/bin/` in the root `.gitignore`.
- Difference `condition` texts stay distinct from first-row texts: the spec's worked example (evidence-bundle, 2283 d3) requires it.
- Fixture production parts of 2297/2129 are one per hunk; 2283 `b2` `not-declared` with the g6 replay comes from the spike.
