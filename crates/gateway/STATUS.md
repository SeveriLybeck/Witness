# gateway — status

Works: every endpoint of `contracts/messages/gateway-api.md`; playground forwarding over the line protocol; sessions per bundle and reviewer; the four approval gates; composed body; submission with the reviewer's token; head comparison with stale marking; a GitHub stand-in. 20 tests, all on port 0, responses checked against the contract schemas.
Test: `nix-shell -p cargo rustc gcc pkg-config --run 'cargo test -p gateway'`.
Run: `cargo run -p gateway --bin github-standin -- --pull fork/junction26-messagepack-csharp#1=<newCommit>:author --token TOK=reviewer-a` (port 9261), then
`target/debug/gateway --bundles <dir of bundle dirs> --data work/gateway --repo <clone> --web web/dist --trees 'work/int/sb/wt/{commit}' --plugins plugins --plugins plugins/web --plugin-host target/debug/plugin-host --pull 1` (port 9260; also `--github`, `--github-repo`, `--prepare`, `--plugin-timeout`, `--port`). Without `--plugins`, a `--plugins DIR` inside `--plugin-host` is used.
- `--web DIR`: non-`/api` GETs serve files from DIR with their content type; `/` and unknown paths give `index.html`; `..` gives 400.

Interface
- Bundles: every subfolder of `--bundles` holding `bundle.json`; key `<old>-<new>` from its commits. `prepared` = no section `running`/`not-run`; `stale` = the head moved past it.
- `files.json` is served annotated by `history::annotate` (D44): each file gains `history: {folder, undeclared, reviews}` from signed reviews of the other known bundles, computed per request; the bundle's own review is excluded.
- Data: `<data>/sessions/<key>/<reviewer>/{session,review}.json`, `<data>/stale/<key>` (holds the new key).
- `/api/file`: `git -C <repo> show <commit>:<path>`; lines without newline; 400 for bad sha or `..`, 404 when absent.
- Viewers: manifests from each `--plugins` folder (first name wins) whose `detect` holds on a checkout of the bundle's new commit: `--trees` with `{commit}` if that folder exists, else `<data>/trees/<commit>` written once from the `--repo` clone (own index and work tree; HEAD untouched), else `--repo` if it is not a clone; a bad manifest skips only itself; `title` from the manifest, else the name capitalised with spaces; `outputs` = distinct types. No `--repo` or no plugins: `{viewers: [], tabs: false}`.
- Pages: files under `<data>/plugins/<key>/<plugin>/<newCommit>/` (the host's workdir given `--work <data>/plugins/<key>`); `..`, symlinks out and missing files give 404; unknown types `application/octet-stream`.
- Playground: host started as `<plugin-host cmd...> [--plugins <folder holding the plugin>] --work <data>/plugins/<key> <plugin> <bundle dir>`, one long-lived process per bundle and plugin; the page's request id is swapped for the gateway's and restored on the reply. Each process has its own lock, so a slow plugin blocks only its own (bundle, plugin). No reply within `--plugin-timeout` seconds (default 90): the process is killed, dropped and restarted on the next request; the page gets 502 `error`. 502 also when the host fails.
- Session GET returns an empty session when none is stored. `spotCheck` is set once; later values are ignored (stable pick).
- Submit: no `Authorization` → 401; empty `risk` → 400; any `return` → `REQUEST_CHANGES` without gates; else gates in order section-running, question-unanswered, spot-check (only with a level-1 block), level-3-unopened (full mode) → 409 with `gate` = first unmet and `items` = all unmet items. Then pull head check → 409 `head-moved`, `items: [<new key>]`. Stand-in 422 → 422 `self-approval`, `review.json` records the refusal.
- Pull request number: `bundle.json` `pullRequest`, else `--pull`. Token is sent as `Bearer`, never stored.
- Body lines: Requirement, Looked at (spot-check marked), Tried, Relied on machine checks, Not opened, Not checked, Answers (returned as `q1 return (b1 TypeCollector.cs:239)`), Risk.
- Library: `gateway::{Config, Gateway, start}`, `gateway::review::{approval_gates, compose_body, returned}`, `gateway::standin::start`.

Deviations and fallbacks
- HTTP/1.1 server and client hand-written over std (one request per connection); runtime deps `serde_json`, `history`, `plugin-host`.
- Viewers list only detected plugins, not the host's fallback viewers (run-and-compare, signature caller): the contract says plugins.
- The plugin host's command line is not in the contract; the argv above is this module's choice.
- Head moved: `--prepare <cmd>` runs as `<cmd> <old> <head> <dir>`; without it a placeholder `bundle.json` with all sections `not-run` is written.
- All fixtures have `pullRequest: null`; tests set it to 1 in a copy.
- Playbook §14 items 2, 8, 9 (toolchain.env, flock, systemd-run) skipped: plain `cargo test -j 2`, build takes seconds.
