-- availability probes on core Lean 4.29.1 (no Mathlib)
import Std.Tactic.BVDecide
theorem bv_ok (x y : BitVec 32) : (x &&& y) + (x ||| y) = x + y := by bv_decide
theorem om_ok (i len : Int) (h : 0 ≤ i) (h2 : i < len) : i + 1 ≤ len := by omega
#print axioms bv_ok
#print axioms om_ok
theorem bv_bad (x : BitVec 32) : x + 1 > x := by bv_decide
