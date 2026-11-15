import Defs
set_option linter.unusedSimpArgs false
set_option linter.unusedVariables false
open M2234
variable {IK SK I : Type} [DecidableEq IK] [DecidableEq SK]


-- row 0 [same]
theorem row_0 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 1 [same]
theorem row_1 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } ∧ bodyNew E false s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 2 [DIFF]
theorem row_2 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { report E (.dup i) s with hidden := s.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 3 [DIFF]
theorem row_3 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { { report E (.dup i) s with hidden := s.hidden + 1 } with collected := i :: { report E (.dup i) s with hidden := s.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 4 [same]
theorem row_4 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok (report E (.mix i) s) ∧ bodyNew E false s i = .ok (report E (.mix i) s) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 5 [DIFF]
theorem row_5 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { (report E (.mix i) s) with collected := i :: (report E (.mix i) s).collected } ∧ bodyNew E false s i = .ok (report E (.mix i) s) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 6 [same]
theorem row_6 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { s with intM := (k1, i) :: s.intM } ∧ bodyNew E false s i = .ok { s with intM := (k1, i) :: s.intM } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 7 [same]
theorem row_7 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { s with intM := (k1, i) :: s.intM } with collected := i :: { s with intM := (k1, i) :: s.intM }.collected } ∧ bodyNew E false s i = .ok { { s with intM := (k1, i) :: s.intM } with collected := i :: { s with intM := (k1, i) :: s.intM }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 8 [DIFF]
theorem row_8 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) s) ∧ bodyNew E false s i = .ok (report E (.dup i) s) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 9 [DIFF]
theorem row_9 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) s) ∧ bodyNew E false s i = .ok { (report E (.dup i) s) with collected := i :: (report E (.dup i) s).collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 10 [same]
theorem row_10 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } ∧ bodyNew E false s i = .ok { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 11 [same]
theorem row_11 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } with collected := i :: { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM }.collected } ∧ bodyNew E false s i = .ok { { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } with collected := i :: { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 12 [DIFF]
theorem row_12 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) { s with first := false, isInt := true }) ∧ bodyNew E false s i = .ok (report E (.dup i) { s with first := false, isInt := true }) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 13 [DIFF]
theorem row_13 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) { s with first := false, isInt := true }) ∧ bodyNew E false s i = .ok { (report E (.dup i) { s with first := false, isInt := true }) with collected := i :: (report E (.dup i) { s with first := false, isInt := true }).collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 14 [same]
theorem row_14 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 15 [same]
theorem row_15 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } ∧ bodyNew E false s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 16 [DIFF]
theorem row_16 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { report E (.dup i) s with hidden := s.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 17 [DIFF]
theorem row_17 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { { report E (.dup i) s with hidden := s.hidden + 1 } with collected := i :: { report E (.dup i) s with hidden := s.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 18 [same]
theorem row_18 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok (report E (.mix i) s) ∧ bodyNew E false s i = .ok (report E (.mix i) s) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 19 [DIFF]
theorem row_19 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { (report E (.mix i) s) with collected := i :: (report E (.mix i) s).collected } ∧ bodyNew E false s i = .ok (report E (.mix i) s) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 20 [same]
theorem row_20 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { s with intM := (k1, i) :: s.intM } ∧ bodyNew E false s i = .ok { s with intM := (k1, i) :: s.intM } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 21 [same]
theorem row_21 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { s with intM := (k1, i) :: s.intM } with collected := i :: { s with intM := (k1, i) :: s.intM }.collected } ∧ bodyNew E false s i = .ok { { s with intM := (k1, i) :: s.intM } with collected := i :: { s with intM := (k1, i) :: s.intM }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 22 [DIFF]
theorem row_22 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) s) ∧ bodyNew E false s i = .ok (report E (.dup i) s) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 23 [DIFF]
theorem row_23 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) s) ∧ bodyNew E false s i = .ok { (report E (.dup i) s) with collected := i :: (report E (.dup i) s).collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 24 [same]
theorem row_24 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } ∧ bodyNew E false s i = .ok { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 25 [same]
theorem row_25 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } with collected := i :: { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM }.collected } ∧ bodyNew E false s i = .ok { { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } with collected := i :: { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 26 [DIFF]
theorem row_26 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) { s with first := false, isInt := true }) ∧ bodyNew E false s i = .ok (report E (.dup i) { s with first := false, isInt := true }) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 27 [DIFF]
theorem row_27 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) { s with first := false, isInt := true }) ∧ bodyNew E false s i = .ok { (report E (.dup i) { s with first := false, isInt := true }) with collected := i :: (report E (.dup i) { s with first := false, isInt := true }).collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 28 [same]
theorem row_28 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { (report E (.aot i) s) with strM := (k2, i) :: (report E (.aot i) s).strM, hidden := (report E (.aot i) s).hidden + 1 } ∧ bodyNew E false s i = .ok { (report E (.aot i) s) with strM := (k2, i) :: (report E (.aot i) s).strM, hidden := (report E (.aot i) s).hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 29 [same]
theorem row_29 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { (report E (.aot i) s) with strM := (k2, i) :: (report E (.aot i) s).strM, hidden := (report E (.aot i) s).hidden + 1 } with collected := i :: { (report E (.aot i) s) with strM := (k2, i) :: (report E (.aot i) s).strM, hidden := (report E (.aot i) s).hidden + 1 }.collected } ∧ bodyNew E false s i = .ok { { (report E (.aot i) s) with strM := (k2, i) :: (report E (.aot i) s).strM, hidden := (report E (.aot i) s).hidden + 1 } with collected := i :: { (report E (.aot i) s) with strM := (k2, i) :: (report E (.aot i) s).strM, hidden := (report E (.aot i) s).hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 30 [DIFF]
theorem row_30 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) (report E (.aot i) s) with hidden := (report E (.aot i) s).hidden + 1 } ∧ bodyNew E false s i = .ok { report E (.dup i) (report E (.aot i) s) with hidden := (report E (.aot i) s).hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 31 [DIFF]
theorem row_31 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) (report E (.aot i) s) with hidden := (report E (.aot i) s).hidden + 1 } ∧ bodyNew E false s i = .ok { { report E (.dup i) (report E (.aot i) s) with hidden := (report E (.aot i) s).hidden + 1 } with collected := i :: { report E (.dup i) (report E (.aot i) s) with hidden := (report E (.aot i) s).hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 32 [same]
theorem row_32 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok (report E (.mix i) (report E (.aot i) s)) ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.aot i) s)) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 33 [DIFF]
theorem row_33 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { (report E (.mix i) (report E (.aot i) s)) with collected := i :: (report E (.mix i) (report E (.aot i) s)).collected } ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.aot i) s)) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 34 [same]
theorem row_34 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { (report E (.aot i) s) with intM := (k1, i) :: (report E (.aot i) s).intM } ∧ bodyNew E false s i = .ok { (report E (.aot i) s) with intM := (k1, i) :: (report E (.aot i) s).intM } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 35 [same]
theorem row_35 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { (report E (.aot i) s) with intM := (k1, i) :: (report E (.aot i) s).intM } with collected := i :: { (report E (.aot i) s) with intM := (k1, i) :: (report E (.aot i) s).intM }.collected } ∧ bodyNew E false s i = .ok { { (report E (.aot i) s) with intM := (k1, i) :: (report E (.aot i) s).intM } with collected := i :: { (report E (.aot i) s) with intM := (k1, i) :: (report E (.aot i) s).intM }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 36 [DIFF]
theorem row_36 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) (report E (.aot i) s)) ∧ bodyNew E false s i = .ok (report E (.dup i) (report E (.aot i) s)) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 37 [DIFF]
theorem row_37 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) (report E (.aot i) s)) ∧ bodyNew E false s i = .ok { (report E (.dup i) (report E (.aot i) s)) with collected := i :: (report E (.dup i) (report E (.aot i) s)).collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 38 [same]
theorem row_38 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { { (report E (.aot i) s) with first := false, isInt := true } with intM := (k1, i) :: { (report E (.aot i) s) with first := false, isInt := true }.intM } ∧ bodyNew E false s i = .ok { { (report E (.aot i) s) with first := false, isInt := true } with intM := (k1, i) :: { (report E (.aot i) s) with first := false, isInt := true }.intM } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 39 [same]
theorem row_39 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { { (report E (.aot i) s) with first := false, isInt := true } with intM := (k1, i) :: { (report E (.aot i) s) with first := false, isInt := true }.intM } with collected := i :: { { (report E (.aot i) s) with first := false, isInt := true } with intM := (k1, i) :: { (report E (.aot i) s) with first := false, isInt := true }.intM }.collected } ∧ bodyNew E false s i = .ok { { { (report E (.aot i) s) with first := false, isInt := true } with intM := (k1, i) :: { (report E (.aot i) s) with first := false, isInt := true }.intM } with collected := i :: { { (report E (.aot i) s) with first := false, isInt := true } with intM := (k1, i) :: { (report E (.aot i) s) with first := false, isInt := true }.intM }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 40 [DIFF]
theorem row_40 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) { (report E (.aot i) s) with first := false, isInt := true }) ∧ bodyNew E false s i = .ok (report E (.dup i) { (report E (.aot i) s) with first := false, isInt := true }) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 41 [DIFF]
theorem row_41 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) { (report E (.aot i) s) with first := false, isInt := true }) ∧ bodyNew E false s i = .ok { (report E (.dup i) { (report E (.aot i) s) with first := false, isInt := true }) with collected := i :: (report E (.dup i) { (report E (.aot i) s) with first := false, isInt := true }).collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 42 [same]
theorem row_42 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 43 [same]
theorem row_43 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } ∧ bodyNew E false s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 44 [DIFF]
theorem row_44 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { report E (.dup i) s with hidden := s.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 45 [DIFF]
theorem row_45 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { { report E (.dup i) s with hidden := s.hidden + 1 } with collected := i :: { report E (.dup i) s with hidden := s.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 46 [same]
theorem row_46 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = none) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok (report E (.mix i) s) ∧ bodyNew E false s i = .ok (report E (.mix i) s) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 47 [DIFF]
theorem row_47 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = none) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { (report E (.mix i) s) with collected := i :: (report E (.mix i) s).collected } ∧ bodyNew E false s i = .ok (report E (.mix i) s) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 48 [same]
theorem row_48 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { s with intM := (k1, i) :: s.intM } ∧ bodyNew E false s i = .ok { s with intM := (k1, i) :: s.intM } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 49 [same]
theorem row_49 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { s with intM := (k1, i) :: s.intM } with collected := i :: { s with intM := (k1, i) :: s.intM }.collected } ∧ bodyNew E false s i = .ok { { s with intM := (k1, i) :: s.intM } with collected := i :: { s with intM := (k1, i) :: s.intM }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 50 [DIFF]
theorem row_50 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) s) ∧ bodyNew E false s i = .ok (report E (.dup i) s) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 51 [DIFF]
theorem row_51 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) s) ∧ bodyNew E false s i = .ok { (report E (.dup i) s) with collected := i :: (report E (.dup i) s).collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 52 [same]
theorem row_52 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = true) (h5 : s.first = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } ∧ bodyNew E false s i = .ok { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 53 [same]
theorem row_53 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = true) (h5 : s.first = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } with collected := i :: { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM }.collected } ∧ bodyNew E false s i = .ok { { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } with collected := i :: { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 54 [DIFF]
theorem row_54 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = true) (h5 : s.first = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) { s with first := false, isInt := true }) ∧ bodyNew E false s i = .ok (report E (.dup i) { s with first := false, isInt := true }) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 55 [DIFF]
theorem row_55 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h2 : E.suppress = true) (h5 : s.first = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup (report E (.dup i) { s with first := false, isInt := true }) ∧ bodyNew E false s i = .ok { (report E (.dup i) { s with first := false, isInt := true }) with collected := i :: (report E (.dup i) { s with first := false, isInt := true }).collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 56 [same]
theorem row_56 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 57 [same]
theorem row_57 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } ∧ bodyNew E false s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 58 [DIFF]
theorem row_58 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { report E (.dup i) s with hidden := s.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 59 [DIFF]
theorem row_59 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { { report E (.dup i) s with hidden := s.hidden + 1 } with collected := i :: { report E (.dup i) s with hidden := s.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 60 [DIFF]
theorem row_60 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = true) :
    bodyOld E false s i = .threw .invalidOp (report E (.mix i) s) ∧ bodyNew E false s i = .ok (report E (.mix i) s) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 61 [same]
theorem row_61 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = true) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E false s i = .ok { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 62 [same]
theorem row_62 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = true) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } with collected := i :: { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 }.collected } ∧ bodyNew E false s i = .ok { { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } with collected := i :: { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 63 [DIFF]
theorem row_63 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = true) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E false s i = .ok { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 64 [DIFF]
theorem row_64 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = true) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E false s i = .ok { { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } with collected := i :: { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 65 [same]
theorem row_65 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 66 [same]
theorem row_66 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } ∧ bodyNew E false s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 67 [DIFF]
theorem row_67 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { report E (.dup i) s with hidden := s.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 68 [DIFF]
theorem row_68 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { { report E (.dup i) s with hidden := s.hidden + 1 } with collected := i :: { report E (.dup i) s with hidden := s.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 69 [DIFF]
theorem row_69 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = true) :
    bodyOld E false s i = .threw .invalidOp (report E (.mix i) s) ∧ bodyNew E false s i = .ok (report E (.mix i) s) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 70 [same]
theorem row_70 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = true) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E false s i = .ok { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 71 [same]
theorem row_71 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = true) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } with collected := i :: { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 }.collected } ∧ bodyNew E false s i = .ok { { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } with collected := i :: { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 72 [DIFF]
theorem row_72 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = true) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E false s i = .ok { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 73 [DIFF]
theorem row_73 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = true) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E false s i = .ok { { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } with collected := i :: { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 74 [same]
theorem row_74 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { (report E (.aot i) s) with strM := (k2, i) :: (report E (.aot i) s).strM, hidden := (report E (.aot i) s).hidden + 1 } ∧ bodyNew E false s i = .ok { (report E (.aot i) s) with strM := (k2, i) :: (report E (.aot i) s).strM, hidden := (report E (.aot i) s).hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 75 [same]
theorem row_75 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { (report E (.aot i) s) with strM := (k2, i) :: (report E (.aot i) s).strM, hidden := (report E (.aot i) s).hidden + 1 } with collected := i :: { (report E (.aot i) s) with strM := (k2, i) :: (report E (.aot i) s).strM, hidden := (report E (.aot i) s).hidden + 1 }.collected } ∧ bodyNew E false s i = .ok { { (report E (.aot i) s) with strM := (k2, i) :: (report E (.aot i) s).strM, hidden := (report E (.aot i) s).hidden + 1 } with collected := i :: { (report E (.aot i) s) with strM := (k2, i) :: (report E (.aot i) s).strM, hidden := (report E (.aot i) s).hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 76 [DIFF]
theorem row_76 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) (report E (.aot i) s) with hidden := (report E (.aot i) s).hidden + 1 } ∧ bodyNew E false s i = .ok { report E (.dup i) (report E (.aot i) s) with hidden := (report E (.aot i) s).hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 77 [DIFF]
theorem row_77 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) (report E (.aot i) s) with hidden := (report E (.aot i) s).hidden + 1 } ∧ bodyNew E false s i = .ok { { report E (.dup i) (report E (.aot i) s) with hidden := (report E (.aot i) s).hidden + 1 } with collected := i :: { report E (.dup i) (report E (.aot i) s) with hidden := (report E (.aot i) s).hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 78 [DIFF]
theorem row_78 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = true) :
    bodyOld E false s i = .threw .invalidOp (report E (.mix i) (report E (.aot i) s)) ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.aot i) s)) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 79 [same]
theorem row_79 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = true) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { { (report E (.aot i) s) with first := false, isInt := false } with strM := (k2, i) :: { (report E (.aot i) s) with first := false, isInt := false }.strM, hidden := { (report E (.aot i) s) with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E false s i = .ok { { (report E (.aot i) s) with first := false, isInt := false } with strM := (k2, i) :: { (report E (.aot i) s) with first := false, isInt := false }.strM, hidden := { (report E (.aot i) s) with first := false, isInt := false }.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 80 [same]
theorem row_80 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = true) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { { (report E (.aot i) s) with first := false, isInt := false } with strM := (k2, i) :: { (report E (.aot i) s) with first := false, isInt := false }.strM, hidden := { (report E (.aot i) s) with first := false, isInt := false }.hidden + 1 } with collected := i :: { { (report E (.aot i) s) with first := false, isInt := false } with strM := (k2, i) :: { (report E (.aot i) s) with first := false, isInt := false }.strM, hidden := { (report E (.aot i) s) with first := false, isInt := false }.hidden + 1 }.collected } ∧ bodyNew E false s i = .ok { { { (report E (.aot i) s) with first := false, isInt := false } with strM := (k2, i) :: { (report E (.aot i) s) with first := false, isInt := false }.strM, hidden := { (report E (.aot i) s) with first := false, isInt := false }.hidden + 1 } with collected := i :: { { (report E (.aot i) s) with first := false, isInt := false } with strM := (k2, i) :: { (report E (.aot i) s) with first := false, isInt := false }.strM, hidden := { (report E (.aot i) s) with first := false, isInt := false }.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 81 [DIFF]
theorem row_81 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = true) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) { (report E (.aot i) s) with first := false, isInt := false } with hidden := { (report E (.aot i) s) with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E false s i = .ok { report E (.dup i) { (report E (.aot i) s) with first := false, isInt := false } with hidden := { (report E (.aot i) s) with first := false, isInt := false }.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 82 [DIFF]
theorem row_82 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = true) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) { (report E (.aot i) s) with first := false, isInt := false } with hidden := { (report E (.aot i) s) with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E false s i = .ok { { report E (.dup i) { (report E (.aot i) s) with first := false, isInt := false } with hidden := { (report E (.aot i) s) with first := false, isInt := false }.hidden + 1 } with collected := i :: { report E (.dup i) { (report E (.aot i) s) with first := false, isInt := false } with hidden := { (report E (.aot i) s) with first := false, isInt := false }.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 83 [same]
theorem row_83 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 84 [same]
theorem row_84 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } ∧ bodyNew E false s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 85 [DIFF]
theorem row_85 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { report E (.dup i) s with hidden := s.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 86 [DIFF]
theorem row_86 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E false s i = .ok { { report E (.dup i) s with hidden := s.hidden + 1 } with collected := i :: { report E (.dup i) s with hidden := s.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 87 [DIFF]
theorem row_87 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = true) :
    bodyOld E false s i = .threw .invalidOp (report E (.mix i) s) ∧ bodyNew E false s i = .ok (report E (.mix i) s) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 88 [same]
theorem row_88 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = true) (h5 : s.first = true) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E false s i = .ok { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 89 [same]
theorem row_89 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = true) (h5 : s.first = true) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } with collected := i :: { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 }.collected } ∧ bodyNew E false s i = .ok { { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } with collected := i :: { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 90 [DIFF]
theorem row_90 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = true) (h5 : s.first = true) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E false s i = .ok { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 91 [DIFF]
theorem row_91 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h2 : E.suppress = true) (h5 : s.first = true) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .threw .argDup { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E false s i = .ok { { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } with collected := i :: { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 92 [same]
theorem row_92 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok (report E (.mix i) (report E (.bothNull i) s)) ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.bothNull i) s)) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 93 [DIFF]
theorem row_93 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { (report E (.mix i) (report E (.bothNull i) s)) with collected := i :: (report E (.mix i) (report E (.bothNull i) s)).collected } ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.bothNull i) s)) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 94 [DIFF]
theorem row_94 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = false) (h6 : s.isInt = true) :
    bodyOld E false s i = .threw .invalidOp (report E (.mix i) (report E (.bothNull i) s)) ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.bothNull i) s)) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 95 [same]
