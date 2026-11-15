# plugin-host — status

Works: manifest loading and checks, deterministic detection, the long-lived line-protocol process with no network, the viewer ladder, signature forms.
Run: `nix-shell -p cargo rustc gcc pkg-config --run 'cargo test -p plugin-host'` (10 tests, test plugin `src/bin/echo-plugin.rs`).
Binary (the gateway's argv): `plugin-host [--plugins <dir>] [--work <dir>] <plugin> <bundle dir>`; plugins dir also `$PLUGIN_HOST_PLUGINS` (default `plugins/`), work `$PLUGIN_HOST_WORK` (default `work/plugins`). Starts `<plugins>/<plugin>/plugin.json` via `PluginProcess::start_with_bundle` (env `PLUGIN_BUNDLE_DIR=<absolute bundle dir>`) in `<work>/<plugin>/<newCommit>`, relays lines with the caller's `id` unchanged; a `describe` without `bundle`/`workdir` gets them from `<bundle dir>/bundle.json` and the folder. A failed relay answers `{id, op, ok: false, error}`.

Interface (Rust, `plugin_host::`)
- `Manifest::load(path) / parse(json, dir)`: rejects an output outside `RESULT_TYPES` with `output <name>: unknown result type "<t>"`, then validates against `plugin-manifest.schema.json` (no sandbox field). `name() executable() args() outputs() config()`.
- `load_plugins(dir)`: every `<dir>/*/plugin.json`, sorted.
- `Manifest::detect(repo)`: all rules must hold (AND). Globs: `**/`, `**`, `*`, `?`, over sorted repo files, `.git` skipped. `package-reference` matches `Include="X"`, `"X":`, or a line `X =`.
- `PluginProcess::start(&manifest, workdir) / spawn(exe, args, workdir)`: one process, cwd = workdir (created), no network. `request(json)` assigns `id`, writes one line, reads one line, checks id and `plugin-reply.schema.json`. `start_with_bundle(&m, workdir, bundle_dir)` / `spawn_env(exe, args, workdir, env)` add environment. `relay(json)` keeps the caller's id. Helpers `describe(old, new)`, `run(subject, inputs, sides, preset)`, `shutdown()`.
- `choose_viewers(repo, &plugins) -> Viewers { viewers, tabs }`: matching plugins; else `RunAndCompare` if `runnable(repo)` (a build file from `RUNNABLE`); else `SignatureCaller`. `tabs = viewers.len() > 1`. No test viewer exists.
- `signature_slots(&[(name, type)])` / `form_field(type)`: plugin-reply slots; unknown types are `constructible: false, field: null`.

Deviations and fallbacks
- No playbook fallback taken.
- Network isolation is chosen by the host, not the contract: `unshare -rn` (user + empty net namespace; `lo` brought up via `sh -c 'ip link set lo up; exec …'` when `ip` exists, so plugins can serve and connect on 127.0.0.1), else `bwrap --unshare-net` (brings `lo` up itself); with neither, spawn fails.
- Detection rules in one manifest are ANDed (the schema does not say); the 2283 example needs both.
- Run-and-compare and signature-caller are viewer descriptions only; running them is left to their consumers.
- No request timeout: a plugin that never answers blocks `request`.
