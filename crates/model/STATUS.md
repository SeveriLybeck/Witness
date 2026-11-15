# model — status

Short-check prompt (supersedes the review notes below where they differ): `review_prompt` asks for at most `MAX_STATEMENTS` (10) statements, `MAX_PER_HEADING` (2, 4, 2, 3), each `text` one line of at most `LINE_WORDS` (12) words, related evidence grouped in one statement, optional `detail`, and `summaries` (one line per heading); statements, details and summaries state only what the evidence shows and name nothing that is not in the prompt. Prompt and schema are frozen at this text. `review_schema` requires `summaries` and `statements` (`maxItems` 10, `text` `maxLength` 110). `check_review` keeps `detail`, writes `summaries` for headings that have a statement, and drops a statement or summary with a banned word. 17 tests. `tests/data/2283/bundle` is the current prepared 2283 bundle and `tests/data/recordings/ba0f0038…` is the real response to the frozen prompt (one live call): 6 statements for 7 differences, 0 dropped. The prompt change invalidates `integration/recordings/` reviews and the contract's fixture recording key; re-record with `integration/record.sh 2283 2234 2297 2129`.

Works: recorded model calls (replay and live), the AI review prompt from bundle facts, the citation and banned-word check, proposed replay snippet acceptance. 15 tests, no network, no API key.
Run: `nix-shell -p cargo rustc gcc pkg-config --run 'cargo test -p model'`.

Binary: `model-review [--recordings DIR] [--live CLI] <stageDir> <bundleDir> <repo> <old> <new>` writes `<stageDir>/ai-review.json`; recordings default `$WITNESS_DATA/recordings`, else `work/recordings`; replay only unless `--live`.
The contract's fixture recording is keyed on `review_prompt` of `fixtures/2283` and replays by key. Prompt and schema bytes are pinned by a test against the committed `integration/recordings/` keys.

Interface (`model::`)
- `Recorder::replay(dir)` / `Recorder::live(dir, cli)`; `call(purpose, prompt, schema) -> Result<(key, response), ModelError>`. Key = `contracts::recording_key`; file `<dir>/<key>.json` per `recording.schema.json`. Replay never spawns; missing file is `ModelError::NoRecording(key)`. Live runs `<cli> -p --output-format json --json-schema <canonical schema>` with the prompt on stdin, reads `structured_output` (else `result` as JSON text), writes the recording.
- `Bundle::load(dir)`; `review_prompt(&Bundle)`; `review_schema()`; `check_review(&response, &Bundle, Option<key>) -> ai-review.json value`; `review(bundle_dir, &Recorder)` returns `no_review()` (`recording: null`, no statements) when there is no recording.
- Citable: whatever `contracts::cite_resolves` accepts (ids of differences, blocks, questions, parts; test names; `preset:d<n>`; `reproduction`; a test category in `tests.json`; `File.cs:<line>` of a changed file). `Bundle::contract_bundle()` gives the contracts-crate bundle.
- `check_review` drops statements with no cite, any cite `cite_resolves` rejects, an unknown heading, or a banned word (proved, safe, verified, trusted, correct; case-insensitive substring), sorts by the four headings and appends a `left-to-you` statement (`<rule> <block>`) per question the model did not cite there.
- Snippets: `snippet_prompt`, `snippet_schema`, `propose_snippet(&Bundle, &difference, &Recorder)` (purpose `replay-snippet`); `snippet_matches(&difference, real_old, real_new)`; `accept_snippet(...)` gives a `replays.json` entry, `seen-live` with source `model` or `not-replayed` with `input: null`.
- Outcome match: `classify` gives `Outcome {throws: Option<exception name>, ids: diagnostic IDs like MsgPack004, CS8785}`. Match when the row's throw (if any) equals the real one and the row's IDs are all among the real IDs; a row with neither ("nothing reported") needs a real outcome with neither. An IDs-only row matches a real outcome that also crashed.

Deviations and fallbacks
- No playbook fallback taken (`FALLBACKS.md`, line 1 is the contract hash).
- The real `claude` CLI is never run in tests; a fake shell script stands in for the live path. The envelope field `structured_output` is assumed from the CLI's JSON output; `result` is the fallback.
- Running the snippet for real is group 11's job; this crate only takes its old and new outcome strings.
- Real 2283 review (`integration/recordings/a601ab…` on `replays/2283/bundle`, copies in `tests/data/`) replays by key: 17 kept, 0 dropped; the record passes the contract.
- Purpose names `ai-review` and `replay-snippet` are this crate's choice; the fixture recording uses `ai-review`.
