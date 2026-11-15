# translator-ts — status

Works: TypeScript compiler API 5.9.3 (`js/package.json`, `js/package-lock.json`) lowers pure functions to the model; the differ reads the models and finds the planted difference in `tests/samples/price.{old,new}.ts` (old throws `RangeError` on `total < 0`, new returns 0: one `crash-fixed` row; same version: no rows). React render causes on `tests/samples/Counter.tsx`. 6 tests.
Test: `nix-shell -p cargo rustc gcc pkg-config z3 nodejs --run 'cargo test -p translator-ts'` (first run does `npm ci` in `js/`, network once).
Run: `cargo run -q -p translator-ts -- lower <file.ts> <function> <old|new> [unit-id] > model.X.json`, then `differ --old model.old.json --new model.new.json ...`; `cargo run -q -p translator-ts -- render <file.tsx>`.

Interface
- `translator_ts::lower(file, function, version, unit) -> Result<Value, String>`: a `model.schema.json` object. Unit id default `<function>#<line of its name>`; pass one id for both versions.
- `translator_ts::render_causes(file)`: `{components:[{component, file, memo, heading:"may re-render when", causes:[{cause, name, line, label}]}]}`, cause `prop|state|context|store|parent`, labels like `prop label changes`, `parent re-renders` (absent under `memo`).
- `ensure_js()` installs the pinned compiler; `js_dir()`.

Lowering (closed list): `return`, `throw new E(..)` (throw name `E`), `if`/`else` (rest of the block goes into each arm), `const` (`let`), blocks; expressions: integer/string/bool literals, `null`/`undefined` (`none`), params and consts (`var` with sort), `?:`, `!`, `&&`, `||`, `===`/`==`/`!==`/`!=` (`eq`, `isnull` against null), arithmetic and comparisons, calls.
Sorts from the checker: number Int, boolean Bool, string String, else Obj; unions of one sort keep it.
Locations are `{file: basename, line, endLine}`; `lines.modelled` = the function's lines minus not-modelled ones.

Deviations and fallbacks (FALLBACKS.md)
- X2: anything outside the closed list is in `notModelled` and stands in as an opaque `unmodelled_<Sort>` call; a statement outside it ends the unit there.
- Operators and calls are opaque (`op:<tok>(sorts)`, `fn:<name>(sorts)`): no arithmetic op in the model, so `>` against `>=` compares as two different uninterpreted functions.
- Render causes are syntactic, not checked against the playground's render counts.
- Node from nix `nodejs` (24.21.0), npm, not bun.
