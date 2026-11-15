#!/usr/bin/env bash
# usage (repo root, in the integration nix-shell): integration/start-page.sh
# Starts the GitHub stand-in (9381) and the gateway (9380) serving web/dist, the bundles of work/int/data and the
# generator-lab plugin through the plugin host. Run prepare-all.sh first. Page: http://127.0.0.1:9380/
# Stops both on exit (Ctrl-C). PIDs in work/int/page.pids. Token for sign-off: reviewer-token (login reviewer-a).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
source integration/lib/env.sh
[ -f web/dist/index.html ] || (npm --prefix web ci --no-audit --no-fund && npm --prefix web run build) >&2
repo_name=fork/junction26-messagepack-csharp
pulls=()
for ch in 2283 2234 2297 2129; do
  case $ch in 2283) n=1;; 2234) n=2;; 2297) n=3;; 2129) n=4;; esac
  new="$(new_commit "$ch" 2>/dev/null)" || continue
  pulls+=(--pull "$repo_name#$n=$new:agent-author")
done
export GENERATOR_LAB_TREES="$SB/wt/{commit}" GENERATOR_LAB_WORKER="$WORKER_DLL" GENERATOR_LAB_BUNDLES="$WORK/data/bundles"
export GENERATOR_LAB_REVERTED="$ROOT/work/sandbox/testcontract-{new12}/testcontract/rev/{new12}/{block}"
export NUGET_PACKAGES="$SB_NUGET"
mkdir -p "$WORK/gateway" "$WORK/logs"
target/debug/github-standin --port "$PORT_GITHUB" "${pulls[@]}" --token reviewer-token=reviewer-a > "$WORK/logs/standin.log" 2>&1 &
s=$!
target/debug/gateway --port "$PORT_GATEWAY" --bundles "$WORK/data/bundles" --data "$WORK/gateway" --repo "$SB/repo" \
  --web web/dist --trees "$SB/wt/{commit}" --plugins plugins --plugins plugins/web --plugin-host target/debug/plugin-host --plugin-timeout 120 \
  --github "http://127.0.0.1:$PORT_GITHUB" --github-repo "$repo_name" > "$WORK/logs/gateway.log" 2>&1 &
g=$!
echo "$s $g" > "$WORK/page.pids"
trap 'kill $s $g 2>/dev/null' EXIT INT TERM
for _ in $(seq 100); do (exec 3<>/dev/tcp/127.0.0.1/$PORT_GATEWAY) 2>/dev/null && break; sleep 0.1; done
echo "page http://127.0.0.1:$PORT_GATEWAY/"
wait $g
