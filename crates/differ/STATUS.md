# differ: status

Works: symbolic executor over the model (all 20 ops), one `z3 -in` per unit, row pairing (identical rows kept, row id null), D21 kinds, D18 properties, per-iteration loops, grouping, interpreter, mutation self-checks, Lean bridge, `rows.json`, scope fill. 20 tests.
Test: `nix-shell -p cargo rustc gcc pkg-config z3 --run 'cargo test -j 2 -p differ'` (add `lean4` to also run lean-certify on the bridge; skipped without `lean`).
Run, one unit:
`differ --old model.old.json --new model.new.json --out solver-rows.json --rows rows.json --bridge bridge.json --scope-in scope.json --scope-out scope.json [--detail detail.json] [--first-row N] [--first-difference N] [--file <unit file>] [--changed <l,l,…>] [--disable <contract>]… [--no-dependency]`
then `lean-certify --old model.old.json --new model.new.json --rows solver-rows.json --bridge bridge.json --out DIR`.
2283 contract models: 3 differing rows (r1–r3, 3 groups), all `checked` by Lean with this bridge (6 of 6 rows), coverage `checked`. Each array term under the FirstOrDefault fact gets a bridge fact `(empty ∧ fod = none) ∨ (¬empty ∧ fod = some a[0])`, the same fact z3 is given.

Interface
- `diff_models(&Model, &Model, &Options) -> UnitResult`; `solver_rows_json` (`solver-rows`); `rows_record(&[&UnitResult], first_difference)` (`rows` with `perIteration` and `property` per row, plus `kind`, `literals`; pinned on real 2234 `#1022` per-iteration units: 4 rows, all per iteration); `fill_scope(&mut scope, units)`; `bridge::bridge_json(old, new, &UnitResult)`; `detail_json`.
- `group(rows)` keys on (kind, old outcome tag and text, new outcome tag and text, firstChanged); `rows.json` `difference` carries that grouping; `kind_of(old, new, property, nullReceiver, publicApi)`; `throwing_paths(&Model, disabled)`; `interp::run(model, inputs, &mut dyn Oracle, disabled)`.
- Ids: rows `r<first_row>`…, differences `d<first-difference>`… in group order. Over several units, pass running offsets.
- firstChanged is always a changed line of the new side: `--changed` when given (integration should pass the new-side changed lines of the unit's file; model `lines` can hold context lines), else the new model's `lines`. In order: the old throw site, the last located decision on the new path, a location reached on the new path, the site `#N` of the unit id, the first changed line. Tested on the real 2283 replies via the translator crate (dev-dependency). A row with no such location, or one off the changed lines, gets the unit site: the file from a model location, else `--file <path>`, and the line `#N` if changed, else the first changed line. With neither file nor changed line, the run fails naming the unit; no placeholder is written.

Deviations and fallbacks (playbook D1–D3, D6 in `FALLBACKS.md`)
- Reference types share one sort `Obj` with `w_null`; helpers `w_len`, `w_at`, `w_is_<Type>`, `w_contains_*`, `w_val_*`.
- Effect outcomes read `calls Type.Method` from the opaque's symbol (`calls List.Add`), never the generated name.
- A throwing path's text carries its earlier reports: `reports <name>, throws <Exception>`; tag stays `throws` (tags and kinds unchanged). Reports were already part of the comparison.
- `value` is read as `Nullable.Value` (with `some`/`none`), not as a return.
- Library facts `exec::FACTS` (`Fact {key, symbol, text}`): length-non-negative, first-or-default, located-null-false, isinsource-null-false, dsr-elements-non-null, getsyntax-non-null; readings `exec::READINGS` (ContainsKey, `T:` type tests). Each row records the facts on its paths (`Row::facts`); `library_facts(&UnitResult)` and `detail.json` `libraryFacts` list each with its rows. `--scope-in/--scope-out` add one `{kind: "library-fact", key, text, rows}` entry per fact a row rests on; `Fact::smt` is the asserted template. Bridge facts: `∀ c, located c none = false` and the FirstOrDefault split per array term.
- Bridge: a literal with no faithful Lean form (null of a sort Lean has no null for, a disjunction) is left out, so its rows stay `not-checked`. A non-null literal implied by the facts maps to no proposition.
- A row's assumptions are the contracts whose throwing case it takes. `no-exception` is set only on differing rows where new throws. One-version check: `throwing_paths`.
- `no-new-dependency` only for inputs both models declare; new-only inputs (2283 `comp`) go to `newInputs`.
- The fixture model covers only `:985` (2 differences). Three differences are shown on a hand-lowered two-site test model in `tests/support`, which is test data, never output. 2234 is also a hand-lowered test model.
- With `index-empty` disabled, the `failed-cast` row stays crash-fixed (the fixture lists both). The `Length > 0 → > 1` mutant has no model op, so `[0] → [1]` replaces it.
- `unknown`: the row stays differing with `undecided: true`. No cvc5 retry.

Outcomes, conditions, new code
- Outcome text lists every observable thing of the path in order: `reports A, B`, `calls x.M ×n` (receiver name when it is a plain input), `skips the rest of the iteration` / `leaves the loop` (translator `region:skip`, `region:leave`), `returns v` (cut at 100 characters), `throws E`, then `without x.M` for each effect the other version's path performs or attempts (the call that threw) and this one does not. A lone throw or lone reports keep the short form (`ArgumentException`, `MsgPack005`). Tags are unchanged (throw, reports, value or effect, nothing). Equal texts on a differing row get `, with other values` on the new side.
- `rows.json` `condition` is in the source's terms (`read.rs`): the `syntax` of each branch test (`X is null`, negations flipped), a phrase per contract (`a[0] is out of range`, `the key is already present at d.Add(…)`), else the solver literal printed through the opaque symbols (`item.DeclaringSyntaxReferences is empty`). `literals` keep the solver text.
- `carried` on a closing `nothing` is evaluated and compared like event arguments (no event, no tag change).
- Event shapes include argument sorts (an `Add` with an Int key never pairs with one with a String key).
- `--new-only` (`Options::new_only`): new code against an empty old model; only throwing paths get row ids (`property: no-exception`, old text `not present`), the rest are unnumbered rows for coverage.
