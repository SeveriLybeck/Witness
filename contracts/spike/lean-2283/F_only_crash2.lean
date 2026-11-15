import Defs
section
variable {Sym Ref Syn Loc Diag Comp Tree : Type}
variable (dsrLen : Sym → Nat) (dsrAt : Sym → Nat → Ref) (getSyntax : Ref → Syn) (isPDS : Syn → Bool)
variable (identLoc : Syn → Loc) (diagCreate : Option Loc → Diag) (diagLoc : Diag → Loc)
variable (isInSource : Loc → Bool) (sourceTree : Loc → Tree) (containsTree : Comp → Tree → Bool) (locNone : Loc)
local notation "BASE" => base dsrLen dsrAt getSyntax isPDS identLoc diagCreate
local notation "HEAD" => head dsrLen dsrAt getSyntax isPDS identLoc diagCreate diagLoc isInSource sourceTree containsTree
variable (K3 : diagLoc (diagCreate none) = locNone) (K4 : isInSource locNone = false)
variable (K6 : ∀ l, diagLoc (diagCreate (some l)) = l)
include K3 K4 K6

/-- FALSE: "only the crash changed" -/
theorem only_crash_changed (comp : Comp) (n : Bool) (item : Sym) :
    HEAD comp n item = BASE n item ∨ (BASE n item = .threw .IndexOutOfRange ∧ HEAD comp n item = .silent) := by
  try unfold base
  try unfold head
  repeat' split
  all_goals (try simp_all)
  repeat' split
  all_goals (try simp_all)
end
