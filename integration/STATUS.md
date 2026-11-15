# integration — status (group 19)

`verify.sh` runs groups 0-18 (each without integration's variables), then per change: prepare from real runs and
recordings (provenance `pipeline`), validate, compare with design.md and the run's established facts
(`check/truth.mjs`; figures and `extra:` lines in RESULTS.md), and the browser click-through twice on its own
stand-in and gateway (free ports 9382-9399, fresh data, `--trees`, `--plugins plugins --plugins plugins/web`).
Click-through, every change: start page lists four changes; first screen within its height budget, review box ≤ 400 px;
one AI-review citation of each kind present followed to its target; Lean sheet lists lean.json's facts; a playground
preset run through the gateway's viewers endpoint; sign-off to APPROVE through the GitHub stand-in.
2283 also: AI-review statement → proof row → playground run → map block.
Recordings: `integration/recordings/` from `record.sh` (round 3 reviews; the only live step, Claude CLI default model;
checked for host paths and secrets); round-2 recordings only.
19.5: each WORKS result is copied to `work/int/replays/<change>/`; tags are the orchestrator's.

Final run: WORKS 2234, 2297, 2129; 2283 fails on a host path shown in the playground output (the old
generator's CS8785 diagnostic text holds an absolute work path): NEEDS web/g11. Its 42 → 0 check passes.

Deviations
- Proof module keeps a worker restart path: a dead worker leaves that file's units out of the compared units.
- Replay snippets: one model call per difference left not replayed (same prompt gives the same key).
- `target/` in this worktree is a symlink to the main checkout's (disk).
- Playwright: nix `playwright-driver.browsers` chromium-1217 matches playwright-core 1.59.1; no fallback.
- Click-through closes the open map block before opening the next level-3 block.
