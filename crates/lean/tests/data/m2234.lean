-- Test data: the 2234 spike model (research/spikes/src/lean-2234/Defs.lean) with its own run and fold lemmas
-- removed and its outcome type replaced by the emitted W.Step. Not pipeline output.
/-! Hand-written model of TypeCollector.GetObjectInfo loop bodies, change 2234.
    Region: from `var intKey = ...` to `CollectCore`, property loop (field := false)
    and field loop (field := true). Old = 32258fdb, New = origin/ju26-issue-2234.
    Library calls are opaque (fields of Env). Dictionaries are association lists. -/
namespace M2234

inductive Exc | invalidOp | argNull | argDup
  deriving DecidableEq, Repr

inductive Diag (I : Type) | bothNull (i : I) | aot (i : I) | mix (i : I) | dup (i : I)
  deriving DecidableEq, Repr

structure Env (IK SK I : Type) where
  ik : I → Option IK        -- key is { Value: int }
  sk : I → Option SK        -- key is { Value: string }
  initOnly : I → Bool
  hasInit : I → Bool        -- initializer is not null (opaque Roslyn query)
  fmtNull : I → Bool        -- messagePackFormatter == null
  rep : Bool                -- this.reportDiagnostic != null
  suppress : Bool           -- suppressSourceGeneration is true

structure St (IK SK I : Type) where
  first : Bool
  isInt : Bool
  intM : List (IK × I)
  strM : List (SK × I)
  diags : List (Diag I)
  hidden : Nat
  collected : List I        -- CollectCore calls, most recent first
  deriving DecidableEq, Repr

abbrev Out (σ : Type) := W.Step Exc σ

variable {IK SK I : Type} [DecidableEq IK] [DecidableEq SK]

def has {K V : Type} [DecidableEq K] (m : List (K × V)) (k : K) : Bool :=
  m.any (fun p => decide (p.1 = k))

/-- reportDiagnostic?.Invoke: `emit` is opaque (null delegate => implementation-defined, unchanged by the PR) -/
opaque emit {I : Type} (rep : Bool) (d : Diag I) (ds : List (Diag I)) : List (Diag I)

def report (E : Env IK SK I) (d : Diag I) (s : St IK SK I) : St IK SK I :=
  { s with diags := emit E.rep d s.diags }

/-- state after the unchanged prefix; used only in theorem statements -/
def preState (E : Env IK SK I) (field : Bool) (s : St IK SK I) (i : I) : St IK SK I :=
  let s1 := if (E.ik i).isNone && (E.sk i).isNone then report E (.bothNull i) s else s
  if !field && !E.suppress && E.initOnly i && E.hasInit i then report E (.aot i) s1 else s1

/-! Basic blocks in continuation style (one def per block, as a CFG translator emits). -/

/-- `if (messagePackFormatter == null) CollectCore(...)`, end of body -/
def fin (E : Env IK SK I) (i : I) (s : St IK SK I) : Out (St IK SK I) :=
  if E.fmtNull i then .ok { s with collected := i :: s.collected } else .ok s

def mixCond (E : Env IK SK I) (s : St IK SK I) (i : I) : Bool :=
  (s.isInt && (E.ik i).isNone) || (!s.isInt && (E.sk i).isNone)

/-- OLD: `if (isIntKey) {...} else if (stringKey is not null) {...}` (field loop: plain `else`) -/
def tailOld (E : Env IK SK I) (field : Bool) (s : St IK SK I) (i : I) : Out (St IK SK I) :=
  if s.isInt then
    match E.ik i with
    | none => .threw .invalidOp s                                      -- intKey!.Value on null
    | some k =>
      if has s.intM k then .threw .argDup (report E (.dup i) s)       -- report, then Add throws
      else fin E i { s with intM := (k, i) :: s.intM }
  else
    match E.sk i with
    | none => if field then .threw .argNull s else fin E i s          -- ContainsKey(null)
    | some k =>
      if has s.strM k then .threw .argDup { report E (.dup i) s with hidden := s.hidden + 1 }
      else fin E i { s with strM := (k, i) :: s.strM, hidden := s.hidden + 1 }

