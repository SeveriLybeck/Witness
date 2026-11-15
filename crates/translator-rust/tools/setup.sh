#!/usr/bin/env bash
# Installs Charon, Aeneas and Aeneas's Lean library under <repo>/work, pinned; prints versions.
# Finished steps are skipped. The Lean library step needs mathlib (about 10 GB free disk).
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
W=$ROOT/work
AENEAS_REV=0d21a9ec7bbbb110110cacc14464a47fd71c8705
NIX="nix --extra-experimental-features nix-command --extra-experimental-features flakes"
mkdir -p "$W"
# Aeneas's flake builds aeneas and the charon it pins
[ -e "$W/aeneas/bin/aeneas" ] || $NIX build "github:AeneasVerif/aeneas/$AENEAS_REV" -o "$W/aeneas"
[ -e "$W/charon/bin/charon" ] || $NIX build "github:AeneasVerif/aeneas/$AENEAS_REV#charon" -o "$W/charon"
echo "charon: $("$W/charon/bin/charon" version)"
echo "aeneas: $("$W/aeneas/bin/aeneas" -version)"
SRC=$($NIX flake metadata "github:AeneasVerif/aeneas/$AENEAS_REV" --json | sed -n 's/.*"path":"\([^"]*\)".*/\1/p' | head -1)
export ELAN_HOME=$W/elan
if [ ! -e "$W/aeneas-lean/.lake/build/lib/lean/Aeneas.olean" ]; then
  FREE=$(df -Pk "$W" | awk 'NR==2 {print int($4/1048576)}')
  if [ "$FREE" -lt 12 ]; then
    echo "aeneas-lean: NOT BUILT, ${FREE} GB free, mathlib needs about 10 GB (needs 12)" >&2
    exit 3
  fi
  [ -d "$W/aeneas-lean" ] || { cp -r "$SRC/backends/lean" "$W/aeneas-lean"; chmod -R u+w "$W/aeneas-lean"; }
  (cd "$W/aeneas-lean" && nix-shell -p elan git curl cacert --run 'lake exe cache get && lake build Aeneas')
fi
echo "lean: $("$W/elan/toolchains/leanprover--lean4---v4.36.0-rc2/bin/lean" --version)"
