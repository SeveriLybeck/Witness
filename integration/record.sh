#!/usr/bin/env bash
# usage (repo root, integration nix-shell): integration/record.sh <change>...
# The only step that makes live model calls (D43): prepares each change with WITNESS_LIVE set, so the review
# module (one ai-review call) and the playground module (one replay-snippet call per difference left not replayed)
# call the Claude CLI through the model crate and record under integration/recordings/. verify.sh never sets it.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
[ $# -gt 0 ] || { echo "usage: integration/record.sh <change>..." >&2; exit 2; }
export WITNESS_LIVE="${WITNESS_LIVE_CLI:-$(command -v claude)}"
[ -x "$WITNESS_LIVE" ] || { echo "record: no claude CLI" >&2; exit 1; }
before=$(ls integration/recordings | wc -l)
integration/prepare-all.sh "$@"
echo "record: $(( $(ls integration/recordings | wc -l) - before )) new recordings in integration/recordings" >&2
