# translator-rust — status

Works: Charon (0.1.315, the revision Aeneas 0d21a9e pins) on the pair `tests/data/{old,new}.rs` (`pick`: old panics when `flag`, new returns 0). Its LLBC is lowered to the model, which validates. The differ finds the planted row (throws `panic` → value, literal `flag`), and `lean-certify` reads it "checked by Lean". Aeneas translates the pair to Lean (old `massert (¬ flag)`, new none).
Not yet: Lean check of a row over Aeneas's definitions. The Aeneas Lean library needs mathlib (~10 GB); the shared disk ran out. The test skips loudly until `work/aeneas-lean` is built.
Setup: `crates/translator-rust/tools/setup.sh` (nix flakes; pinned; prints versions; exit 3 below 12 GB free at the mathlib step).
Test: `nix-shell -p cargo rustc gcc pkg-config z3 lean4 --run 'cargo test -j 2 -p translator-rust'` (8 tests). The tools are found under `<repo>/work/` (override with CHARON, AENEAS, AENEAS_LEAN, ELAN_HOME); they are not taken from the shell.
CLI: `translator-rust lower --src FILE --function NAME --version old|new [--file PATH] [--work DIR] > model.json`; stderr says `model from charon` or `model from syn-fallback`.

Interface
- `lower_file(src, &Options{function, file, version}, work) -> (model, Producer::{Charon, SynFallback})`: Charon when installed, else `syn`.
- `llbc::lower(&llbc_json, function, file, version)` reads `--no-dedup-serialized-ast` LLBC. `lower(src_text, &Options)` is the syn fallback.
- `tools::{charon, aeneas, aeneas_lean, charon_llbc, aeneas_to_lean, check_lean, version}`.
- Unit id `<fn>#<first line of the fn>`. Sorts: bool → Bool, integers → Int, slices/refs to them → Obj (`index` carries the element sort).

LLBC subset: Assign (Use, Ref, Not, Eq/Ne/BitAnd/BitOr), Assert (if ok then rest else `throw panic`), boolean Switch (each arm continues with the statements after the switch unless it returns), Return, Abort, `core::slice::index` (`index`, contract `index-empty`), panic calls. Storage statements are ignored.

Deviations and fallbacks (FALLBACKS.md)
- X2: anything else is an opaque `unmodelled` call and its line is not modelled. Integer arithmetic has no model op.
- The `syn` lowering stays only as a labelled fallback when Charon is absent.
- All panics share the exception name `panic`.
- 25.3 open: the Aeneas row theorem (`row_flag`, `simp [pick_old.pick, pick_new.pick, massert]`) has never run; its proof script may need adjusting once the library is built.
