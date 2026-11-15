3efb84947716c040074aa020ef0af6c3809a0e11

- T6: core Lean 4.29.1 only, no lake, Mathlib or elan; one file per unit, no imports.
- L1: an emit error (unsupported op, sort mismatch) makes the unit's rows not-checked; never shown as a rejected claim.
- L3: hasSorry, any error in a theorem's span, or an axiom beyond propext, Quot.sound, Classical.choice rejects that theorem.
- L4: wall-clock 30 s per theorem in the file and 1.5 GB RSS (polled from /proc); a kill accepts nothing.
- L5: all theorems of a unit in one file; messages mapped to theorems by line span.
