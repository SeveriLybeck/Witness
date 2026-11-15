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
theorem exhaustive : ∀ (n z p l : Bool), ((n) || (!n && z) || (!n && !z && !p) || (!n && !z && p && l) || (!n && !z && p && !l)) = true := by decide
#print axioms exhaustive
end
