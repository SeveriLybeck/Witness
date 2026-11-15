3efb84947716c040074aa020ef0af6c3809a0e11

- C2 free: network on for the test step only (sandbox `test`); a failure naming NuGet or HTTP is `not-run`, never `fail`.
- C3 free: only the pull request's test methods run (`--filter FullyQualifiedName=...`); a test file that does not compile on old is left out of the old run (`not-compiled`).
- C4 free fallback: one run per side, labelled nowhere yet (no field in `tests.json`).
- C5: generated test from a fixed template filled with the replay input; kept as a patch with its compile result; not run on old and new.
