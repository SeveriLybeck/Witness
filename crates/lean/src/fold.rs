//! Generic fold lemmas, emitted unchanged for every `foreach` (D19). A one-version
//! property of the loop body is checked once and lifted to any number of iterations.

pub const FOLD_LEMMAS: &str = "\
namespace W
inductive Step (ε σ : Type) where
  | ok (s : σ)
  | threw (e : ε) (s : σ)
def throws {ε σ : Type} : Step ε σ → Bool
  | .ok _ => false
  | .threw _ _ => true
def run {ε σ α : Type} (b : σ → α → Step ε σ) : List α → σ → Step ε σ
  | [], s => .ok s
  | i :: is, s => match b s i with
    | .ok s' => run b is s'
    | .threw e s' => .threw e s'
def refines {ε σ τ : Type} (R : σ → τ → Prop) : Step ε σ → Step ε τ → Prop
  | .ok a, .ok b => R a b
  | .threw _ _, _ => True
  | .ok _, .threw _ _ => False
theorem run_nothrow {ε σ α : Type} (b : σ → α → Step ε σ) (h : ∀ s i, throws (b s i) = false) :
    ∀ l s, throws (run b l s) = false := by
  intro l
  induction l with
  | nil => intro s; rfl
  | cons i is ih =>
    intro s
    have hs := h s i
    simp only [run]
    revert hs
    cases b s i <;> simp [throws] <;> exact ih _
theorem run_refines {ε σ τ α : Type} (b1 : σ → α → Step ε σ) (b2 : τ → α → Step ε τ)
    (R : σ → τ → Prop) (h : ∀ s t i, R s t → refines R (b1 s i) (b2 t i)) :
    ∀ l s t, R s t → refines R (run b1 l s) (run b2 l t) := by
  intro l
  induction l with
  | nil => intro s t hr; simpa [run, refines] using hr
  | cons i is ih =>
    intro s t hr
    have hs := h s t i hr
    simp only [run]
    revert hs
    cases b1 s i <;> cases b2 t i <;> simp [refines] <;> intro hr' <;> exact ih _ _ hr'
end W
";

/// The lifted theorem: `binders` are the theorem's parameters, `body` the loop body
/// `σ → α → Step ε σ` applied to its environment, `body_lemma` the per-iteration lemma
/// applied to the same environment (`∀ s i, W.throws (body s i) = false`).
pub fn lift_nothrow(name: &str, binders: &str, body: &str, body_lemma: &str) -> String {
    format!(
        "theorem {name} {binders} :\n    ∀ l s, W.throws (W.run ({body}) l s) = false :=\n  W.run_nothrow _ ({body_lemma})\n#print axioms {name}\n"
    )
}
