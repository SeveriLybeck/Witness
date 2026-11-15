#!/usr/bin/env bash
# playground module: <stageDir> <bundleDir> <repo> <old> <new> -> <stageDir>/replays.json (generator-lab replay, D23)
# First pass: bundle input and issue reproduction. Then, for differences still not replayed, model-proposed snippets
# (purpose replay-snippet) from recordings; live calls only when WITNESS_LIVE names the CLI (integration/record.sh).
set -euo pipefail
lab=plugins/generator-lab/target/debug/generator-lab
"$lab" replay "$2" --out "$1/replays.first.json"
live=(); [ -n "${WITNESS_LIVE:-}" ] && live=(--live "$WITNESS_LIVE")
target/debug/witness-snippets --recordings "${WITNESS_RECORDINGS:?}" "${live[@]}" --not-replayed "$1/replays.first.json" "$2" "$1/proposed.json"
"$lab" replay "$2" --proposed "$1/proposed.json" --out "$1/replays.json"
rm -f "$1/replays.first.json"
