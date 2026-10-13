#!/usr/bin/env bash
#
# Runs the nanonbt comparison benchmarks and regenerates the chart report.
#
# Usage:
#   benchmarks/run.sh [--quick] [criterion-filter ...]
#
# Examples:
#   benchmarks/run.sh                 # full run
#   benchmarks/run.sh --quick         # fewer samples (fast iteration)
#   benchmarks/run.sh small chunk     # only those criterion groups
#
# simdnbt requires nightly-only features; this script prefers a nightly
# toolchain and falls back to RUSTC_BOOTSTRAP=1 on stable.

set -euo pipefail

cd "$(dirname "$0")/.."

# Keep the benchmark build artifacts and criterion data in the workspace target
# directory, so repeated runs reuse the compiled workspace dependencies and the
# report tool finds `target/criterion` without extra flags.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"

quick=0
if [[ "${1:-}" == "--quick" ]]; then
    quick=1
    shift
fi

if rustup run nightly rustc --version >/dev/null 2>&1; then
    CARGO=(cargo +nightly)
else
    echo "note: nightly toolchain not found, using RUSTC_BOOTSTRAP=1 on stable" >&2
    CARGO=(cargo)
    export RUSTC_BOOTSTRAP=1
fi

if [[ "$quick" == "1" ]]; then
    export NANONBT_BENCH_QUICK=1
fi

"${CARGO[@]}" bench --manifest-path benchmarks/Cargo.toml --locked --bench compare -- "$@"
"${CARGO[@]}" run --manifest-path benchmarks/Cargo.toml --locked --bin report
