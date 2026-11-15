#!/usr/bin/env bash
# Regenerates bundle.json of each fixture from fixtures/src/<n>.sections.json, the commit message and the issue text.
# usage: make-bundles.sh <dir with <n>.sha, <n>.base, <n>.msg> <issues dir>   (needs jq)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
diffs="$1"; issues="$2"
for n in 2283 2234 2297 2129; do
  jq -n --arg old "$(cat "$diffs/$n.base")" --arg new "$(cat "$diffs/$n.sha")" --arg msg "$(cat "$diffs/$n.msg")" \
     --slurpfile issue "$issues/issue-$n.json" --slurpfile sec "$here/$n.sections.json" '{
    provenance: "fixture",
    repository: "junction26-messagepack-csharp",
    pullRequest: null,
    oldCommit: $old, newCommit: $new,
    requirement: {id: ($issue[0].number|tostring), title: $issue[0].title, text: $issue[0].body},
    description: ($msg | rtrimstr("\n")),
    sections: $sec[0]
  }' > "$here/../$n/bundle.json"
done
