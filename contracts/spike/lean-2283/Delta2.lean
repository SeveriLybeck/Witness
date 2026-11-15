import Defs
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
/-- THE DELTA THEOREM for TC:985. Head equals base except in three named classes; in each, head is silent. -/
theorem delta (comp : Comp) (n : Bool) (item : Sym) :
    HEAD comp n item = BASE n item
    ∨ (BASE n item = .threw .IndexOutOfRange ∧ HEAD comp n item = .silent ∧ dsrLen item = 0)                          -- C1
    ∨ (BASE n item = .threw .InvalidCast ∧ HEAD comp n item = .silent ∧ isPDS (getSyntax (dsrAt item 0)) = false)     -- C2
    ∨ (∃ d, BASE n item = .reported d ∧ HEAD comp n item = .silent ∧ LOCATED comp (diagLoc d) = false) := by          -- C4
  try unfold base
  try unfold head
  unfold located
  repeat' split
  all_goals (try simp_all)
  repeat' split
  all_goals (try simp_all)
end
#print axioms delta
