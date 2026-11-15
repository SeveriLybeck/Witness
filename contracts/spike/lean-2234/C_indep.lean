import Defs
set_option linter.unusedSimpArgs false
open M2234
variable {IK SK I : Type} [DecidableEq IK] [DecidableEq SK]
set_option hygiene false in
macro "atom_split" : tactic => `(tactic| (
  rcases hik : E.ik i with _ | k1 <;> rcases hsk : E.sk i with _ | k2 <;>
  cases hsup : E.suppress <;> cases hio : E.initOnly i <;> cases hhi : E.hasInit i <;>
  cases hf : s.first <;> cases hii : s.isInt <;> cases hfm : E.fmtNull i <;>
  (try cases hhI : has s.intM ‹IK›) <;> (try cases hhS : has s.strM ‹SK›) <;>
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew,
            fin, report, mixCond, preState]))
theorem indep_suppress_C (E : Env IK SK I) (s : St IK SK I) (i : I) :
    (bodyOld E false s i = bodyNew E false s i) ↔
    (bodyOld { E with suppress := !E.suppress } false s i = bodyNew { E with suppress := !E.suppress } false s i) := by
  atom_split
#print axioms indep_suppress_C
