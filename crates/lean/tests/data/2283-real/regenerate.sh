#!/bin/sh
# Real 2283 units from the stored worker replies (crates/translator/tests/data/2283.*.cfg.json).
# Unit rule of integration/proof: the whole method GetObjectInfo does not lower from these trimmed
# replies (`missing block 0`) and :985/:1077 have no enclosing foreach in them, so the rule's
# per-site fallback applies: one unit per site, helpers and the :239 delegate inlined.
# New-only units: the two new helpers lowered alone (`differ --new-only`, old = nothing).
# usage: sh crates/lean/tests/data/2283-real/regenerate.sh OUTDIR   (repo root, nix-shell with z3 lean4 jq)
set -e
O=$1; mkdir -p $O
T=crates/translator/tests/data
cargo build -q -j 2 -p translator -p differ -p lean
B=target/debug
F=src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs
OC=239,985,1077
NC=239,985,1077,1429,1430,1431,1432,1433,1434,1435,1436,1437,1438,1439,1440,1441,1442,1443,1444,1445,1446,1447,1448,1449,1450,1451
IN="--inline GetPropertyIdentifierLocation --inline ReportIfLocatedInCompilation"
for site in 985 1077; do
  $B/translator lower --cfg $T/2283.old.cfg.json --unit TypeCollector.GetObjectInfo --version old --site $site $IN \
    --derive-delegates $OC --part 239 --part 985 --part 1077 > $O/model.old.$site.json
  $B/translator lower --cfg $T/2283.new.cfg.json --unit TypeCollector.GetObjectInfo --version new --site $site $IN \
    --derive-delegates $NC --part 239 --part 985 --part 1077 --part 1429,1430,1431,1432,1433,1434,1435 \
    --part 1436,1437,1438,1439,1440,1441,1442,1443,1444,1445,1446,1447,1448,1449,1450,1451 > $O/model.new.$site.json
  $B/differ --old $O/model.old.$site.json --new $O/model.new.$site.json --out $O/solver-rows.$site.json \
    --rows $O/rows.$site.json --bridge $O/bridge.$site.json --scope-in contracts/fixtures/2283/scope.json --scope-out $O/scope.$site.json \
    --file $F --changed $NC
  $B/lean-certify --old $O/model.old.$site.json --new $O/model.new.$site.json --rows $O/solver-rows.$site.json \
    --bridge $O/bridge.$site.json --out $O/out.$site || true
done
for m in GetPropertyIdentifierLocation ReportIfLocatedInCompilation; do
  $B/translator lower --cfg $T/2283.new.cfg.json --unit TypeCollector.$m --version new --unit-id TypeCollector.$m > $O/model.new.$m.json
  jq '{provenance:"pipeline",unit:.unit,version:"old",params:.params,opaque:[],contracts:[],body:{op:"nothing"},lines:{modelled:[],notModelled:[]}}' \
    $O/model.new.$m.json > $O/model.old.$m.json
  $B/differ --new-only --old $O/model.old.$m.json --new $O/model.new.$m.json --out $O/solver-rows.$m.json \
    --rows $O/rows.$m.json --bridge $O/bridge.$m.json --file $F --changed $NC
  $B/lean-certify --old $O/model.old.$m.json --new $O/model.new.$m.json --rows $O/solver-rows.$m.json \
    --bridge $O/bridge.$m.json --out $O/out.$m || true
done
