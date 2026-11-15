import Defs
set_option linter.unusedSimpArgs false
open M2234
variable {IK SK I : Type} [DecidableEq IK] [DecidableEq SK]

-- Strategy C: split on the named atoms first, then simp_all at each leaf
set_option hygiene false in
macro "atom_split" : tactic => `(tactic| (
  rcases hik : E.ik i with _ | k1 <;> rcases hsk : E.sk i with _ | k2 <;>
  cases hsup : E.suppress <;> cases hio : E.initOnly i <;> cases hhi : E.hasInit i <;>
  cases hf : s.first <;> cases hii : s.isInt <;> cases hfm : E.fmtNull i <;>
  (try cases hhI : has s.intM ‹IK›) <;> (try cases hhS : has s.strM ‹SK›) <;>
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew,
            fin, report, mixCond, preState]))

theorem delta_prop_C (E : Env IK SK I) (s : St IK SK I) (i : I) :
    bodyNew E false s i = bodyOld E false s i
    ∨ ((preState E false s i).first = false ∧ mixCond E (preState E false s i) i = true
        ∧ bodyNew E false s i = .ok (report E (.mix i) (preState E false s i)))
    ∨ (∃ x, bodyOld E false s i = .threw .argDup x ∧ bodyNew E false s i = fin E i x) := by
  atom_split
#print axioms delta_prop_C
