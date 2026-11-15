#!/usr/bin/env bash
# usage: integration/lib/setup.sh <change>...  (change = 2283 2234 2297 2129)
# Prepares work/int/sb: a clone of the sample, a worktree per commit at wt/<commit>, its build (network only for
# restore), and the C# worker build. Idempotent. Everything runs inside contracts/spike/tools/sb (bwrap).
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
source "$here/env.sh"
sb="$repo/contracts/spike/tools/sb"
mkdir -p "$SB" "$SB/home"
export SB_NUGET="$SB/nuget"
run() { timeout 1200 "$sb" --root "$SB" "$@"; }

# seed the package folder from the main checkout's shared cache when present (hard links, read-mostly)
if [ ! -d "$SB_NUGET" ]; then
  seed="${WITNESS_NUGET_SEED:-}"
  if [ -n "$seed" ] && [ -d "$seed" ]; then cp -al "$seed" "$SB_NUGET" 2>/dev/null || cp -r "$seed" "$SB_NUGET"; else mkdir -p "$SB_NUGET"; fi
fi

if [ ! -d "$SB/repo/.git" ]; then
  run nonet --src git -c core.hooksPath=/dev/null clone --no-hardlinks -q /untrusted repo
  run nonet --src git -C repo fetch -q origin 'refs/remotes/origin/ju26-*:refs/remotes/origin/ju26-*'
  git -C "$SB/repo" config core.hooksPath /dev/null
fi

build_commit() { # <commit>
  local c="$1"
  [ -d "$SB/wt/$c" ] || run nonet git -C repo -c core.hooksPath=/dev/null worktree add -q --detach "../wt/$c" "$c"
  local dll="$SB/wt/$c/bin/MessagePack.SourceGenerator/Release/netstandard2.0/MessagePack.SourceGenerator.dll"
  [ -f "$dll" ] && [ -d "$SB/wt/$c/bin/MessagePack/Release/net8.0" ] && return 0
  local proj=src/MessagePack/MessagePack.csproj
  run net bash -c "cd wt/$c && dotnet restore $proj -v q" >&2
  # the build may fail on a change; that is a result, not an abort
  run nonet bash -c "cd wt/$c && dotnet build $proj -c Release -f net8.0 --no-restore -m:2 -p:UseSharedCompilation=false -v q -nologo" >&2 || echo "build failed $c" >&2
}

worker_build() {
  local dll="$SB/worker/src/Witness.Worker/bin/Release/net8.0/witness-worker.dll"
  [ -f "$dll" ] && [ "$SB/worker/src/Witness.Worker" -nt "$repo/worker/src/Witness.Worker" ] && return 0
  rm -rf "$SB/worker"; mkdir -p "$SB/worker"
  cp -r "$repo/worker/src" "$repo/worker/nuget.config" "$SB/worker/"
  find "$SB/worker" \( -name bin -o -name obj \) -prune -exec rm -rf {} +
  run net dotnet restore worker/src/Witness.Worker/Witness.Worker.csproj --configfile worker/nuget.config -v q >&2
  run nonet dotnet build worker/src/Witness.Worker/Witness.Worker.csproj -c Release --no-restore -m:2 -p:UseSharedCompilation=false -v q -nologo >&2
}

worker_build
build_commit "$OLD"
for ch in "$@"; do build_commit "$(new_commit "$ch")"; done
