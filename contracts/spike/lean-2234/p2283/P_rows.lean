import Defs
set_option linter.unusedSimpArgs false
set_option linter.unusedVariables false
section
variable {Sym Ref Syn Loc Diag Comp Tree : Type}
variable (dsrLen : Sym → Nat) (dsrAt : Sym → Nat → Ref) (getSyntax : Ref → Syn) (isPDS : Syn → Bool)
variable (identLoc : Syn → Loc) (diagCreate : Option Loc → Diag) (diagLoc : Diag → Loc)
variable (isInSource : Loc → Bool) (sourceTree : Loc → Tree) (containsTree : Comp → Tree → Bool) (locNone : Loc)
local notation "BASE" => base dsrLen dsrAt getSyntax isPDS identLoc diagCreate
local notation "HEAD" => head dsrLen dsrAt getSyntax isPDS identLoc diagCreate diagLoc isInSource sourceTree containsTree
local notation "LOCATED" => located isInSource sourceTree containsTree
variable (K3 : diagLoc (diagCreate none) = locNone) (K4 : isInSource locNone = false)
variable (K6 : ∀ l, diagLoc (diagCreate (some l)) = l)
include K3 K4 K6
theorem row_0 (comp : Comp) (n : Bool) (item : Sym) (h_n : n = true) :
    BASE n item = .silent ∧ HEAD comp n item = .silent := by
  simp_all [base, head, located]
#print axioms row_0
theorem row_1 (comp : Comp) (n : Bool) (item : Sym) (h_n : n = false) (h_z : dsrLen item = 0) :
    BASE n item = .threw .IndexOutOfRange ∧ HEAD comp n item = .silent := by
  simp_all [base, head, located]
#print axioms row_1
theorem row_2 (comp : Comp) (n : Bool) (item : Sym) (h_n : n = false) (h_z : ¬ dsrLen item = 0) (h_p : isPDS (getSyntax (dsrAt item 0)) = false) :
    BASE n item = .threw .InvalidCast ∧ HEAD comp n item = .silent := by
  simp_all [base, head, located]
#print axioms row_2
theorem row_3 (comp : Comp) (n : Bool) (item : Sym) (h_n : n = false) (h_z : ¬ dsrLen item = 0) (h_p : isPDS (getSyntax (dsrAt item 0)) = true) (h_l : LOCATED comp (identLoc (getSyntax (dsrAt item 0))) = true) :
    BASE n item = .reported (diagCreate (some (identLoc (getSyntax (dsrAt item 0))))) ∧ HEAD comp n item = .reported (diagCreate (some (identLoc (getSyntax (dsrAt item 0))))) := by
  simp_all [base, head, located]
#print axioms row_3
theorem row_4 (comp : Comp) (n : Bool) (item : Sym) (h_n : n = false) (h_z : ¬ dsrLen item = 0) (h_p : isPDS (getSyntax (dsrAt item 0)) = true) (h_l : LOCATED comp (identLoc (getSyntax (dsrAt item 0))) = false) :
    BASE n item = .reported (diagCreate (some (identLoc (getSyntax (dsrAt item 0))))) ∧ HEAD comp n item = .silent := by
  simp_all [base, head, located]
#print axioms row_4
end
