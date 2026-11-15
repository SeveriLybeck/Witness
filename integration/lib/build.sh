#!/usr/bin/env bash
# Builds every binary integration runs: workspace bins, the proof module, the generator-lab plugin.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
cargo build -q -j 3 --workspace --bins
cargo build -q -j 3 --offline --manifest-path integration/proof/Cargo.toml --target-dir target
cargo build -q -j 3 --manifest-path plugins/generator-lab/Cargo.toml
cargo build -q -j 3 --offline --manifest-path integration/snippets/Cargo.toml --target-dir target
