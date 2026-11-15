#!/usr/bin/env bash
# Writes the proof, test and rule records of the two "read, not run" fixtures (2297, 2129) from their parts and blocks.
# Every production line is `not modelled` (translator not run), every test `not-run`. Needs jq.
set -euo pipefail
f="$(cd "$(dirname "$0")/.." && pwd)"

common() { # $1 = change, $2 = differences JSON, $3 = tests JSON, $4 = difference block
  local d="$f/$1"
  jq --argjson diffs "$2" '{provenance: "fixture", differences: $diffs}' -n > "$d/differences.json"
  echo '{"provenance": "fixture", "rows": []}' > "$d/rows.json"
  echo '{"provenance": "fixture", "rows": [], "coverage": []}' > "$d/lean.json"
  jq --argjson tests "$3" -n '{provenance: "fixture", tests: $tests}' > "$d/tests.json"
  echo '{"provenance": "fixture", "dependencies": []}' > "$d/dependencies.json"
  [ -e "$d/groups.json" ] || echo '{"provenance": "fixture", "groups": []}' > "$d/groups.json"
  jq --arg reason "$5" '. as $p | {provenance: "fixture",
    changedLines: (reduce ("production","test","docs","snapshot","generated") as $c ({};
      .[$c] = ([$p.parts[] | select(.category == $c) | (.addedLines|length) + (.deletedLines|length)] | add // 0))),
    modelledLines: 0, assumptions: [],
    notModelled: [$p.parts[] | select(.category == "production" and .new != null) | {location: .new, reason: $reason}]}' "$d/parts.json" > "$d/scope.json"
  jq --arg db "$4" '{provenance: "fixture", links: [.blocks[] | select(.category == "production") | {block: .id, state: "not-declared", tests: [], placement: "outside"}]}' "$d/blocks.json" > "$d/links.json"
  jq --arg db "$4" --argjson arch "${6:-[]}" '{provenance: "fixture", levels: [.blocks[] | if .category == "production"
     then {block: .id, level: 3, rules: ((if .id == $db then ["undeclared-difference"] else [] end) + (if (.id as $i | $arch | index($i)) then ["architectural"] else [] end) + ["not-modelled-line"])}
     else {block: .id, level: 1, rules: []} end]}' "$d/blocks.json" > "$d/levels.json"
  jq --arg db "$4" --slurpfile t "$d/tests.json" '[.blocks[] | select(.category == "production")] as $pb
    | ([{rule: "undeclared-difference", block: $db, differences: ["d1"], tests: []}]
       + [$pb[] | select(.id != $db) | {rule: "level-3-block", block: .id, differences: [], tests: []}]
       + [{rule: "test-group", block: null, differences: [], tests: [$t[0].tests[].name]}])
    | {provenance: "fixture", questions: [to_entries[] | {id: ("q" + ((.key + 1)|tostring))} + .value]}' "$d/blocks.json" > "$d/questions.json"
  echo '{"provenance": "fixture", "mode": "full", "rules": ["level-3-block", "not-run-tests"]}' > "$d/mode.json"
}

t() { # name file line
  jq -n --arg n "$1" --arg f "$2" --argjson l "$3" '{name: $n, location: {file: $f, line: $l, endLine: $l}, change: "added",
    category: "not-run", old: "not-run", new: "not-run", failureMessage: null, assertion: {before: null, after: null}}'
}

# 2297: one output-changed difference on the GenericDataTypeWithPrivateMember snapshot, attached to TransformUtilities.cs:58
snap='tests/MessagePack.SourceGenerator.Tests/Resources/GenericDataTypeWithPrivateMember/Formatters.HasPrivateSerializedMemberAndIsGenericNamed`1.HasPrivateSerializedMemberAndIsGenericNamedFormatter.g.cs'
b9=$(jq -r '.blocks[] | select(.name == "TransformUtilities.cs:58") | .id' "$f/2297/blocks.json")
d2297=$(jq -n --arg s "$snap" --arg b "$b9" '[{id: "d1", number: 1, kind: "output-changed", solverKind: "output-changed", checker: "file-comparison",
  rows: [], block: $b, condition: "generic type with constraints", old: "{ and class on one line", new: "{ on its own line, class below the guard",
  location: {file: $s, line: 9, endLine: 13}}]')
ct=tests/MessagePack.SourceGenerator.Tests/CodeCoverageExclusionTests.cs
tests2297=$(jq -s . <(t CodeCoverageExclusionTests.AllGeneratedTypesAreExcludedFromCodeCoverage $ct 65) <(t CodeCoverageExclusionTests.ExcludeAttributeIsGuardedForTargetsWhereItIsUnavailable $ct 82))
common 2297 "$d2297" "$tests2297" "$b9" "translator not run"

# 2129: one public-api difference for the new attribute
b1=$(jq -r '.blocks[] | select(.name == "Attributes.cs:142") | .id' "$f/2129/blocks.json")
d2129=$(jq -n --arg b "$b1" '[{id: "d1", number: 1, kind: "public-api", solverKind: "public-api", checker: "api-comparison",
  rows: [], block: $b, condition: "new public attribute", old: "absent", new: "MessagePack.GeneratedMessagePackFormatterAttribute",
  location: {file: "src/MessagePack.Annotations/Attributes.cs", line: 142, endLine: 177}}]')
et=tests/MessagePack.SourceGenerator.ExecutionTests/GeneratedFormatterForExternalTypesTests.cs
at=tests/MessagePack.SourceGenerator.Tests/GeneratedMessagePackFormatterTests.cs
tests2129=$(jq -s . \
  <(t GeneratedFormatterForExternalTypesTests.ContractlessClassRoundTrips $et 12) \
  <(t GeneratedFormatterForExternalTypesTests.ContractlessClassUsesStringKeysForPublicReadWriteMembers $et 24) \
  <(t GeneratedFormatterForExternalTypesTests.ContractlessClassNullRoundTrips $et 37) \
  <(t GeneratedFormatterForExternalTypesTests.ContractlessStructRoundTrips $et 44) \
  <(t GeneratedFormatterForExternalTypesTests.SurrogateControlsKeysAndMembership $et 55) \
  <(t GeneratedFormatterForExternalTypesTests.MessagePackObjectCanContainExternalTypes $et 72) \
  <(t GeneratedFormatterForExternalTypesTests.GeneratedFormattersAreRegisteredInGeneratedResolver $et 89) \
  <(t GeneratedMessagePackFormatterTests.FormatterMustBePartial $at 29) \
  <(t GeneratedMessagePackFormatterTests.FormatterMustImplementFormatterOfT $at 57) \
  <(t GeneratedMessagePackFormatterTests.FormattedTypeMustNotBeAbstract $at 78) \
  <(t GeneratedMessagePackFormatterTests.SurrogateMemberMustMatchFormattedType $at 105) \
  <(t GeneratedMessagePackFormatterTests.SurrogateMemberMustHaveMatchingType $at 139) \
  <(t GeneratedMessagePackFormatterTests.SurrogateKeysMustNotMixStringAndInt $at 173))
arch=$(jq -c '[.blocks[] | select(.name | test("^(Attributes.cs|GeneratedFormatterCollector.cs|GeneratedFormatterRequest.cs|MsgPack00xMessagePackAnalyzer.cs)")) | .id]' "$f/2129/blocks.json")
common 2129 "$d2129" "$tests2129" "$b1" "translator not run" "$arch"