def midOld (E : Env IK SK I) (field : Bool) (s : St IK SK I) (i : I) : Out (St IK SK I) :=
  if s.first then tailOld E field { s with first := false, isInt := (E.ik i).isSome } i
  else if mixCond E s i then tailOld E field (report E (.mix i) s) i
  else tailOld E field s i

/-- unchanged prefix: AOTInitProperty check (property loop only) -/
def aotOld (E : Env IK SK I) (field : Bool) (s : St IK SK I) (i : I) : Out (St IK SK I) :=
  if !field && !E.suppress && E.initOnly i && E.hasInit i then midOld E field (report E (.aot i) s) i
  else midOld E field s i

def bodyOld (E : Env IK SK I) (field : Bool) (s : St IK SK I) (i : I) : Out (St IK SK I) :=
  if (E.ik i).isNone && (E.sk i).isNone then aotOld E field (report E (.bothNull i) s) i
  else aotOld E field s i

/-- NEW: `if (ContainsKey) report; ...; if (!ContainsKey) Add` -/
def addIntNew (E : Env IK SK I) (k : IK) (s : St IK SK I) (i : I) : Out (St IK SK I) :=
  if !(has s.intM k) then fin E i { s with intM := (k, i) :: s.intM } else fin E i s

def addStrNew (E : Env IK SK I) (k : SK) (s : St IK SK I) (i : I) : Out (St IK SK I) :=
  if !(has s.strM k) then fin E i { s with strM := (k, i) :: s.strM } else fin E i s

def tailNew (E : Env IK SK I) (field : Bool) (s : St IK SK I) (i : I) : Out (St IK SK I) :=
  if s.isInt then
    match E.ik i with
    | none => .threw .invalidOp s
    | some k =>
      if has s.intM k then addIntNew E k (report E (.dup i) s) i else addIntNew E k s i
  else
    match E.sk i with
    | none => if field then .threw .argNull s else fin E i s
    | some k =>
      if has s.strM k then addStrNew E k { report E (.dup i) s with hidden := s.hidden + 1 } i
      else addStrNew E k { s with hidden := s.hidden + 1 } i

/-- NEW: mixed-key diagnostic followed by `continue` -/
def midNew (E : Env IK SK I) (field : Bool) (s : St IK SK I) (i : I) : Out (St IK SK I) :=
  if s.first then tailNew E field { s with first := false, isInt := (E.ik i).isSome } i
  else if mixCond E s i then .ok (report E (.mix i) s)
  else tailNew E field s i

def aotNew (E : Env IK SK I) (field : Bool) (s : St IK SK I) (i : I) : Out (St IK SK I) :=
  if !field && !E.suppress && E.initOnly i && E.hasInit i then midNew E field (report E (.aot i) s) i
  else midNew E field s i

def bodyNew (E : Env IK SK I) (field : Bool) (s : St IK SK I) (i : I) : Out (St IK SK I) :=
  if (E.ik i).isNone && (E.sk i).isNone then aotNew E field (report E (.bothNull i) s) i
  else aotNew E field s i

def Rel (s t : St IK SK I) : Prop :=
  s.first = t.first ∧ s.isInt = t.isInt ∧ s.intM = t.intM ∧ s.strM = t.strM ∧
  s.diags = t.diags ∧ s.hidden = t.hidden ∧ t.collected.Sublist s.collected
/-- THE fixed proof script, used unchanged for every per-iteration and unrolled theorem -/
macro "fixed_script" : tactic => `(tactic| (
  intros
  repeat' (first
    | split
    | unfold bodyOld | unfold bodyNew | unfold aotOld | unfold aotNew | unfold midOld | unfold midNew
    | unfold tailOld | unfold tailNew | unfold addIntNew | unfold addStrNew | unfold fin
    | unfold preState | unfold W.run)
  all_goals (try simp_all [W.throws, W.refines, Rel, report, mixCond])))

end M2234
