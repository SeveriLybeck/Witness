import Defs
section
variable {Sym Ref Syn Loc Diag Comp Tree : Type}
variable (dsrLen : Sym → Nat) (dsrAt : Sym → Nat → Ref) (getSyntax : Ref → Syn) (isPDS : Syn → Bool)
variable (identLoc : Syn → Loc) (diagCreate : Option Loc → Diag) (diagLoc : Diag → Loc)
variable (isInSource : Loc → Bool) (sourceTree : Loc → Tree) (containsTree : Comp → Tree → Bool) (locNone : Loc)
local notation "BASE" => base dsrLen dsrAt getSyntax isPDS identLoc diagCreate
local notation "HEAD" => head dsrLen dsrAt getSyntax isPDS identLoc diagCreate diagLoc isInSource sourceTree containsTree

/-- ABLATION: C1 without the Roslyn contracts K3,K4 must NOT be provable -/
theorem C1_noK (comp : Comp) (item : Sym) (h : dsrLen item = 0) :
    BASE false item = .threw .IndexOutOfRange ∧ HEAD comp false item = .silent := by
  try unfold base
  try unfold head
  repeat' split
  all_goals (try simp_all)
  repeat' split
  all_goals (try simp_all)
end
