#!/usr/bin/env bash
# usage (repo root, inside the verify nix-shell): integration/prepare-all.sh [change...]   (default 2283 2234 2297 2129)
# Prepares each change from real runs through `witness prepare --wait` with integration/modules.json.
# Bundles land in work/int/data/bundles/<old>-<new>/; logs in work/int/logs/<change>.log.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
source integration/lib/env.sh
changes=("$@"); [ ${#changes[@]} -gt 0 ] || changes=(2283 2234 2297 2129)
mkdir -p "$WORK/logs" "$WORK/data"

integration/lib/build.sh
integration/lib/setup.sh "${changes[@]}"

pr_of() { case "$1" in 2283) echo 1;; 2234) echo 2;; 2297) echo 3;; 2129) echo 4;; esac; }
export WITNESS_WORKER="$ROOT/contracts/spike/tools/sb --root $SB nonet dotnet $WORKER_DLL"
export WITNESS_RECORDINGS="$INT/recordings"
export GENERATOR_LAB_TREES="$SB/wt/{commit}" GENERATOR_LAB_WORKER="$WORKER_DLL" GENERATOR_LAB_BUNDLES="$WORK/data/bundles"
export NUGET_PACKAGES="$SB_NUGET" SB_NUGET
for ch in "${changes[@]}"; do
  new="$(new_commit "$ch")"
  export WITNESS_WT_OLD="$SB/wt/$OLD" WITNESS_WT_NEW="$SB/wt/$new"
  export GENERATOR_LAB_REVERTED="$ROOT/work/sandbox/testcontract-${new:0:12}/testcontract/rev/{new12}/{block}"
  rm -rf "$WORK/data/bundles/$OLD-$new"
  echo "prepare $ch $new" >&2
  if target/debug/witness prepare --data "$WORK/data" --modules integration/modules.json --issues "$ISSUES" --pr "$(pr_of "$ch")" \
       --wait "$SB/repo" "$OLD" "$new" > "$WORK/logs/$ch.log" 2>&1; then
    echo "prepared $ch $WORK/data/bundles/$OLD-$new" >&2
  else
    echo "not prepared $ch (log $WORK/logs/$ch.log)" >&2
  fi
done