theorem row_95 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { (report E (.bothNull i) s) with first := false, isInt := false } ∧ bodyNew E false s i = .ok { (report E (.bothNull i) s) with first := false, isInt := false } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 96 [same]
theorem row_96 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = false) (h5 : s.first = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { (report E (.bothNull i) s) with first := false, isInt := false } with collected := i :: { (report E (.bothNull i) s) with first := false, isInt := false }.collected } ∧ bodyNew E false s i = .ok { { (report E (.bothNull i) s) with first := false, isInt := false } with collected := i :: { (report E (.bothNull i) s) with first := false, isInt := false }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 97 [same]
theorem row_97 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok (report E (.mix i) (report E (.bothNull i) s)) ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.bothNull i) s)) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 98 [DIFF]
theorem row_98 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { (report E (.mix i) (report E (.bothNull i) s)) with collected := i :: (report E (.mix i) (report E (.bothNull i) s)).collected } ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.bothNull i) s)) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 99 [DIFF]
theorem row_99 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = false) (h6 : s.isInt = true) :
    bodyOld E false s i = .threw .invalidOp (report E (.mix i) (report E (.bothNull i) s)) ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.bothNull i) s)) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 100 [same]
theorem row_100 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { (report E (.bothNull i) s) with first := false, isInt := false } ∧ bodyNew E false s i = .ok { (report E (.bothNull i) s) with first := false, isInt := false } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 101 [same]
theorem row_101 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = false) (h5 : s.first = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { (report E (.bothNull i) s) with first := false, isInt := false } with collected := i :: { (report E (.bothNull i) s) with first := false, isInt := false }.collected } ∧ bodyNew E false s i = .ok { { (report E (.bothNull i) s) with first := false, isInt := false } with collected := i :: { (report E (.bothNull i) s) with first := false, isInt := false }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 102 [same]
theorem row_102 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok (report E (.mix i) (report E (.aot i) (report E (.bothNull i) s))) ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.aot i) (report E (.bothNull i) s))) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 103 [DIFF]
theorem row_103 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { (report E (.mix i) (report E (.aot i) (report E (.bothNull i) s))) with collected := i :: (report E (.mix i) (report E (.aot i) (report E (.bothNull i) s))).collected } ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.aot i) (report E (.bothNull i) s))) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 104 [DIFF]
theorem row_104 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = false) (h6 : s.isInt = true) :
    bodyOld E false s i = .threw .invalidOp (report E (.mix i) (report E (.aot i) (report E (.bothNull i) s))) ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.aot i) (report E (.bothNull i) s))) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 105 [same]
