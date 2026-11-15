#!/usr/bin/env bash
# usage: worker/setup.sh — prepares worker/work/sb: a clone of the sample, worktrees of old, 2283 and 2234,
# their generator builds (network only for restore). Idempotent; skips what exists.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/.." && pwd)"
sb="$repo/contracts/spike/tools/sb"
root="$here/work/sb"
mkdir -p "$root"
run() { timeout 900 "$sb" --root "$root" "$@"; }

if [ ! -d "$root/repo/.git" ]; then
  run nonet --src git -c core.hooksPath=/dev/null clone --no-hardlinks -q /untrusted repo
  run nonet --src git -C repo fetch -q origin 'refs/remotes/origin/ju26-*:refs/remotes/origin/ju26-*'
fi
declare -A refs=([old]=32258fdbee4bcd653c79f51b4376b5103c6f4d27 [h2283]=origin/ju26-issue-2283 [h2234]=origin/ju26-issue-2234 [h2129]=origin/ju26-issue-2129)
for v in old h2283 h2234 h2129; do
  if [ -d "$root/wt/$v" ] && [ ! -e "$root/wt/$v/.git" ]; then rm -rf "$root/wt/$v" "$root/wt/$v.ready"; run nonet git -C repo worktree prune; fi
  [ -d "$root/wt/$v" ] || run nonet git -C repo -c core.hooksPath=/dev/null worktree add -q --detach "../wt/$v" "${refs[$v]}"
done
# old and 2283: full runtime build (generator + MessagePack.dll); 2234: generator only (for Version.cs and binding)
for v in old h2283 h2234; do
  proj=src/MessagePack/MessagePack.csproj; [ "$v" = h2234 ] && proj=src/MessagePack.SourceGenerator/MessagePack.SourceGenerator.csproj
  dll="$root/wt/$v/bin/MessagePack.SourceGenerator/Release/netstandard2.0/MessagePack.SourceGenerator.dll"
  marker="$root/wt/$v.ready"   # written only after restore and build both succeeded
  [ -f "$marker" ] && [ -f "$dll" ] && continue
  rm -f "$marker"
  tfm=net8.0; [ "$v" = h2234 ] && tfm=netstandard2.0
  build() { run nonet bash -c "cd wt/$v && dotnet build $proj -c Release -f $tfm --no-restore -m:2 -p:UseSharedCompilation=false -v q -nologo"; }
  run net bash -c "cd wt/$v && dotnet restore $proj -p:Configuration=Release -v q"
  if ! build; then
    echo "setup: build of $v failed; restoring again (forced) and retrying" >&2
    run net bash -c "cd wt/$v && dotnet restore $proj -p:Configuration=Release --force -v q"
    build
  fi
  touch "$marker"
done
for v in old h2283 h2234 h2129; do git -C "$root/wt/$v" rev-parse HEAD > "$root/wt/$v.commit" 2>/dev/null || run nonet git -C "wt/$v" rev-parse HEAD > "$root/wt/$v.commit"; done
