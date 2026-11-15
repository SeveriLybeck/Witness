import Defs
set_option linter.unusedSimpArgs false
set_option linter.unusedVariables false
open M2234
variable {IK SK I : Type} [DecidableEq IK] [DecidableEq SK]


def sameTab (ikN skN sup io hi fst isI hI hS fmt : Bool) : Bool :=
  if !ikN && !skN && !sup && !io && !fst && !isI && !hS && !fmt then true else
  if !ikN && !skN && !sup && !io && !fst && !isI && !hS && fmt then true else
  if !ikN && !skN && !sup && !io && !fst && !isI && hS && !fmt then false else
  if !ikN && !skN && !sup && !io && !fst && !isI && hS && fmt then false else
  if !ikN && skN && !sup && !io && !fst && !isI && !fmt then true else
  if !ikN && skN && !sup && !io && !fst && !isI && fmt then false else
  if !ikN && !sup && !io && !fst && isI && !hI && !fmt then true else
  if !ikN && !sup && !io && !fst && isI && !hI && fmt then true else
  if !ikN && !sup && !io && !fst && isI && hI && !fmt then false else
  if !ikN && !sup && !io && !fst && isI && hI && fmt then false else
  if !ikN && !sup && !io && fst && !hI && !fmt then true else
  if !ikN && !sup && !io && fst && !hI && fmt then true else
  if !ikN && !sup && !io && fst && hI && !fmt then false else
  if !ikN && !sup && !io && fst && hI && fmt then false else
  if !ikN && !skN && !sup && io && !hi && !fst && !isI && !hS && !fmt then true else
  if !ikN && !skN && !sup && io && !hi && !fst && !isI && !hS && fmt then true else
  if !ikN && !skN && !sup && io && !hi && !fst && !isI && hS && !fmt then false else
  if !ikN && !skN && !sup && io && !hi && !fst && !isI && hS && fmt then false else
  if !ikN && skN && !sup && io && !hi && !fst && !isI && !fmt then true else
  if !ikN && skN && !sup && io && !hi && !fst && !isI && fmt then false else
  if !ikN && !sup && io && !hi && !fst && isI && !hI && !fmt then true else
  if !ikN && !sup && io && !hi && !fst && isI && !hI && fmt then true else
  if !ikN && !sup && io && !hi && !fst && isI && hI && !fmt then false else
  if !ikN && !sup && io && !hi && !fst && isI && hI && fmt then false else
  if !ikN && !sup && io && !hi && fst && !hI && !fmt then true else
  if !ikN && !sup && io && !hi && fst && !hI && fmt then true else
  if !ikN && !sup && io && !hi && fst && hI && !fmt then false else
  if !ikN && !sup && io && !hi && fst && hI && fmt then false else
  if !ikN && !skN && !sup && io && hi && !fst && !isI && !hS && !fmt then true else
  if !ikN && !skN && !sup && io && hi && !fst && !isI && !hS && fmt then true else
  if !ikN && !skN && !sup && io && hi && !fst && !isI && hS && !fmt then false else
  if !ikN && !skN && !sup && io && hi && !fst && !isI && hS && fmt then false else
  if !ikN && skN && !sup && io && hi && !fst && !isI && !fmt then true else
  if !ikN && skN && !sup && io && hi && !fst && !isI && fmt then false else
  if !ikN && !sup && io && hi && !fst && isI && !hI && !fmt then true else
  if !ikN && !sup && io && hi && !fst && isI && !hI && fmt then true else
  if !ikN && !sup && io && hi && !fst && isI && hI && !fmt then false else
  if !ikN && !sup && io && hi && !fst && isI && hI && fmt then false else
  if !ikN && !sup && io && hi && fst && !hI && !fmt then true else
  if !ikN && !sup && io && hi && fst && !hI && fmt then true else
  if !ikN && !sup && io && hi && fst && hI && !fmt then false else
  if !ikN && !sup && io && hi && fst && hI && fmt then false else
  if !ikN && !skN && sup && !fst && !isI && !hS && !fmt then true else
  if !ikN && !skN && sup && !fst && !isI && !hS && fmt then true else
  if !ikN && !skN && sup && !fst && !isI && hS && !fmt then false else
  if !ikN && !skN && sup && !fst && !isI && hS && fmt then false else
  if !ikN && skN && sup && !fst && !isI && !fmt then true else
  if !ikN && skN && sup && !fst && !isI && fmt then false else
  if !ikN && sup && !fst && isI && !hI && !fmt then true else
  if !ikN && sup && !fst && isI && !hI && fmt then true else
  if !ikN && sup && !fst && isI && hI && !fmt then false else
  if !ikN && sup && !fst && isI && hI && fmt then false else
  if !ikN && sup && fst && !hI && !fmt then true else
  if !ikN && sup && fst && !hI && fmt then true else
  if !ikN && sup && fst && hI && !fmt then false else
  if !ikN && sup && fst && hI && fmt then false else
  if ikN && !skN && !sup && !io && !fst && !isI && !hS && !fmt then true else
  if ikN && !skN && !sup && !io && !fst && !isI && !hS && fmt then true else
  if ikN && !skN && !sup && !io && !fst && !isI && hS && !fmt then false else
  if ikN && !skN && !sup && !io && !fst && !isI && hS && fmt then false else
  if ikN && !skN && !sup && !io && !fst && isI then false else
  if ikN && !skN && !sup && !io && fst && !hS && !fmt then true else
  if ikN && !skN && !sup && !io && fst && !hS && fmt then true else
  if ikN && !skN && !sup && !io && fst && hS && !fmt then false else
  if ikN && !skN && !sup && !io && fst && hS && fmt then false else
  if ikN && !skN && !sup && io && !hi && !fst && !isI && !hS && !fmt then true else
  if ikN && !skN && !sup && io && !hi && !fst && !isI && !hS && fmt then true else
  if ikN && !skN && !sup && io && !hi && !fst && !isI && hS && !fmt then false else
  if ikN && !skN && !sup && io && !hi && !fst && !isI && hS && fmt then false else
  if ikN && !skN && !sup && io && !hi && !fst && isI then false else
  if ikN && !skN && !sup && io && !hi && fst && !hS && !fmt then true else
  if ikN && !skN && !sup && io && !hi && fst && !hS && fmt then true else
  if ikN && !skN && !sup && io && !hi && fst && hS && !fmt then false else
  if ikN && !skN && !sup && io && !hi && fst && hS && fmt then false else
  if ikN && !skN && !sup && io && hi && !fst && !isI && !hS && !fmt then true else
  if ikN && !skN && !sup && io && hi && !fst && !isI && !hS && fmt then true else
  if ikN && !skN && !sup && io && hi && !fst && !isI && hS && !fmt then false else
  if ikN && !skN && !sup && io && hi && !fst && !isI && hS && fmt then false else
  if ikN && !skN && !sup && io && hi && !fst && isI then false else
  if ikN && !skN && !sup && io && hi && fst && !hS && !fmt then true else
  if ikN && !skN && !sup && io && hi && fst && !hS && fmt then true else
  if ikN && !skN && !sup && io && hi && fst && hS && !fmt then false else
  if ikN && !skN && !sup && io && hi && fst && hS && fmt then false else
  if ikN && !skN && sup && !fst && !isI && !hS && !fmt then true else
  if ikN && !skN && sup && !fst && !isI && !hS && fmt then true else
  if ikN && !skN && sup && !fst && !isI && hS && !fmt then false else
  if ikN && !skN && sup && !fst && !isI && hS && fmt then false else
  if ikN && !skN && sup && !fst && isI then false else
  if ikN && !skN && sup && fst && !hS && !fmt then true else
  if ikN && !skN && sup && fst && !hS && fmt then true else
  if ikN && !skN && sup && fst && hS && !fmt then false else
  if ikN && !skN && sup && fst && hS && fmt then false else
  if ikN && skN && !sup && !io && !fst && !isI && !fmt then true else
  if ikN && skN && !sup && !io && !fst && !isI && fmt then false else
  if ikN && skN && !sup && !io && !fst && isI then false else
  if ikN && skN && !sup && !io && fst && !fmt then true else
  if ikN && skN && !sup && !io && fst && fmt then true else
  if ikN && skN && !sup && io && !hi && !fst && !isI && !fmt then true else
  if ikN && skN && !sup && io && !hi && !fst && !isI && fmt then false else
  if ikN && skN && !sup && io && !hi && !fst && isI then false else
  if ikN && skN && !sup && io && !hi && fst && !fmt then true else
  if ikN && skN && !sup && io && !hi && fst && fmt then true else
  if ikN && skN && !sup && io && hi && !fst && !isI && !fmt then true else
  if ikN && skN && !sup && io && hi && !fst && !isI && fmt then false else
  if ikN && skN && !sup && io && hi && !fst && isI then false else
  if ikN && skN && !sup && io && hi && fst && !fmt then true else
  if ikN && skN && !sup && io && hi && fst && fmt then true else
  if ikN && skN && sup && !fst && !isI && !fmt then true else
  if ikN && skN && sup && !fst && !isI && fmt then false else
  if ikN && skN && sup && !fst && isI then false else
  if ikN && skN && sup && fst && !fmt then true else
  if ikN && skN && sup && fst && fmt then true else true
theorem indep_tab : ∀ (ikN skN sup io hi fst isI hI hS fmt : Bool), sameTab ikN skN sup io hi fst isI hI hS fmt = sameTab ikN skN (!sup) io hi fst isI hI hS fmt := by decide
#print axioms indep_tab
