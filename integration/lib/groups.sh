#!/usr/bin/env bash
# 19.1: playbook §14 checks, then every merged group's verify (tasks.md, groups 0-18) on this tree, inside the
# integration nix-shell (which holds every group's packages). Prints `GROUP <n> ok|FAILS`; exit 1 if any fails.
# Logs: work/int/logs/group-<n>.log
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
source integration/lib/env.sh
mkdir -p "$WORK/logs"

echo "CHECK contract $(git log -1 --format=%H -- contracts/)"
echo "CHECK nproc $(nproc) load $(cut -d' ' -f1 /proc/loadavg) free $(free -g | awk '/Mem/{print $7}')G disk $(df -h . | awk 'NR==2{print $4}')"
for t in cargo rustc z3 lean dotnet node bwrap git; do command -v "$t" >/dev/null || { echo "CHECK missing $t"; exit 1; }; done

declare -a cmds=(
  [0]='cargo test -p contracts'
  [1]='cargo test -p sandbox'
  [2]='cargo test -p bundle'
  [3]='cargo test -p cutter'
  [4]='worker/verify.sh'
  [5]='cargo test -p translator'
  [6]='cargo test -p differ'
  [7]='cargo test -p lean'
  [8]='cargo test -p testcontract'
  [9]='cargo test -p rules'
  [10]='cargo test -p plugin-host'
  [11]='cargo test --manifest-path plugins/generator-lab/Cargo.toml'
  [12]='cargo test -p model'
  [13]='cargo test -p gateway'
  [14]='npm --prefix web ci && npm --prefix web test'
  [15]='npm --prefix web test -- src/sections/proof'
  [16]='npm --prefix web test -- src/sections/playground'
  [17]='npm --prefix web test -- src/sections/map'
  [18]='npm --prefix web test -- src/sections/review'
)
# each group verify runs as when run alone: without any variable integration exports
clean() { local u=() v; for v in $(compgen -e); do case $v in WORK|SB|SB_*|OLD|ISSUES|INT|ROOT|WORKER_DLL|PORT_*|NUGET_PACKAGES|WITNESS_*|GENERATOR_LAB_*|PLUGIN_HOST_*|CARGO_BUILD_JOBS|CARGO_TARGET_DIR|LOCKS) u+=(-u "$v");; esac; done; env "${u[@]}" "$@"; }
rc=0
for n in ${WITNESS_GROUPS:-${!cmds[@]}}; do
  if clean bash -c "${cmds[$n]}" > "$WORK/logs/group-$n.log" 2>&1; then echo "GROUP $n ok"; else echo "GROUP $n FAILS (work/int/logs/group-$n.log)"; rc=1; fi
done
exit $rc
