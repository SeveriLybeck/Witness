# bundle — status

Works: `witness prepare` runs modules as programs with time budgets, validates and writes records atomically, groups rows into differences (real rows: 2283 gives 7, 2234 gives 4), applies replays (d1 becomes `data-loss`), computes the boundary (b1 `declared-region`), keeps section states.
Run: `nix-shell -p cargo rustc gcc pkg-config --run 'cargo test -p bundle'` (12 tests; stub shell modules replay the fixtures).
CLI: `witness prepare [--data D] [--modules F] [--issues D] [--description T] [--pr N] [--wait] <repo> <old> <new>`; prints the bundle dir, exit 0 when prepared, 2 when not. Without `--wait` it starts `witness finish ... <repo> <oldSha> <newSha>` in the background. Env: `WITNESS_DATA` (default `./data`), `WITNESS_MODULES`, `WITNESS_ISSUES`.

Interface
- Modules file: `{"modules": [{"name", "section": map|proof|playground|review|tests|rules, "command": [..], "budgetSeconds"?}]}`.
- A module runs as `command... <stageDir> <bundleDir> <repo> <old> <new>` (also `WITNESS_STAGE/BUNDLE/REPO`), writes `<record>.json` of its section into `<stageDir>`, may read earlier records from `<bundleDir>`. Non-zero exit, overrun, invalid record → section `not-run` with `detail` naming module and field; no module → `unavailable`.
- Order: map (files, parts, blocks required), proof (rows, scope required; lean optional; non-solver `differences` optional), playground (replays) → prepared. `finish`: tests (tests, links) → rules (levels, questions, mode) → review (ai-review).
- Proof: the differ writes schema-valid `rows.json`; IDs and `difference` are reassigned. Grouping key (spec: same kind, same old and new outcome): block of `firstChanged`, kind, old tag+text, new tag+text, across units; the differ's `unit`/`difference` are ignored. Outcome text already phrased (`throws`/`reports`/`calls`/`nothing reported`) is kept as is. Kind from the D21 table unless a row carries a valid `kind`. Row IDs in `scope.assumptions` and `lean.rows` are remapped.
- Playground: missing replays become `not-replayed` with null input; a seen-live `roundTrip` with sent != received sets `kind` to `data-loss`.
- Links: placement is recomputed; each production block needs exactly one entry.
- Facts (optional, not a record): `facts.json` in the map or proof stage dir, the worker's `facts` reply as-is (`{id, method, result: {facts}}`) or `{facts}`. Kept unchanged as `<bundleDir>/facts.json` for rules. `public-type`, `public-member` (unless inside a new type by span or `container`/`containingType`/symbol owner, or marked `diagnostic`), `new-diagnostic` (one each, condition `new diagnostic <id>`; spec gives no kind, so `public-api`), `changed-signature` → `public-api` difference, checker `api-comparison`, block of its location; null locations skipped.
- Snapshots (optional): `snapshots.json` in the tests stage dir or `<bundleDir>/tests-extra/snapshots.json` (testcontract format). Each entry with non-null, differing before/after → `output-changed`, checker `file-comparison`, appended after existing IDs in `finish` (before links placement and rules), with a `not-replayed` replay. Block: entry `block` if given, else the first production block with a `.cs` part; extra field `blockRule`: `test-contract` or `first-cs-block`.
- Rust: `bundle::{prepare, Builder::{start, section, finish, prepared, validate}, Config, Module, load_modules, group_rows, apply_replays, place_links, solver_kind, write_atomic}`.

Deviations and fallbacks
- The AI review runs after the test contract (it cites tests); before that `review` is `running`. A review citing missing IDs is removed and `review` is `not-run`.
- If tests are not `ready`, `rules` stays `running` (detail `tests not ready`), as the contract demands.
- `requirement.id` is the bare issue number (`2234`, per the spec); the 2283 fixture uses `#2283`.
- Difference `condition` is the first row's condition. The 2283 fixture groups r2/r3 (different exception texts) into one difference; the spec rule gives 4 there (test pins 4).
- Snapshot differences are numbered after the existing ones, not by block, so earlier IDs stay. The block rule without `block` is a heuristic (2297: TransformUtilities block); testcontract could set `block` from reverts.
- Description defaults to `git log -1 --format=%B <new>` (hooks off); no pull-request fetch yet (`--description`, `--pr`).
- No fallback from the playbook taken (`FALLBACKS.md`).
