# sourced: shared paths for the integration scripts
INT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$INT/.." && pwd)"
export WORK="${WITNESS_INT_WORK:-$ROOT/work/int}"
export SB="$WORK/sb"
export SB_NUGET="$SB/nuget"
export OLD=32258fdbee4bcd653c79f51b4376b5103c6f4d27
export ISSUES="${WITNESS_ISSUES:-/home/slopsoftwareagent1/Documents/junction/challenge_material/issues}"
export WITNESS_NUGET_SEED="${WITNESS_NUGET_SEED:-/home/slopsoftwareagent1/Documents/junction/reviewInterface/work/nuget}"
export WORKER_DLL="$SB/worker/src/Witness.Worker/bin/Release/net8.0/witness-worker.dll"
export CI=1 GIT_PAGER=cat GIT_EDITOR=true DOTNET_NOLOGO=1 DOTNET_CLI_TELEMETRY_OPTOUT=1 MSBUILDDISABLENODEREUSE=1 CARGO_BUILD_JOBS=3
# integration ports (contracts/ports.md: group 19 owns 9380-9399)
export PORT_GATEWAY="${PORT_GATEWAY:-9380}" PORT_GITHUB="${PORT_GITHUB:-9381}"
new_commit() { git -C "$SB/repo" rev-parse "refs/remotes/origin/ju26-issue-$1"; }
