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

/-- crash-freedom of head -/
theorem head_never_throws (comp : Comp) (n : Bool) (item : Sym) (e : Exn) : HEAD comp n item ≠ .threw e := by
  try unfold base
  try unfold head
  repeat' split
  all_goals (try simp_all)
  repeat' split
  all_goals (try simp_all)
/-- no new diagnostics -/
theorem head_reports_subset (comp : Comp) (n : Bool) (item : Sym) (d : Diag) :
    HEAD comp n item = .reported d → BASE n item = .reported d := by
  try unfold base
  try unfold head
  repeat' split
  all_goals (try simp_all)
  repeat' split
  all_goals (try simp_all)
/-- generator path identical -/
theorem generator_identical (comp : Comp) (item : Sym) : HEAD comp true item = BASE true item := by
  try unfold base
  try unfold head
  repeat' split
  all_goals (try simp_all)
  repeat' split
  all_goals (try simp_all)
/-- C1: metadata member: crash becomes SILENCE (needs K3,K4) -/
theorem C1 (comp : Comp) (item : Sym) (h : dsrLen item = 0) :
    BASE false item = .threw .IndexOutOfRange ∧ HEAD comp false item = .silent := by
  try unfold base
  try unfold head
  repeat' split
  all_goals (try simp_all)
  repeat' split
  all_goals (try simp_all)
/-- noninterference (self-composition): on the generator path the outcome is independent of item and comp -/
theorem ni_generator (c1 c2 : Comp) (i1 i2 : Sym) : HEAD c1 true i1 = HEAD c2 true i2 := by
  try unfold base
  try unfold head
  repeat' split
  all_goals (try simp_all)
  repeat' split
  all_goals (try simp_all)
end
#print axioms head_never_throws
#print axioms C1
