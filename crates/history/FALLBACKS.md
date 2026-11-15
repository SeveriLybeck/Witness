3efb84947716c040074aa020ef0af6c3809a0e11

- Playbook §14 items 2, 8, 9 (toolchain.env, flock, systemd-run) skipped: plain `cargo test -j 2`, build takes seconds; no C#, Lean or Z3 needed.
- No contract defines folder history; output is `files.json` with an extra per-file `history` field (extra fields are allowed by `files.schema.json`).
