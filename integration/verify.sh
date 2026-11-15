#!/usr/bin/env bash
# usage (repo root, in the group 19 nix-shell): integration/verify.sh [change]
# Full run: group verifies (19.1) first, then each change: prepare from real runs, validate, ground-truth checks.
# Prints `WORKS <change>` or `FAILS <change>: <first failing step>`, writes integration/RESULTS.md, exit 0 only if all work.
# WITNESS_REUSE=1 keeps existing bundles; WITNESS_SKIP_GROUPS=1 skips 19.1.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
source integration/lib/env.sh
unset WITNESS_LIVE WITNESS_LIVE_CLI   # replay only: never a live model call in verify
changes=("$@"); [ ${#changes[@]} -gt 0 ] || changes=(2283 2234 2297 2129)
rc=0
mkdir -p "$WORK/logs" "$WORK/data/bundles" "$WORK/gateway"
if [ $# -eq 0 ] && [ -z "${WITNESS_SKIP_GROUPS:-}" ]; then integration/lib/groups.sh || rc=1; fi
integration/lib/build.sh || { echo "FAILS build"; exit 1; }
integration/lib/setup.sh "${changes[@]}" > "$WORK/logs/setup.log" 2>&1 || { echo "FAILS setup"; exit 1; }
results=()
check() { # <change> -> echo first failing step, or nothing
  local ch="$1" new b
  new="$(new_commit "$ch")"; b="$WORK/data/bundles/$OLD-$new"; rm -f "$WORK/logs/extra-$ch.log"
  [ -n "${WITNESS_REUSE:-}" ] && [ -f "$b/bundle.json" ] || integration/prepare-all.sh "$ch" 2>/dev/null
  [ -f "$b/bundle.json" ] || { echo "prepare: no bundle"; return; }
  grep -q '"provenance": "pipeline"' "$b/bundle.json" || { echo "provenance not pipeline"; return; }
  target/debug/validate --repo "$SB/repo" "$b" > "$WORK/logs/validate-$ch.log" 2>&1 || { echo "validate: $(tail -1 "$WORK/logs/validate-$ch.log")"; return; }
  local s; s="$(node integration/check/sections.mjs "$b")"; [ -z "$s" ] || { echo "$s"; return; }
  node integration/check/truth.mjs "$ch" "$b" 2> "$WORK/logs/extra-$ch.log" || return
  local i c
  for i in 1 2; do   # twice, each on a fresh gateway, data dir and session: the run is repeatable
    c="$(click_once "$ch" "$OLD-$new")"
    [ "$c" = ok ] || { echo "click-through run $i: $c"; return; }
  done
}
busy() { (exec 3<>/dev/tcp/127.0.0.1/$1) 2>/dev/null; }
free_port() { local p; for p in $(seq 9382 9399); do busy "$p" || [[ " $* " == *" $p "* ]] || { echo "$p"; return; }; done; return 1; }
click_once() { # <change> <key>: own stand-in and gateway on free ports, fresh data, one click-through
  local gp hp gw page c
  gp="$(free_port)" && hp="$(free_port "$gp")" || { echo "no free port"; return; }
  gw="$WORK/gateway"; rm -rf "$gw"
  PORT_GATEWAY=$gp PORT_GITHUB=$hp integration/start-page.sh > "$WORK/logs/page.log" 2>&1 &
  page=$!
  for _ in $(seq 150); do busy "$gp" && break; kill -0 $page 2>/dev/null || break; sleep 0.2; done
  if ! kill -0 $page 2>/dev/null; then echo "gateway did not start ($(tail -1 "$WORK/logs/page.log"))"; return; fi
  c="$(node integration/e2e/click.mjs "$1" "$2" "http://127.0.0.1:$gp" "$gw" "$WORK/data/bundles/$2" 2>>"$WORK/logs/click-$1.log")"
  kill $page 2>/dev/null; wait $page 2>/dev/null
  echo "$c"
}
export PLAYWRIGHT_BROWSERS_PATH="${PLAYWRIGHT_BROWSERS_PATH:-$(nix-build '<nixpkgs>' -A playwright-driver.browsers --no-out-link 2>/dev/null)}"
[ -d integration/e2e/node_modules/playwright-core ] || npm --prefix integration/e2e install --no-audit --no-fund >/dev/null 2>&1
(npm --prefix web ci --no-audit --no-fund && npm --prefix web run build) > "$WORK/logs/web.log" 2>&1 || { echo "FAILS web build"; exit 1; }
for ch in "${changes[@]}"; do
  f="$(check "$ch" | head -1)"
  if [ -z "$f" ]; then
    line="WORKS $ch"
    # 19.5: keep a replay of every result (bundle, review session, logs); the page plays it with no model call
    new="$(new_commit "$ch")"; r="$WORK/replays/$ch"; rm -rf "$r"; mkdir -p "$r"
    cp -r "$WORK/data/bundles/$OLD-$new" "$r/bundle"; cp -r "$WORK/gateway" "$r/gateway" 2>/dev/null; cp "$WORK/logs/$ch.log" "$r/" 2>/dev/null
  else line="FAILS $ch: $f"; rc=1; fi
  echo "$line"; results+=("$line"); while read -r x; do results+=("  $ch $x"); done < <(cat "$WORK/logs/extra-$ch.log" 2>/dev/null)
done
printf '%s\n' "# Integration results" "" "${results[@]/#/- }" > integration/RESULTS.md
exit $rc
