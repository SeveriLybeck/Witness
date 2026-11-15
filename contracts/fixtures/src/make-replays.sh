#!/usr/bin/env bash
# Writes replays.json for 2283 and 2234 from the spike's consumer cases (contracts/spike/cases).
# Needs jq.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; c="$here/../../spike/cases"; f="$here/.."
pair() { printf '// lib\n%s\n// app\n%s' "$(cat "$c/$1.lib.cs")" "$(cat "$c/$2.app.cs")"; }
app() { cat "$c/$1.app.cs"; }
g6='// lib
using MessagePack;
namespace Lib;
[MessagePackObject]
public struct Item<T> { [Key(null)] public string Key { get; set; } }
// app
using MessagePack; using Lib;
namespace App;
[MessagePackObject]
public sealed class Cmd { [Key(1)] public Item<int> Member { get; set; } }'
jq -n --arg a3 "$(pair A3 A3)" --arg g6 "$g6" --arg a1 "$(pair A1 A1)" '{provenance: "fixture", replays: [
  {difference: "d1", input: {source: "model", text: $a3}, old: "MsgPack004: global::Lib.S<int>.Unkeyed", new: "0 warnings; round trip Unkeyed 42 -> 0",
   state: "seen-live", roundTrip: {sent: "42", received: "0"}},
  {difference: "d2", input: {source: "model", text: $g6}, old: "CS8785, AD0001: IndexOutOfRangeException", new: "0 warnings; formatter without Key",
   state: "seen-live", roundTrip: null},
  {difference: "d3", input: {source: "issue", text: $a1}, old: "CS8785, AD0001: IndexOutOfRangeException", new: "0 warnings; round trip [\"s\",[\"k\"]]",
   state: "seen-live", roundTrip: null}]}' > "$f/2283/replays.json"
jq -n --arg b6 "$(app B6)" --arg b4 "$(app B4)" --arg b4s "$(app B4s)" '{provenance: "fixture", replays: [
  {difference: "d1", input: {source: "model", text: $b6, buildProperties: {NoWarn: "MsgPack005"}}, old: "MsgPack005, AD0001, CS8785: InvalidOperationException", new: "0 warnings; round trip B 2 -> 0",
   state: "seen-live", roundTrip: {sent: "2", received: "0"}},
  {difference: "d2", input: {source: "model", text: $b4, buildProperties: {NoWarn: "MsgPack005"}}, old: "MsgPack005, AD0001, CS8785: ArgumentException", new: "0 warnings; round trip B 2 -> 0",
   state: "seen-live", roundTrip: {sent: "2", received: "0"}},
  {difference: "d3", input: {source: "model", text: $b4s, buildProperties: {NoWarn: "MsgPack005"}}, old: "MsgPack005, AD0001, CS8785: ArgumentException", new: "0 warnings; round trip B 2 -> 0",
   state: "seen-live", roundTrip: {sent: "2", received: "0"}}]}' > "$f/2234/replays.json"
# The 2283 recording (fixtures/recordings/) is keyed on crates/model review_prompt() of fixtures/2283 with
# purpose ai-review and model::review_schema(); regenerate it with a program calling those, keeping its response.
