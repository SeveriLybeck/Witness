# Witness

Witness is a review page for AI-written pull requests. An SMT solver finds every behaviour change between the old and the new code, Lean proves each one, and a named engineer signs off on what nobody asked for. No verdict comes from an AI.

Built for Junction 2026, Tieto's challenge "Earned Trust in AI Code". `RUN-REPORT.md` has the results on the four sample changes.

## How it works

A pull request goes through eight steps. Only the last one before the page involves a language model, and it decides nothing.

1. **Cut the change.** `crates/cutter` reads `git diff -U0` and uses tree-sitter to cut the changed C# into statement-level parts and blocks. Identical mechanical edits are folded (402 edits in 222 files become one item).
2. **Read the code with the compiler.** `worker/` is a C# process on Roslyn. It binds the project and returns the control-flow graph of every changed method, with each operation's kind and the documentation ID of every symbol, plus references, dependencies and public-API facts.
3. **Translate to a model.** `crates/translator` is our own lowering from a Roslyn control-flow graph to a small logical model (20 operations). It builds one unit per changed site, inlines changed helper members and delegates wired up in constructors, and turns implicit exceptions into explicit branches: indexing an empty list, a failed cast, a call on null. Every changed line it cannot model is counted, so the page can say "28 of 29 lines compared".
4. **Find the differences.** `crates/differ` is a symbolic executor over the old and the new model. It runs one Z3 process per unit, enumerates the feasible paths of both versions, pairs them and keeps each pair that ends differently, with the exact condition. Loop bodies are compared per iteration. Each difference gets a kind from fixed rules: crash fixed, new exception, check turned off, output changed.
5. **Certify with Lean.** `crates/lean` writes one lemma per solver row (`condition → old ends as A ∧ new ends as B`) and one coverage lemma per unit (the rows cover every input). A lemma counts only if `lean` reports no error and no `sorry`, and `#print axioms` shows nothing beyond `propext`, `Quot.sound` and `Classical.choice`. What is assumed about .NET library calls is passed in as named hypotheses and listed on the page.
6. **Read intent from the tests.** `crates/testcontract` runs the pull request's own tests on the old and the new code inside a `bwrap` sandbox, then reverts each block on its own to see which tests depend on it. That tells declared changes from undeclared ones.
7. **Apply fixed rules.** `crates/rules` reads thresholds from one config file and produces the importance of each block, the status of the change and the list of decisions left to a person.
8. **Summarise.** One model call per change compares the ticket with the evidence. Its statements are kept only if every citation resolves to a record. Calls are recorded, so a run can be replayed without a model.

`crates/gateway` serves the records and the page. `crates/sandbox` runs everything from the untrusted repository without network.

The solver (Z3), the proof checker (Lean 4) and the compiler (Roslyn) are existing tools. The translator, the differ, the Lean emitter, the rules and the page are ours. First versions of a Rust front end (through Charon and Aeneas) and a TypeScript front end are in `crates/translator-rust` and `crates/translator-ts`.

## Run it

You need Nix, the challenge's sample repository (MessagePack-CSharp with the `ju26-issue-*` branches) and its issue files. Their locations are set in `integration/lib/env.sh` and `contracts/spike/tools/sb`; change them to where your copies are.

```
nix-shell -p cargo rustc gcc pkg-config z3 lean4 dotnet-sdk_8 nodejs bubblewrap git playwright-driver.browsers

integration/prepare-all.sh        # prepares the four sample changes; model calls are replayed from recordings
integration/lib/build.sh && npm --prefix web ci && npm --prefix web run build
PORT_GATEWAY=9380 PORT_GITHUB=9381 integration/start-page.sh
```

Open http://127.0.0.1:9380/. To sign off, use the token `reviewer-token` (login `reviewer-a`).

`integration/verify.sh` runs every check and takes about 15 minutes.

## Layout

| Folder | What |
|---|---|
| `crates/` | The pipeline: cut the diff into blocks, translate to a model, find differences (Z3), certify (Lean), rules, gateway |
| `worker/` | C# worker on Roslyn: control flow, references, facts |
| `web/` | The review page |
| `contracts/` | Record formats shared by all parts, with fixtures |
| `replays/` | Prepared evidence for the four sample changes |
