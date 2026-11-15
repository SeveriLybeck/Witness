# sandbox — status

Works: bwrap runner, working copy per commit, restore-then-offline build of the 2283 head, build queue, per-step network record.
Run: `nix-shell -p cargo rustc gcc pkg-config dotnet-sdk_8 bubblewrap git --run 'cargo test -p sandbox'` (9 tests); a missing or broken clone or working copy is re-created on the next call.
CLI: `cargo run -p sandbox -- [--root NAME] net|nonet [--src] -- cmd...` and `... checkout <rev>` (prints the working copy path).

Interface (`sandbox::`)
- `Sandbox::new(root, nuget)` / `Sandbox::named(name)` (root `$SB_ROOT` or `work/sandbox/<name>`, NuGet `$SB_NUGET` or `work/nuget`).
- `Step::new(name, Net::Off|On, argv)`, `Step::sh(name, net, script)`, `.cwd(rel)`, `.src()` (sample repo read-only at `/untrusted`), `.timeout(d)`.
- `sb.run(&step)` light; `sb.run_heavy(&queue, &step)` waits for a queue slot and runs in `systemd-run --user --scope -p MemoryMax=4G`.
  Both return `StepRecord {step, network, argv, exit, timed_out, millis, stdout, stderr}` and append `{step, network, argv, exit, timedOut, millis}` to `<root>/.sb/steps.jsonl` (`sb.steps()`).
- `sb.working_copy(rev) -> WorkingCopy {commit (40 chars), rel "wt/<commit>", abs}`; `sb.resolve(rev)`; `sb.ensure_clone()`.
- `sb.restore(q, wc, proj)` (net on), `sb.build(q, wc, proj, extra)` (`--no-restore -m:2 -p:UseSharedCompilation=false`, net off), `sb.test(q, wc, proj, extra)` (`--no-build`, net on).
- `Queue::shared()` (four `heavy.N` flock slots in `$LOCKS` or `work/locks`, across processes), `Queue::new(dir, n)`, `q.acquire()`.

Sandbox: `--unshare-all`, `--unshare-net` unless `Net::On` (then `/etc/resolv.conf` read-only), `--clearenv`, `--die-with-parent`, read-only `/nix/store`,
one writable root (cwd, HOME, DOTNET_CLI_HOME), the shared NuGet folder bound read-write, stub one-line `/etc/passwd` and `/etc/group`, tmpfs `/tmp` and `/dev/shm`.
Tools come from the host `PATH` (store paths of dotnet, git, bash, coreutils, grep, sed, find, curl when present); CA bundle from `SSL_CERT_FILE`/`NIX_SSL_CERT_FILE` or `/etc/ssl/certs`.

Deviations and fallbacks
- Toolchain is resolved from `PATH` (the verify `nix-shell`) instead of `contracts/toolchain.env` links, so the crate needs no `.toolchain/` setup.
- `FALLBACKS.md` lives in this directory (writes are limited to `crates/sandbox/`); line 1 is the contract hash, checked by a test.
- Locks default to `work/locks`, not `contracts/LOCKS` (absent; not ours to create). Set `LOCKS` to share slots with other modules.
- M2: if `systemd-run --user --scope` fails, heavy steps run without the scope (checked once; `SB_NO_SCOPE=1` forces this).
- T4 option A: one shared `NUGET_PACKAGES` (`work/nuget`), restore with network, then `--no-restore`. `restore` holds `<nuget>/.restore.lock` (before its queue slot), so restores into one folder run one at a time.
- Clone, fetch and worktree creation hold `<root>/.sb/sandbox-clone.lock` (not `clone.lock`: flock is not re-entrant and testcontract holds that one around its calls). `lock_file(path)` is public.
- Roots under `/tmp` work (bound over the inner tmpfs); `/`, `/tmp`, `/dev/shm`, `/nix`, `/proc`, `/dev` themselves are refused by `Sandbox::new`.
- Network probe uses bash `/dev/tcp` (no curl in the verify shell).
- The clone is offline from the read-only sample at `/untrusted`; change branches fetched as `refs/remotes/origin/ju26-*`; hooks off, `gc.auto=0`.
