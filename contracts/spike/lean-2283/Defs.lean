-- minimal probe: which tactic handles the shape quickly?
inductive Exn where | IndexOutOfRange | InvalidCast deriving DecidableEq
inductive Outcome (Diag : Type) where | silent | reported (d : Diag) | threw (e : Exn)
  deriving DecidableEq

section
variable {Sym Ref Syn Loc Diag Comp Tree : Type}
variable (dsrLen : Sym → Nat) (dsrAt : Sym → Nat → Ref) (getSyntax : Ref → Syn) (isPDS : Syn → Bool)
variable (identLoc : Syn → Loc) (diagCreate : Option Loc → Diag) (diagLoc : Diag → Loc)
variable (isInSource : Loc → Bool) (sourceTree : Loc → Tree) (containsTree : Comp → Tree → Bool) (locNone : Loc)

def base (reportIsNull : Bool) (item : Sym) : Outcome Diag :=
  if reportIsNull then .silent
  else if dsrLen item = 0 then .threw .IndexOutOfRange
  else if isPDS (getSyntax (dsrAt item 0)) = false then .threw .InvalidCast
  else .reported (diagCreate (some (identLoc (getSyntax (dsrAt item 0)))))

def head (comp : Comp) (reportIsNull : Bool) (item : Sym) : Outcome Diag :=
  if reportIsNull then .silent
  else
    let r : Option Ref := if dsrLen item = 0 then none else some (dsrAt item 0)
    let s : Option Syn := r.map getSyntax
    let p : Option Syn := s.bind (fun x => if isPDS x then some x else none)
    let loc : Option Loc := p.map identLoc
    let d := diagCreate loc
    if isInSource (diagLoc d) && containsTree comp (sourceTree (diagLoc d))
    then .reported d else .silent
def located (comp : Comp) (l : Loc) : Bool := isInSource l && containsTree comp (sourceTree l)
end
