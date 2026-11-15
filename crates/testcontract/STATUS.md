# testcontract — status

Works: the PR's tests from the test-file diff, old and new runs through the sandbox queue, categories, verbatim failure and assertion, block reverts and links, kept reverted builds, snapshot "output changed" entries, generated xUnit tests as patches (compiled in the sandbox).
Run: `nix-shell -p cargo rustc gcc pkg-config dotnet-sdk_8 bubblewrap git --run 'cargo test -p testcontract'` (8 unit + 7 integration tests; the 2283 run takes several minutes).
CLI: `testcontract --records <dir> --old <rev> --new <rev> --out <dir> [--root NAME]`, or module form `[--root NAME] <stageDir> <bundleDir> <repo> <old> <new>` (records = bundleDir, out = stageDir, repo ignored).
Sandbox root `work/sandbox/<NAME>`, default `testcontract-<first 12 of new commit>`. Module form also writes `snapshots.json`, `suggested-tests.json`, `patches/`, `reverted.json` to `<bundleDir>/tests-extra/`; `reverted.json` paths are absolute host paths.

Interface (`testcontract::`)
- `prepare(&Sandbox, &Queue, &Inputs {records, old, new}, work) -> Output {tests, links, snapshots, suggested, reverted, old_commit, new_commit}`; `write(&Output, dir)`.
- Reads `parts.json`, `blocks.json`; uses `differences.json`, `replays.json`, `groups.json`, `scope.json` when present. Never reads recordings (tests.json is the same with or without them).
- Writes `tests.json`, `links.json` (contract records, `provenance: pipeline`), `snapshots.json` `{snapshots: [{part, kind: output-changed, checker: file-comparison, file, location, before, after}]}`,
  `suggested-tests.json` `{suggested: [{difference, block, file, test, compiles, patch}]}`, `patches/<difference>.patch`, `reverted.json` `{reverted: [{block, compiles, path}]}`.
- Reverted builds: `<sandbox root>/<work>/rev/<new12>/<block>` (full working copy with `bin/`); old run: `<work>/mix/<old12>-<new12>`; generated-test build: `<work>/gen/<new12>`.
- Helpers: `diff_tests`, `categorize`, `snapshot_entries`, `csharp::test_methods`, `trx::parse`, `revert::{parse_hunks, revert_file}`, `gen::test_source`.

Rules taken
- Test files: parts with category `test`. A test is an `[Fact]`/`[Theory]` method; name is namespace.type.method. Edited = text differs; loosened = an old assertion statement is gone and there are no more assertions than before.
- Assertion `after`: the statement at the first stack frame inside the test file of the old-code failure; otherwise the first `Assert.` statement in the method or in same-file helpers it calls. `before`: the same for the old method.
- `failureMessage`: TRX `Message` of the old run (declared), the first compile error of the file (new-api), else null.
- Revert: `git diff -U0` hunks of the block's files, restricted to the parts' `addedLines`/`deletedLines`; then restore, build, re-run declared and new-api tests. Only test projects holding declared tests are built (on 2234 the ExecutionTests project never builds; building it made every block `not-linked`). Generated twins (`X.tt` → `X.cs`, category `generated`) revert with the block. `not-linked` = compiler errors (`: error CS`), first 5 kept in `reverted.json` `errors`; a revert build that could not run (worktree, restore, timeout, non-compiler failure) fails the module, so the builder marks section `tests` `not-run` with the detail. Placement: declared → `declared-region`; not-linked, or a difference or not-modelled line → `outside`; else `neither`.

Deviations and fallbacks
- A test that does not pass on new code (fail or infra error) is `not-run`: no category in D26 fits; it is never declared.
- C2: network on for the test step only; failures naming NuGet/HTTP are `not-run`. C3: filtered runs; non-compiling test files left out of the old run only. C4 fallback: one run per side.
- C5: the generated test is a fixed template (lib compiled to metadata, generator run, warning IDs asserted against the replay's `new` text); kept with its compile result, not run on old and new.
- Snapshots and suggested tests are not contract records (no schema has a place for a patch or a block-less difference); the bundle builder and gateway attach them.
- Concurrent cold restores into the shared NuGet folder fail: restores take `<nuget>/.testcontract-restore.lock` (flock, threads and processes) and retry twice. Clone and `worktree add` take `<root>/.sb/clone.lock` (`ensure_clone` in `crates/sandbox` has no lock of its own). Tests use roots `testcontract-2283`, `testcontract-2297`.
