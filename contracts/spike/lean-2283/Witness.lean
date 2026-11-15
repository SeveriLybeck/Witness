import Defs
/- A concrete countermodel (the shape an SMT model has), CHECKED BY THE LEAN KERNEL via `decide`.
   Loc := Bool (true = a source location, false = Location.None); Diag := its location;
   the tree of the location is NOT in the compilation (C4a) -/
def W_diagCreate : Option Bool → Bool := fun o => o.getD false
def W_base : Outcome Bool :=
  base (Sym:=Unit) (Ref:=Unit) (Syn:=Unit) (fun _ => 1) (fun _ _ => ()) (fun _ => ()) (fun _ => true) (fun _ => true) W_diagCreate false ()
def W_head : Outcome Bool :=
  head (Sym:=Unit) (Ref:=Unit) (Syn:=Unit) (Tree:=Unit) (Comp:=Unit) (fun _ => 1) (fun _ _ => ()) (fun _ => ()) (fun _ => true) (fun _ => true)
       W_diagCreate id id (fun _ => ()) (fun _ _ => false) () false ()
-- the contracts hold in this model
example : id (W_diagCreate none) = false ∧ id false = false ∧ (∀ l : Bool, id (W_diagCreate (some l)) = l) := by decide
/-- the witness: base reports, head is silent -/
theorem witness_C4 : W_base = .reported true ∧ W_head = .silent := by decide
/-- a WRONG witness must be rejected -/
theorem witness_wrong : W_head = .reported true := by decide
