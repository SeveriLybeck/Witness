#!/usr/bin/env bash
# usage: run.sh <wt> <level> <out> <ranges> <methods>
SP=/tmp/claude-1000/-home-slopsoftwareagent1-Documents-junction-reviewInterface/6f8f94d6-23d7-4568-b0f3-5740533d854a/scratchpad/spike
$SP/tools/sb nonet dotnet $SP/cfg-check/bin/Release/net8.0/cfg-check.dll $SP/wt/$1 $SP/home/.nuget/packages $2 $SP/cfg-check/$3 "$4" "$5"
