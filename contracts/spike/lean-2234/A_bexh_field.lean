import Defs
set_option linter.unusedSimpArgs false
set_option linter.unusedVariables false
open M2234
variable {IK SK I : Type} [DecidableEq IK] [DecidableEq SK]


theorem exhaustive : ∀ (ikN skN sup io hi fst isI hI hS fmt : Bool),
    ((!ikN && !skN && !fst && !isI && !hS && !fmt)
    || (!ikN && !skN && !fst && !isI && !hS && fmt)
    || (!ikN && !skN && !fst && !isI && hS && !fmt)
    || (!ikN && !skN && !fst && !isI && hS && fmt)
    || (!ikN && skN && !fst && !isI)
    || (!ikN && !fst && isI && !hI && !fmt)
    || (!ikN && !fst && isI && !hI && fmt)
    || (!ikN && !fst && isI && hI && !fmt)
    || (!ikN && !fst && isI && hI && fmt)
    || (!ikN && fst && !hI && !fmt)
    || (!ikN && fst && !hI && fmt)
    || (!ikN && fst && hI && !fmt)
    || (!ikN && fst && hI && fmt)
    || (ikN && !skN && !fst && !isI && !hS && !fmt)
    || (ikN && !skN && !fst && !isI && !hS && fmt)
    || (ikN && !skN && !fst && !isI && hS && !fmt)
    || (ikN && !skN && !fst && !isI && hS && fmt)
    || (ikN && !skN && !fst && isI)
    || (ikN && !skN && fst && !hS && !fmt)
    || (ikN && !skN && fst && !hS && fmt)
    || (ikN && !skN && fst && hS && !fmt)
    || (ikN && !skN && fst && hS && fmt)
    || (ikN && skN && !fst && !isI)
    || (ikN && skN && !fst && isI)
    || (ikN && skN && fst)) = true := by
  decide
#print axioms exhaustive
