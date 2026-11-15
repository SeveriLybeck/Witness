3efb84947716c040074aa020ef0af6c3809a0e11

# translator-rust fallbacks

Tools (2026-11-14), installed by `crates/translator-rust/tools/setup.sh` into `<repo>/work/`:
- aeneas: aeneas 0d21a9e (flake `github:AeneasVerif/aeneas/0d21a9ec7bbbb110110cacc14464a47fd71c8705`), `work/aeneas/bin/aeneas`.
- charon: 0.1.315 (454586e9b770231e66d2062993e632dfc06463e2), the charon Aeneas pins (`…aeneas/<rev>#charon`), `work/charon/bin/charon`.
- Aeneas Lean library: `backends/lean` of that revision, Lean `leanprover/lean4:v4.36.0-rc2` (elan, `work/elan`), mathlib 9d581dff (via `lake exe cache get`). NOT BUILT: the mathlib cache filled the shared disk (ENOSPC at 9.4 GB free; deleted again). setup.sh now refuses below 12 GB free.
- Run: `charon rustc --preset=aeneas [--no-dedup-serialized-ast] --dest-file X.llbc -- --crate-type=lib --crate-name N file.rs`; `aeneas -backend lean -use-lean-modules false -dest DIR X.llbc`; `lake env lean F.lean` in `work/aeneas-lean`.

Fallbacks
- Default model path is Charon LLBC (`llbc::lower`). The `syn` lowering (`lower`) runs only when Charon is absent; `lower_file` returns `Producer::{Charon, SynFallback}`.
- X2: LLBC statements, rvalues and calls outside the subset are an opaque `unmodelled` call, line not modelled.
- 25.3: Aeneas Lean is generated and inspected; the Lean check of a row over Aeneas's definitions skips loudly until the library is built. The differ's rows on the Charon model are checked by `lean-certify` (nix lean 4.29.1) meanwhile.
