import Defs
section
variable {Sym Ref Syn Loc Diag Comp Tree : Type}
variable (dsrLen : Sym → Nat) (dsrAt : Sym → Nat → Ref) (getSyntax : Ref → Syn) (isPDS : Syn → Bool)
variable (identLoc : Syn → Loc) (diagCreate : Option Loc → Diag) (diagLoc : Diag → Loc)
variable (isInSource : Loc → Bool) (sourceTree : Loc → Tree) (containsTree : Comp → Tree → Bool) (locNone : Loc)
variable (K3 : diagLoc (diagCreate none) = locNone) (K4 : isInSource locNone = false)
variable (K6 : ∀ l, diagLoc (diagCreate (some l)) = l)
include K3 K4 K6
theorem delta (comp : Comp) (n : Bool) (item : Sym) :
    head dsrLen dsrAt getSyntax isPDS identLoc diagCreate diagLoc isInSource sourceTree containsTree comp n item
      = base dsrLen dsrAt getSyntax isPDS identLoc diagCreate n item
    ∨ (base dsrLen dsrAt getSyntax isPDS identLoc diagCreate n item = .threw .IndexOutOfRange
        ∧ head dsrLen dsrAt getSyntax isPDS identLoc diagCreate diagLoc isInSource sourceTree containsTree comp n item = .silent
        ∧ dsrLen item = 0)
    ∨ (base dsrLen dsrAt getSyntax isPDS identLoc diagCreate n item = .threw .InvalidCast
        ∧ head dsrLen dsrAt getSyntax isPDS identLoc diagCreate diagLoc isInSource sourceTree containsTree comp n item = .silent
        ∧ isPDS (getSyntax (dsrAt item 0)) = false)
    ∨ (∃ d, base dsrLen dsrAt getSyntax isPDS identLoc diagCreate n item = .reported d
        ∧ head dsrLen dsrAt getSyntax isPDS identLoc diagCreate diagLoc isInSource sourceTree containsTree comp n item = .silent
        ∧ located isInSource sourceTree containsTree comp (diagLoc d) = false) := by
  unfold base head located; repeat' split
  all_goals simp_all
end
#print axioms delta
