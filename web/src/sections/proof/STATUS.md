# proof — status (group 15)

Works: scope line (modelled of changed production lines, assumptions, not-modelled ranges, Lean sheet), difference rows (number, kind, condition, old → new, location, test, check), headline with the count Lean did not check, tests list by category with verbatim before and after. 9 tests over all four fixtures: 2283 shows 29 changed lines and "3 differences"; 2234 shows 4 declared, 4 hanging, 2 not run; a not-checked Lean row reads "found by the solver, not checked by Lean"; no banned word in any render or label.
Run: `nix-shell -p nodejs --run 'npm --prefix web ci && npm --prefix web test -- src/sections/proof'`.

Interface
- `index.tsx` exports `sections = [{ slot: 'proof', Component: Proof, headline }]`, plus `ScopeLine`, `DifferenceRows`, `TestsList`, `headline(data)`.
- `model.ts`: `checkOf(data, diff)`, `notLeanCount(data)`, `testsFor(data, diff)`, `beforeAfter(test)`, record readers.
- `labels.ts`: copy deck (`kindLabels`, `checkLabels`, `categoryLabels`, `proofLabels`, `allProofLabels()`).
- Rows: `tr[data-diff=<id>][data-row=<id>]`, `aria-selected` follows `selection.difference`; clicking a row calls `selectDifference`. Check cell `td[data-check]`.
- Tests: `[data-testid=proof-tests] section[data-category=<cat>] .proof-test[data-test=<name>]`.
- CSS in `proof.css`, injected once as `style#witness-proof-css`.

Deviations and fallbacks
- Headline is "N differences", with " · K not checked by Lean" only when K > 0.
- "Not checked by Lean" counts every difference whose rows are not all `checked`/`model-checked`, file and API comparisons included; a solver row with no Lean entry counts as not checked.
- Test column: tests linked to the difference's block (`links.json` is per block, D27); first name plus "+n"; full list in the cell title.
- Before/after: declared and new-api → old-code failure message / assertion; an edited assertion → its old and new text; others show only the assertion.
- Lean sheet: per difference number its wording, file name secondary; per unit the coverage wording, loop result (per iteration / any length) and a counted, expandable list of facts from `lean.json` `facts`/`loops`. Lean source text not shown (not in the bundle).
- Condition: `readableCondition`, else `conditionText`, else `condition` when it has no solver syntax; otherwise a "solver terms" button opens the full text in a sheet.
- Scope line: not-modelled ranges deduped by file:line; separators only between items.
- Real browser (Chromium 1217 via playwright-core, `vite build` + a static /api over `replays/<change>/bundle`), 1280×900 and 390×844, all four changes: table width = container (1152 / 358 px), nothing in `#slot-proof` past the viewport, no solver syntax in rows. Page-level overflow at 390 comes from other sections.
- Tests list shows the `tests` section state beside its heading when not ready.
