#!/usr/bin/env bash
#
# Tracks the nanonbt comparison benchmarks with Bencher from your machine.
#
# Required environment:
#   BENCHER_PROJECT   project slug from the Bencher console
#   BENCHER_API_KEY   Bencher API key (not needed with BENCHER_DRY_RUN=1)
#
# Optional environment:
#   BENCHER_DRY_RUN   set to 1 to parse the benchmarks without uploading
#   BENCHER_TESTBED   testbed name (default: localhost)
#   BENCHER_HOST      self-hosted Bencher instance URL
#   BENCHER_BRANCH    branch name (default: current git branch)
#   BENCH_FILTER      criterion filter passed after `--`
#
# Usage:
#   BENCHER_PROJECT=project-abc123 BENCHER_API_KEY=... benchmarks/bencher-local.sh
#   BENCHER_PROJECT=project-abc123 BENCHER_DRY_RUN=1 benchmarks/bencher-local.sh

set -euo pipefail

cd "$(dirname "$0")/.."

# Keep build artifacts and criterion data in the workspace target directory.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"

: "${BENCHER_PROJECT:?set BENCHER_PROJECT to your Bencher project slug}"

branch="${BENCHER_BRANCH:-$(git rev-parse --abbrev-ref HEAD)}"
testbed="${BENCHER_TESTBED:-localhost}"
filter="${BENCH_FILTER:-}"

dry_run_args=()
key_args=()
if [[ "${BENCHER_DRY_RUN:-0}" == "1" ]]; then
    dry_run_args=(--dry-run)
else
    : "${BENCHER_API_KEY:?set BENCHER_API_KEY to a Bencher API key (or BENCHER_DRY_RUN=1)}"
    key_args=(--key "$BENCHER_API_KEY")
fi

command=(cargo)
if rustup run nightly rustc --version >/dev/null 2>&1; then
    command=(cargo +nightly)
else
    echo "note: nightly toolchain not found, using RUSTC_BOOTSTRAP=1 on stable" >&2
    export RUSTC_BOOTSTRAP=1
fi

host_args=()
if [[ -n "${BENCHER_HOST:-}" ]]; then
    host_args=(--host "$BENCHER_HOST")
fi

bench_args=()
if [[ -n "$filter" ]]; then
    bench_args=(-- "$filter")
fi

bencher run \
    --project "$BENCHER_PROJECT" \
    "${key_args[@]}" \
    --branch "$branch" \
    --testbed "$testbed" \
    --adapter rust_criterion \
    "${dry_run_args[@]}" \
    "${host_args[@]}" \
    "${command[@]}" bench --manifest-path benchmarks/Cargo.toml --locked --bench compare "${bench_args[@]}"
