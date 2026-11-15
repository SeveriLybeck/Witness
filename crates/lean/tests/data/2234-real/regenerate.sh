#!/bin/sh
# Real 2234 loop unit (D19, unit rule of integration/proof): all changed statements of the
# property `foreach` body as one per-iteration unit, same flags as the proof module
# (`--per-iteration --exact`, old and new over the same closed line range).
# The stored worker replies (crates/translator/tests/data/2234.*.cfg.json) hold GetObjectInfo only,
# so the second loop unit of a live run (the field loop, :1163/:1179) cannot be rebuilt from them.
# usage: sh crates/lean/tests/data/2234-real/regenerate.sh OUTDIR   (repo root, nix-shell with z3 lean4)
set -e
O=$1; mkdir -p $O
T=crates/translator/tests/data
cargo build -q -j 2 -p translator -p differ -p lean
B=target/debug
F=src/MessagePack.SourceGenerator/CodeAnalysis/TypeCollector.cs
U=TypeCollector.GetObjectInfo; ID=$U#1005
OS=1005,1022,1035,1036; NS=1005,1008,1023,1026,1039,1042
$B/translator lower --cfg $T/2234.old.cfg.json --unit $U --version old --site $OS --per-iteration --exact --unit-id $ID > $O/model.old.1005.json
$B/translator lower --cfg $T/2234.new.cfg.json --unit $U --version new --site $NS --per-iteration --exact --unit-id $ID > $O/model.new.1005.json
$B/differ --old $O/model.old.1005.json --new $O/model.new.1005.json --out $O/solver-rows.1005.json \
  --rows $O/rows.1005.json --bridge $O/bridge.1005.json --file $F --changed $NS
$B/lean-certify --old $O/model.old.1005.json --new $O/model.new.1005.json --rows $O/solver-rows.1005.json \
  --bridge $O/bridge.1005.json --out $O/out.1005 || true
