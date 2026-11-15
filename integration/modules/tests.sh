#!/usr/bin/env bash
# tests module: test contract with one sandbox root per change (work/sandbox/testcontract-<new12>)
set -euo pipefail
exec target/debug/testcontract --root "testcontract-${5:0:12}" "$@"
