# remote-runner — status

Works: `Runner` trait; `LocalRunner` (bwrap through `crates/sandbox`); `RemoteRunner` stub; GitHub link check; rate limiter; HTML escaping.
Run: `nix-shell -p cargo rustc gcc pkg-config bubblewrap --run 'cargo test -p remote-runner'` (12 tests, no network, no credentials).

Interface (`remote_runner::`)
- `trait Runner { fn run(&self, &Job) -> Result<RunRecord, RunError> }`; `RunError::{Unavailable, Setup}`.
- `Job::new(name, argv)`, `Job::sh(name, script)`, `.limits(Limits {cpu_secs, memory_mb, wall, processes})`, `input: Option<PathBuf>` (read-only at `/job`).
- `RunRecord {name, exit (None when killed), timed_out, millis, stdout, stderr}`; raw output, display only via `stdout_html()` / `stderr_html()`.
- `LocalRunner::new(base)`: fresh root `base/run.<pid>.<n>` per run, removed afterwards; `argv(job, root)` shows the bwrap line.
- `RemoteRunner { endpoint }` always returns `Unavailable` until Verda access exists.
- `parse_link(url)`, `accept_link(url, &dyn Visibility) -> Result<RepoLink, Rejected>`; `Rejected::{NotGithub, Malformed, Credentials, Private, Unknown, RateLimited}`.
- `Visibility`: `GitHubVisibility` (unauthenticated API, 404 = private), `StaticVisibility(table)`.
- `RateLimiter::new(max, window).check(key)` (sliding window per key).
- `escape_html(s)`: `& < > " ' \`` and control characters.

Sandbox per run: no network, `--clearenv`, non-root uid, root and `/nix/store` read-only, writable `/out` (fresh) and tmpfs `/tmp`,
`ulimit -t` CPU, `-v` memory, `-u` processes, wall-time kill. Links: `https://github.com/<owner>/<repo>[.git]` only.

Deviations and fallbacks (see `FALLBACKS.md`)
- No remote access: one interface, local implementation tested here (D45).
- `sandbox::Toolchain::from_path` needs dotnet and git; this crate builds its own `Toolchain` and `Sandbox` value and reuses `bwrap_argv`, rebinding the root read-only.
- CPU and memory via `ulimit` inside bwrap, not cgroups: an `RLIMIT_AS` cap, not a resident-memory cap.
- Not a security boundary on this machine; strangers' code belongs on the remote machine.
