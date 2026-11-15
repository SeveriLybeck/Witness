#!/usr/bin/env bash
# review module: AI review from recordings (D43). A live call happens only when WITNESS_LIVE names the CLI,
# which only integration/record.sh sets; the prompt is checked for host paths and secrets first.
set -euo pipefail
if [ -n "${WITNESS_LIVE:-}" ]; then
  target/debug/witness-snippets --recordings "$WITNESS_RECORDINGS" --review-prompt "$2" /dev/null
  exec target/debug/model-review --recordings "${WITNESS_RECORDINGS:?}" --live "$WITNESS_LIVE" "$@"
fi
exec target/debug/model-review --recordings "${WITNESS_RECORDINGS:?}" "$@"
