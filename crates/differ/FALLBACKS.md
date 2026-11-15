3efb84947716c040074aa020ef0af6c3809a0e11
D1 option A: SMT-LIB text to one `z3 -in` per unit with push/pop; examples via `(get-value …)` on the inputs. Checker `Z3 4.16.0`.
D2 option A: quantifier-free, `(set-option :timeout 2000)`; every reference type is one sort `Obj` (not one per C# type). `unknown` keeps the row as differing with `undecided: true`; cvc5 retry not built.
D3 option A: rows grouped by (kind, old tag, new tag, first changed location) in `group`.
D6 option A: self-checks (same version, flipped branches, `&&`→`||`, `[0]`→`[1]`) are tests and pass.
