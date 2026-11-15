3efb84947716c040074aa020ef0af6c3809a0e11
- P3 option A, partly: the Rust side allows 60 s per worker request, then kills and restarts the worker; inputs over 64 kB are refused. Round trips run in the worker's bwrap child (worker's own 30 s limit); no memory scope on that child.
- P1 option B checked for consumer projects: `write_consumer` adds `Microsoft.Net.Compilers.Toolset` 4.12.0; the in-process worker already uses Roslyn 4.12.0.
- Checklist §14 items 2, 8 skipped: toolchain from the verify `nix-shell`; heavy steps go through `sandbox::Queue::shared()`.
