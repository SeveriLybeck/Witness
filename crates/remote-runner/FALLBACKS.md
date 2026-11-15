3efb84947716c040074aa020ef0af6c3809a0e11
Fallbacks taken (playbook §14, D45):
- No Verda access: `RemoteRunner` is a stub returning `Unavailable`; `LocalRunner` implements the same `Runner` trait on this machine (bwrap via `crates/sandbox`).
- `sandbox::Toolchain::from_path` requires dotnet and git; the local runner builds its own `sandbox::Toolchain` (bwrap, bash, coreutils) from `PATH` and a `sandbox::Sandbox` value directly, then reuses `Sandbox::bwrap_argv`.
- The writable sandbox root is rebound read-only; only `/out` (fresh per run) and tmpfs `/tmp` are writable.
- CPU and memory limits use `ulimit -t/-v/-u` inside the sandbox (no cgroup delegation needed); wall time is a kill-on-timeout loop.
- Public-repository check is the `Visibility` trait; tests use a fixed table. `GitHubVisibility` (unauthenticated `curl` to api.github.com) is never called by tests.
- No contract record carries runner output; no fixture to validate.
