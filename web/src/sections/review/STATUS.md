# review — status (group 18: AI review, Decide, Sign off)

Works: AI review under the four headings with citation chips (statement click follows its first cite), page-side re-check of cites and banned words, dropped lines struck through with their cites as chips, `no AI review` without a recording, sections 1–3 fold when light and nothing unexpected, cited marks on other sections, `a` / arrow keys. `Left to you` has one row per question (AI words when a statement names that question alone; multi-question statements stay as statements). Decide rows: question text per rule (`Accept undeclared difference 2?`), terse rule label (`architectural:changed-wiring` → `Changed wiring`), block, `File.cs:line`, Accept/Return, `<n> open`. Sign off: spot-check first row on every load (local pick until a session stores it), gate rows in spec order with block names, gateway refusal grouped per gate, composed body after submit. 23 tests.
Real browser (Playwright, nix chromium, gateway on the four `replays/<change>/bundle`, 1280×900 and 390×844): no element of the review, decide or sign-off slots extends past the viewport; Left to you = Decide rows (2283: 6, 2234: 11, 2297: 2, 2129: 29); spot-check row present; a test citation lands with the row top in view. The remaining page `scrollWidth` at 390 (2283 496, 2234 535, 2297 1107, 2129 1053) comes from `.proof-table` and the header, not this module.
Run: `nix-shell -p nodejs --run 'npm --prefix web ci && npm --prefix web test -- src/sections/review'`.

Interface
- `index.tsx` registers `review`, `decide`, `signoff`. No headlines (counts live in the bodies, see below).
- Follow targets, tried in order inside `#slot-<slot>`: difference `tr[data-diff=dN]`, `[data-row=dN]`, `[data-difference=dN]` or `[data-cite~="diff:N"]` (proof); block `[data-node=bN]` or `[data-cite~="block:bN"]` (map); test `[data-test=name]`/`[data-cite~="test:name"]`, else code panel at the test; category `[data-cite~="tests:<cat>"]`; preset `[data-cite~="preset:dN"]`/`[data-preset=dN]` (playground, plus `selectDifference`); reproduction `[data-cite~="reproduction"]` (clicked when a button); `p<n>` and `File.cs:line` open the code panel. Targets get focus and `data-flash`; cited ones get `data-ai-cited`.
- Session: `GET/POST /api/sessions/<key>/<reviewer>` through `source.request` (`answer`, `opened`, `spotCheck`, `shown`); reviewer name from the Sign off field, kept in `localStorage` `witness.reviewer`. The map focus (`selection.block`) is posted as `opened`, parts the code panel showed (`shownParts`) as `shown`. Unseen spot-check pick kept in `localStorage` `witness.spot.<key>`.
- Cite spellings exactly as `crates/model`: `d<n> b<n> q<n> p<n>`, test name, test category, `preset:d<n>`, `reproduction`, `File.cs:line`/`path:line`.

Deviations and fallbacks
- Submit goes through `source.request` with the `authorization` header; a thrown `{status, data}` is shown as the refusal.
- Gate rows and `<n> open` are a display computed on the page; the gateway alone enforces (approve disabled only while a section is `running`, as the task asks). Composed body is shown only after a submit, since the API has no preview endpoint.
- `Decide` and `Sign off` counts are in the section body, not `headline(data)`: the headline gets only bundle data and would not update with the session.
- The 2283 fixture's `Not in the description` cites d1+d2 and its dropped line cites d9; tests follow the fixture, not the spec's "2 and 3" / "4" wording.
- Spec heading texts exceed the three-word label limit; kept verbatim, exempted in the wording test.
- Cited marks are applied by a MutationObserver on the DOM, since the shell has no cited-items channel.