theorem row_105 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { (report E (.aot i) (report E (.bothNull i) s)) with first := false, isInt := false } ∧ bodyNew E false s i = .ok { (report E (.aot i) (report E (.bothNull i) s)) with first := false, isInt := false } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 106 [same]
theorem row_106 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = false) (h3 : E.initOnly i = true) (h4 : E.hasInit i = true) (h5 : s.first = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { (report E (.aot i) (report E (.bothNull i) s)) with first := false, isInt := false } with collected := i :: { (report E (.aot i) (report E (.bothNull i) s)) with first := false, isInt := false }.collected } ∧ bodyNew E false s i = .ok { { (report E (.aot i) (report E (.bothNull i) s)) with first := false, isInt := false } with collected := i :: { (report E (.aot i) (report E (.bothNull i) s)) with first := false, isInt := false }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 107 [same]
theorem row_107 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok (report E (.mix i) (report E (.bothNull i) s)) ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.bothNull i) s)) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 108 [DIFF]
theorem row_108 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = false) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { (report E (.mix i) (report E (.bothNull i) s)) with collected := i :: (report E (.mix i) (report E (.bothNull i) s)).collected } ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.bothNull i) s)) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 109 [DIFF]
theorem row_109 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = true) (h5 : s.first = false) (h6 : s.isInt = true) :
    bodyOld E false s i = .threw .invalidOp (report E (.mix i) (report E (.bothNull i) s)) ∧ bodyNew E false s i = .ok (report E (.mix i) (report E (.bothNull i) s)) := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 110 [same]
theorem row_110 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = true) (h5 : s.first = true) (h9 : E.fmtNull i = false) :
    bodyOld E false s i = .ok { (report E (.bothNull i) s) with first := false, isInt := false } ∧ bodyNew E false s i = .ok { (report E (.bothNull i) s) with first := false, isInt := false } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

-- row 111 [same]
theorem row_111 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h2 : E.suppress = true) (h5 : s.first = true) (h9 : E.fmtNull i = true) :
    bodyOld E false s i = .ok { { (report E (.bothNull i) s) with first := false, isInt := false } with collected := i :: { (report E (.bothNull i) s) with first := false, isInt := false }.collected } ∧ bodyNew E false s i = .ok { { (report E (.bothNull i) s) with first := false, isInt := false } with collected := i :: { (report E (.bothNull i) s) with first := false, isInt := false }.collected } := by
  simp_all [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond]

