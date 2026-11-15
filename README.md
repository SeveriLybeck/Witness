# Witness

Witness is a review page for AI-written pull requests. An SMT solver finds every behaviour change between the old and the new code, Lean proves each one, and a named engineer signs off on what nobody asked for. No verdict comes from an AI.

Built for Junction 2026, Tieto's challenge "Earned Trust in AI Code". `RUN-REPORT.md` has the results on the four sample changes.

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
