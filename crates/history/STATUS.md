# history — status

Works: per-folder count of undeclared differences from earlier signed reviews (D44), attached to `files.json` for the files index. 4 tests on the recorded 2283, 2297, 2234, 2129 reviews and bundles.
Test: `nix-shell -p cargo rustc gcc pkg-config --run 'cargo test -p history'`.
Run: `cargo run -p history -- --sessions <data>/sessions --bundle <bundle dir>... --files <bundle>/files.json [--exclude <key>]` prints the annotated `files.json`.

Interface
- Signed review: `review.json` with `event: APPROVE` and `result.status` 2xx. A bundle signed by several reviewers counts once.
- Undeclared difference: its block's `links.json` state is not `declared` (D27). Folder = `location.file` up to the last `/` (root is `""`).
- Sessions layout: `<root>/<key>/<reviewer>/review.json` (the gateway's `--data`); bundles matched by `bundle.json` `oldCommit-newCommit`. Reviews without a known bundle are skipped.
- `--exclude <key>`: the bundle under review is not its own history.
- Output: each `files[]` entry gains `history: {folder, undeclared, reviews}` (`reviews` = number of signed bundles contributing). Validates against `files.schema.json`.
- Library: `history::{count, annotate, undeclared, signed, bundle_key, folder, reviews, Counts, FolderCount}`.

Recorded values: 2283 has 3 undeclared (b2, `src/MessagePack.SourceGenerator/CodeAnalysis`); 2297 has 1.

Deviations and fallbacks
- No contract names a folder-history record or endpoint; the counts ride as an extra field on `files.json` instead of a new record. The gateway does not yet call this; consumers run the binary or library and serve the result as `files.json`.
- Playbook §14 items 2, 8, 9 (toolchain.env, flock, systemd-run) skipped: plain `cargo test -j 2`.
