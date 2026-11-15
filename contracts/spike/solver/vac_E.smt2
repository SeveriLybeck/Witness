; evidence: unit U2  base=32258fdb  head=ju26-issue-2283  file=TypeCollector.cs  stmt@985
(set-option :produce-models true)
(declare-sort Obj 0)
(declare-const null Obj)
(declare-const LocationNone Obj)
(declare-datatypes ((Exn 0)) (((IndexOutOfRange) (InvalidCast))))
(declare-datatypes ((Outcome 0)) (((Silent) (Reported (diag Obj)) (Threw (exn Exn)))))

; ---- inputs shared by both versions
(declare-const item Obj) (declare-const comp Obj)
(declare-const D Obj) (declare-const a0 Obj) (declare-const a1 Obj)
(declare-const reportIsNull Bool)

; ---- havoc'd pure library members, shared by both versions
(declare-fun DSR_len (Obj) Int)
(declare-fun DSR_at (Obj Int) Obj)
(declare-fun GetSyntax (Obj) Obj)
(declare-fun IsPDS (Obj) Bool)
(declare-fun Identifier (Obj) Obj)
(declare-fun GetLocation (Obj) Obj)
(declare-fun DiagCreate (Obj Obj Obj Obj) Obj)
(declare-fun Diag_Location (Obj) Obj)
(declare-fun IsInSource (Obj) Bool)
(declare-fun SourceTree (Obj) Obj)
(declare-fun ContainsSyntaxTree (Obj Obj) Bool)

; ---- BASE
(define-fun sB   () Obj (GetSyntax (DSR_at item 0)))
(define-fun locB () Obj (GetLocation (Identifier sB)))
(define-fun dB   () Obj (DiagCreate D locB a0 a1))
(define-fun base () Outcome
  (ite reportIsNull Silent
  (ite (<= (DSR_len item) 0) (Threw IndexOutOfRange)
  (ite (not (IsPDS sB)) (Threw InvalidCast)
       (Reported dB)))))

; ---- HEAD
(define-fun rH   () Obj (ite (> (DSR_len item) 0) (DSR_at item 0) null))
(define-fun sH   () Obj (ite (= rH null) null (GetSyntax rH)))
(define-fun pH   () Obj (ite (and (distinct sH null) (IsPDS sH)) sH null))
(define-fun locH () Obj (ite (= pH null) null (GetLocation (Identifier pH))))
(define-fun dH   () Obj (DiagCreate D locH a0 a1))
(define-fun passH () Bool (and (IsInSource (Diag_Location dH))
                               (ContainsSyntaxTree comp (SourceTree (Diag_Location dH)))))
(define-fun head () Outcome
  (ite reportIsNull Silent (ite passH (Reported dH) Silent)))

; ---- assumptions A2 and contracts K2..K4, instantiated on the terms that occur (keeps it quantifier-free)
(assert (>= (DSR_len item) 0))
(assert (=> (> (DSR_len item) 0) (distinct (DSR_at item 0) null)))              ; array elements non-null
(assert (=> (> (DSR_len item) 0) (distinct sB null)))                           ; A2 GetSyntax
(assert (distinct locB null))                                                   ; A2 GetLocation
(assert (= (Diag_Location dB) (ite (= locB null) LocationNone locB)))           ; K3
(assert (= (Diag_Location dH) (ite (= locH null) LocationNone locH)))           ; K3
(assert (not (IsInSource LocationNone)))                                        ; K4

; E  generator path                                                    expected: unsat  => IDENTICAL
(push) (assert reportIsNull) (check-sat) (pop)
