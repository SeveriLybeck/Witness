import Defs
set_option linter.unusedSimpArgs false
set_option linter.unusedVariables false
open M2234
variable {IK SK I : Type} [DecidableEq IK] [DecidableEq SK]


-- row 3 [same]
theorem row_3 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

