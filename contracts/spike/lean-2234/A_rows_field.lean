import Defs
set_option linter.unusedSimpArgs false
set_option linter.unusedVariables false
open M2234
variable {IK SK I : Type} [DecidableEq IK] [DecidableEq SK]


-- row 0 [same]
theorem row_0 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E true s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } ∧ bodyNew E true s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 1 [same]
theorem row_1 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E true s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } ∧ bodyNew E true s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 2 [DIFF]
theorem row_2 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E true s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E true s i = .ok { report E (.dup i) s with hidden := s.hidden + 1 } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 3 [DIFF]
theorem row_3 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = some k2) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E true s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E true s i = .ok { { report E (.dup i) s with hidden := s.hidden + 1 } with collected := i :: { report E (.dup i) s with hidden := s.hidden + 1 }.collected } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 4 [DIFF]
theorem row_4 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h1 : E.sk i = none) (h5 : s.first = false) (h6 : s.isInt = false) :
    bodyOld E true s i = .threw .argNull (report E (.mix i) s) ∧ bodyNew E true s i = .ok (report E (.mix i) s) := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 5 [same]
theorem row_5 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = false) :
    bodyOld E true s i = .ok { s with intM := (k1, i) :: s.intM } ∧ bodyNew E true s i = .ok { s with intM := (k1, i) :: s.intM } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 6 [same]
theorem row_6 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = true) :
    bodyOld E true s i = .ok { { s with intM := (k1, i) :: s.intM } with collected := i :: { s with intM := (k1, i) :: s.intM }.collected } ∧ bodyNew E true s i = .ok { { s with intM := (k1, i) :: s.intM } with collected := i :: { s with intM := (k1, i) :: s.intM }.collected } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 7 [DIFF]
theorem row_7 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = false) :
    bodyOld E true s i = .threw .argDup (report E (.dup i) s) ∧ bodyNew E true s i = .ok (report E (.dup i) s) := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 8 [DIFF]
theorem row_8 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h5 : s.first = false) (h6 : s.isInt = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = true) :
    bodyOld E true s i = .threw .argDup (report E (.dup i) s) ∧ bodyNew E true s i = .ok { (report E (.dup i) s) with collected := i :: (report E (.dup i) s).collected } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 9 [same]
theorem row_9 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h5 : s.first = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = false) :
    bodyOld E true s i = .ok { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } ∧ bodyNew E true s i = .ok { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 10 [same]
theorem row_10 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h5 : s.first = true) (h7 : has s.intM k1 = false) (h9 : E.fmtNull i = true) :
    bodyOld E true s i = .ok { { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } with collected := i :: { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM }.collected } ∧ bodyNew E true s i = .ok { { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM } with collected := i :: { { s with first := false, isInt := true } with intM := (k1, i) :: { s with first := false, isInt := true }.intM }.collected } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 11 [DIFF]
theorem row_11 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h5 : s.first = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = false) :
    bodyOld E true s i = .threw .argDup (report E (.dup i) { s with first := false, isInt := true }) ∧ bodyNew E true s i = .ok (report E (.dup i) { s with first := false, isInt := true }) := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 12 [DIFF]
theorem row_12 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = some k1) (h5 : s.first = true) (h7 : has s.intM k1 = true) (h9 : E.fmtNull i = true) :
    bodyOld E true s i = .threw .argDup (report E (.dup i) { s with first := false, isInt := true }) ∧ bodyNew E true s i = .ok { (report E (.dup i) { s with first := false, isInt := true }) with collected := i :: (report E (.dup i) { s with first := false, isInt := true }).collected } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 13 [same]
theorem row_13 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E true s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } ∧ bodyNew E true s i = .ok { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 14 [same]
theorem row_14 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E true s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } ∧ bodyNew E true s i = .ok { { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 } with collected := i :: { s with strM := (k2, i) :: s.strM, hidden := s.hidden + 1 }.collected } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 15 [DIFF]
theorem row_15 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E true s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E true s i = .ok { report E (.dup i) s with hidden := s.hidden + 1 } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 16 [DIFF]
theorem row_16 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h5 : s.first = false) (h6 : s.isInt = false) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E true s i = .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 } ∧ bodyNew E true s i = .ok { { report E (.dup i) s with hidden := s.hidden + 1 } with collected := i :: { report E (.dup i) s with hidden := s.hidden + 1 }.collected } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 17 [DIFF]
theorem row_17 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h5 : s.first = false) (h6 : s.isInt = true) :
    bodyOld E true s i = .threw .invalidOp (report E (.mix i) s) ∧ bodyNew E true s i = .ok (report E (.mix i) s) := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 18 [same]
theorem row_18 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h5 : s.first = true) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = false) :
    bodyOld E true s i = .ok { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E true s i = .ok { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 19 [same]
theorem row_19 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h5 : s.first = true) (h8 : has s.strM k2 = false) (h9 : E.fmtNull i = true) :
    bodyOld E true s i = .ok { { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } with collected := i :: { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 }.collected } ∧ bodyNew E true s i = .ok { { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 } with collected := i :: { { s with first := false, isInt := false } with strM := (k2, i) :: { s with first := false, isInt := false }.strM, hidden := { s with first := false, isInt := false }.hidden + 1 }.collected } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 20 [DIFF]
theorem row_20 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h5 : s.first = true) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = false) :
    bodyOld E true s i = .threw .argDup { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E true s i = .ok { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 21 [DIFF]
theorem row_21 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = some k2) (h5 : s.first = true) (h8 : has s.strM k2 = true) (h9 : E.fmtNull i = true) :
    bodyOld E true s i = .threw .argDup { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } ∧ bodyNew E true s i = .ok { { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 } with collected := i :: { report E (.dup i) { s with first := false, isInt := false } with hidden := { s with first := false, isInt := false }.hidden + 1 }.collected } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 22 [DIFF]
theorem row_22 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h5 : s.first = false) (h6 : s.isInt = false) :
    bodyOld E true s i = .threw .argNull (report E (.mix i) (report E (.bothNull i) s)) ∧ bodyNew E true s i = .ok (report E (.mix i) (report E (.bothNull i) s)) := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 23 [DIFF]
theorem row_23 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h5 : s.first = false) (h6 : s.isInt = true) :
    bodyOld E true s i = .threw .invalidOp (report E (.mix i) (report E (.bothNull i) s)) ∧ bodyNew E true s i = .ok (report E (.mix i) (report E (.bothNull i) s)) := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

-- row 24 [same]
theorem row_24 (E : Env IK SK I) (s : St IK SK I) (i : I) (k1 : IK) (k2 : SK) (h0 : E.ik i = none) (h1 : E.sk i = none) (h5 : s.first = true) :
    bodyOld E true s i = .threw .argNull { (report E (.bothNull i) s) with first := false, isInt := false } ∧ bodyNew E true s i = .threw .argNull { (report E (.bothNull i) s) with first := false, isInt := false } := by
  simp [bodyOld, bodyNew, aotOld, aotNew, midOld, midNew, tailOld, tailNew, addIntNew, addStrNew, fin, report, mixCond, *]

