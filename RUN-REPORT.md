# Run report

Change `ai-pr-review-interface`, 2026-11-14. Unattended build 05:05–11:20, then a supervised extension until about 18:30. Nothing was pushed. Final state: tag `fit-8` on `main`.

## State

- Every task in `tasks.md` is ticked; `BLOCKED.md` has nothing open.
- Last full `integration/verify.sh`, run by the orchestrator on `main` at 18:07: exit 0 — the verify of groups 0–18 passes and all four sample changes print `WORKS`.
- Groups 20–25 last passed their own verify at 12:30–13:40 and were not re-run after the final page rounds (they do not touch the page). Group 25's Aeneas test needs `work/aeneas-lean` and `work/elan`, which are now only in the archive on the NAS.

## The four sample changes (real bundles, final pipeline)

| Change | Status on the page | Differences | Lines compared | Second checker (Lean) | Replays seen live | AI summary statements |
|---|---|---|---|---|---|---|
| 2283 | Needs human review | 7: 4 crash fixed, 2 silent failure, 1 data loss (42 → 0) | 28 of 29 | 26 of 26 rows | 4 of 7 | 6 |
| 2234 | Possible problems | 12: 8 crash fixed, 4 output changed | 22 of 22 | 39 of 39 rows | 3 of 12 | 4 |
| 2297 | Needs human review | 1 output changed (402 identical edits folded) | 0 of 29 | none | 0 of 1 | 5 |
| 2129 | Needs human review | 11: 7 public API, 1 new exception, 3 output changed | 83 of 450 | 4 of 4 rows | 0 of 11 | 9 |

Each bundle is prepared from real runs of the two commits; model calls are replayed from `integration/recordings/` (recorded once with `integration/record.sh`). The browser click-through passes on each: first screen within its length budget, no horizontal scroll at 1280 and 390 px, `42 → 0` visible on 2283, a playground run through the real viewer list, citations landing on their targets, no host path shown, sign-off recorded by the local GitHub stand-in.

## Start the page

```
nix-shell -p cargo rustc gcc pkg-config z3 lean4 dotnet-sdk_8 nodejs bubblewrap git playwright-driver.browsers
integration/prepare-all.sh                       # replays recordings; no live model calls
integration/lib/build.sh && npm --prefix web ci && npm --prefix web run build
PORT_GATEWAY=9380 PORT_GITHUB=9381 integration/start-page.sh
```

Page: http://127.0.0.1:9380/ — sign-off token `reviewer-token` (login `reviewer-a`). `integration/verify.sh` checks everything (about 15 minutes).

## What changed in the extension

- Contracts were unfrozen by the owner and changed in three rounds (replay source `test`, review citations, viewer and page endpoints, library facts as assumptions, corrected fixtures, short-review fields).
- Charon and Aeneas installed (`crates/translator-rust/tools/setup.sh`); one row reads "checked by Lean (Aeneas)".
- The proof chain now compares a loop body per iteration and whole methods otherwise; 2234's missing-member consequence is a difference; conditions are readable.
- Test links fixed (reverts were all read as "does not compile"); rules read the worker's facts; reach direction fixed.
- The page was redesigned twice after the owner's feedback: one screen per review, a status by fixed rules, an AI summary kept apart from computed evidence, behaviour groups, inline diff, start page, "How it works", the Witness logo.

## Known gaps

- 2129 is mostly not compared (83 of 450 lines): new methods with loops do not lower.
- Many differences have no live replay (2234: 3 of 12; 2129: 0 of 11; several kinds cannot be replayed at all).
- No record carries a diagnostic's title, so the page says "analyzer diagnostic MsgPack016" without its meaning.
- 2234 is orange: its "members silently dropped" change is recorded as an output change, which the status rule does not count as severe.
- The pipeline has only ever run on these four changes of this one repository. Intake by pasted link exists as a command-line tool tested on local repositories; it is not connected to the page.
- The Lean check rests on five library facts, now listed per unit; carried loop state is compared by the solver only.
- Web viewers (group 22) work through the plugin host, but nothing prepares the folders they read. The remote runner's remote side returns "unavailable".

## Storage

Local disk is 100 GB and filled three times. On the NAS, `/mnt/storage/witness-build/` holds `aeneas-lean-and-elan.tar` (11 GB) and `agent-worktrees-2026-11-14.tar` (6 GB), with a README on how to restore each.
