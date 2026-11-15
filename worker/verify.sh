#!/usr/bin/env bash
# usage (from the repo root): nix-shell -p dotnet-sdk_8 bubblewrap git --run 'worker/verify.sh'
# Builds and tests the C# worker inside contracts/spike/tools/sb; network only for restore (D5, D43).
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/.." && pwd)"
sb="$repo/contracts/spike/tools/sb"
root="$here/work/sb"
export CI=1 GIT_PAGER=cat GIT_EDITOR=true DOTNET_NOLOGO=1 DOTNET_CLI_TELEMETRY_OPTOUT=1 MSBUILDDISABLENODEREUSE=1
TOOLCHAIN_QUIET=1 source "$repo/contracts/toolchain.env"
mkdir -p "$root"
run() { timeout 900 "$sb" --root "$root" "$@"; }

echo "== checks"
echo "contract $(git -C "$repo" log -1 --format=%H -- contracts/ 2>/dev/null || echo unknown)"
echo "nproc $(nproc); load $(cut -d' ' -f1 /proc/loadavg); free $(free -g | awk '/Mem/{print $7}') GB; disk $(df -h "$repo" | awk 'NR==2{print $4}')"
v="$(run nonet dotnet --version)"
echo "dotnet in sandbox $v"
[ "$v" = 8.0.425 ] || { echo "expected dotnet 8.0.425"; exit 1; }
if run nonet curl -sS -m 5 https://api.nuget.org/v3/index.json >/dev/null 2>&1; then echo "sandbox has network"; exit 1; fi
echo "nonet: no network"

echo "== sample checkouts"
"$here/setup.sh"

echo "== copy sources"
rm -rf "$root/w/src" "$root/w/tests" "$root/contracts"
mkdir -p "$root/w" "$root/contracts/messages" "$root/contracts/fixtures/2283"
cp -r "$here/src" "$here/tests" "$here/nuget.config" "$root/w/"
cp "$repo"/contracts/messages/*.json "$root/contracts/messages/"
cp "$repo/contracts/schemas/common.schema.json" "$root/contracts/messages/"
cp -r "$repo/contracts/fixtures/2283/inputs" "$root/contracts/fixtures/2283/"
find "$root/w" -name bin -o -name obj | xargs rm -rf

proj=w/tests/Witness.Worker.Tests/Witness.Worker.Tests.csproj
echo "== restore (network)"
run net dotnet restore "$proj" --configfile w/nuget.config -v q
echo "== build (no network)"
run nonet dotnet build "$proj" -c Release --no-restore -m:2 -p:UseSharedCompilation=false -v q -nologo
echo "== test (no network)"
tc="$repo/.toolchain"
run nonet env WITNESS_SB="$root" WITNESS_BWRAP="$(readlink -f "$tc/bubblewrap")/bin/bwrap" WITNESS_DOTNET="$(readlink -f "$tc/dotnet-sdk_8")/bin/dotnet" \
  dotnet test "$proj" -c Release --no-build --logger "console;verbosity=detailed" -- RunConfiguration.TestSessionTimeout=600000
