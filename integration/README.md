# Integration

Shell for every command below:

    nix-shell -p cargo rustc gcc pkg-config z3 lean4 dotnet-sdk_8 nodejs bubblewrap git playwright-driver.browsers

Record once (the only step with live model calls: AI review, replay snippets; commit `integration/recordings/`):

    integration/record.sh 2283 2234 2297 2129

Prepare from recordings (bundles in `work/int/data/bundles/<old>-<new>/`, logs in `work/int/logs/`):

    integration/prepare-all.sh                # or: integration/prepare-all.sh 2283

Start the page (GitHub stand-in, gateway with web/dist, plugin host and generator-lab):

    integration/lib/build.sh && npm --prefix web ci && npm --prefix web run build
    PORT_GATEWAY=9380 PORT_GITHUB=9381 integration/start-page.sh     # http://127.0.0.1:9380/ , token reviewer-token

Verify (replay only; prints `WORKS <change>` / `FAILS <change>: <step>`, writes `RESULTS.md`):

    integration/verify.sh                     # group verifies first, then all four
    integration/verify.sh 2283                # one change

Pipeline (`integration/modules.json` for `witness prepare`): cutter with the sandboxed C# worker (map),
`witness-proof` (worker cfg → translator → differ → lean-certify; source `integration/proof/`), generator-lab replay
plus model snippets (`integration/snippets/`), testcontract, rules, model-review.
