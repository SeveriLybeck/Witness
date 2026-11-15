# cutter: status

Works: parts (tree-sitter for production C#, one per hunk elsewhere, one per file for test/docs), file categories, large comments, blocks joined by worker references, rule names, mechanical groups. 10 tests pass on the four sample branches (read with git only).
Run: `nix-shell -p cargo rustc gcc pkg-config git --run 'cargo test -p cutter'`; `cargo run -p cutter -- --repo <clone> --old <rev> --new <rev> [--references worker-reply.references.json] [--comment-run 4] [--comment-ratio 3] --out <dir>` writes `files.json parts.json blocks.json groups.json` (provenance `pipeline`).
Builder form: `cutter [--references <reply>] [--worker "<cmd>"] <stageDir> <bundleDir> <repo> <old> <new>` writes into `<stageDir>`. References: `--references`, else `$WITNESS_REFERENCES`, else `--worker`/`$WITNESS_WORKER`, else none.
Worker flow (`sh -c <cmd>`, one long-lived process, one line each way): parts → `references` (new checkout) → blocks, groups → `dependencies` (old and new checkouts, production blocks with part locations) → `dependencies.json`. Checkouts: `--wt-old/--wt-new` or `$WITNESS_WT_OLD/$WITNESS_WT_NEW`, else detached `git worktree`s under the temp folder, removed afterwards. Exit 1, nothing written, when the change has a changed member and statement parts but the worker returns no references for any of them (wrong checkout). Without a worker no `dependencies.json` is written.
`dependencies.json` extra top-level field `reach`: the worker's array as is, `[{"block": "b1", "callSites": 28, "sites": [loc...]}]` (`sites` when the worker sends it). Tests read the sample at `$WITNESS_SAMPLE_REPO` (default the challenge clone) and skip when it is absent.

Interface
- `cutter::cut_repo(repo, old, new, &refs, &Options) -> Result<Cut, String>`; `cut_repo_with_worker(repo, old, new, cmd, (old_wt, new_wt), &Options)` (fills `Cut::dependencies`); `references_request`, `dependencies_request`, `Checkouts`; `Cut::{files_json, parts_json, blocks_json, groups_json, write}`.
- Two steps for the worker: `cut_parts(&diff, &read, &Options)` gives part ids and locations to send as a `references` request; `join_blocks(&mut parts, &refs)` then `mechanical_groups(&parts)`. `references_from_reply(&Value)` reads a worker reply.
- Part ids: production, test, docs, snapshot, generated; files in diff order; parts by line. 2283 ids equal the fixture's p1..p5, so the fixture reply applies.
- Extra fields: parts carry `largeCommentLines` when `largeComment` is true; groups carry `variants` (counts of distinct untrimmed edits, 2297: [311, 91]).
- Blocks: one per member name per file; statement runs joining a member when exactly one of their references is a changed member; other statement runs keyed by their trimmed edit text; one per test or docs file. Snapshot and generated parts are in no block (as in the fixtures).
- Names: member name; else `<enclosing member> <statement kind>` (e.g. `GetObjectInfo if`); else `File.ext:line`; test/docs: file name.

Deviations and fallbacks
- B3 A: blank and brace-only lines follow the previous changed line of their hunk, else the next; comments follow their next sibling statement or member when that has changed lines.
- B2 A: a hunk with an error node in its syntax is one part. B5 B: no rename or move detection (`--no-renames`); rules `moved` and `rename` never fire.
- Nested changes fold into a changed member, or into a changed statement of at most 15 lines (spike rule).
- Identical edits fold only in snapshot and generated files. Folding in production blocks without differences (D31) needs the differ's output, so the bundle builder must do it; the cutter never folds production.
- A snapshot/generated hunk holding a repeated pure insert plus other lines is split: the repeat joins the group, the rest stays real (2297 GenericDataTypeWithPrivateMember: 404 snapshot parts, 402 grouped).
- `formatting` (whitespace-only edit) fires only on production parts; snapshot whitespace is output.
- Large comment is computed on production `.cs`/`.tt` parts only; thresholds are CLI options until the rules config (group 9) exists.
- Overloads with one name in one file share a block.
