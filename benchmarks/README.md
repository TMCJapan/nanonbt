# nanonbt benchmarks

This crate measures the same logical NBT documents through three libraries:

| Library | Role |
| --- | --- |
| [`nanonbt`](../nanonbt) | the library under test |
| [`fastnbt`](https://crates.io/crates/fastnbt) | the established serde-based encoder/decoder |
| [`simdnbt`](https://crates.io/crates/simdnbt) | the fastest tape-based decoder |

Everything is deterministic: the fixture generators in
[`src/fixtures.rs`](src/fixtures.rs) use fixed seeds, and the interop tests in
[`tests/interop.rs`](tests/interop.rs) assert that all three libraries encode
every scenario **byte-for-byte identically** before any number is compared.

## Requirements

`simdnbt` uses nightly-only compiler features (`portable_simd`), so the
benchmark crate intentionally lives **outside the Cargo workspace** (see
`exclude` in the root `Cargo.toml`); the workspace CI keeps running on a pinned
stable toolchain. Compiling the comparison needs either:

```sh
rustup toolchain install nightly      # recommended
```

or a stable compiler with the nightly feature gates unlocked
(`RUSTC_BOOTSTRAP=1`). The scripts below pick a nightly toolchain when one is
installed and fall back to `RUSTC_BOOTSTRAP=1` otherwise.

The crate has its own `Cargo.lock`; all commands use `--locked` so the
libraries under comparison stay pinned.

## Quick start

The complete local workflow is:

```sh
benches/run.sh            # all scenarios, then generate the report
benches/run.sh --quick    # fewer samples, useful while iterating
benches/run.sh small chunk  # only those criterion groups
```

The equivalent raw commands (the scripts also point `CARGO_TARGET_DIR` at the
workspace `target/` so the report tool finds criterion's output):

```sh
# 1. Run every scenario and write criterion estimates to target/criterion.
CARGO_TARGET_DIR=target cargo +nightly bench \
    --manifest-path benchmarks/Cargo.toml --locked --bench compare

# 2. Re-render the comparison charts from those estimates.
CARGO_TARGET_DIR=target cargo +nightly run \
    --manifest-path benchmarks/Cargo.toml --locked --bin report
```

Criterion's own HTML report (per-benchmark distributions) is written to
`target/criterion/report/index.html`. The comparison report, with grouped and
relative bar charts, is written to `target/nanonbt-bench-report/index.html`
(plus `report.md`).

`NANONBT_BENCH_QUICK=1` shortens warm-up/measurement times and sample counts
without changing which benchmarks run.

## What is measured

> Benchmarks are only as good as the machine they run on. Run the full suite
> on an idle machine (and prefer a dedicated runner for CI comparisons);
> shared runners can vary several-fold between runs on nanosecond-scale
> benchmarks, which is why the default Bencher threshold is a statistical
> interval rather than a fixed percentage.

Each scenario measures the same document through every path the libraries
support:

| Benchmark suffix | Meaning |
| --- | --- |
| `*_write` | encode into a fresh `Vec` (allocation included) |
| `*_write_reuse` | encode into a reused buffer |
| `*_read` | decode into the library's natural typed representation |
| `*_read_borrowed` | decode into a borrowing representation |
| `*_read_manual` | decode with `simdnbt`'s borrow tape and copy out the fields |
| `*_read_value` | decode into `fastnbt::Value` (tree baseline) |
| `*_read_tree_borrow` | `simdnbt::borrow::read` tape parse only |
| `*_read_tree_owned` | `simdnbt::owned::read` owned tree parse |

Read benchmarks walk the whole decoded payload into a checksum
([`src/lib.rs`](src/lib.rs)), so a zero-copy decoder cannot win by skipping
data access.

### Scenarios

| Group | Document |
| --- | --- |
| `small` | eight scalar fields |
| `byte_array` | 1 MiB `TAG_Byte_Array` |
| `int_array` | 65,536 `TAG_Int_Array` values |
| `long_array` | 65,536 `TAG_Long_Array` values |
| `strings` | one 32 KiB string + 2,048 short strings |
| `compounds` | 1,024 compound tags with nested string lists |
| `many_fields` | 48 scalar fields + a string (field-name dispatch) |
| `deep` | four levels of nested compounds |
| `chunk` | Minecraft-flavored chunk section with arrays and a palette |

### Fairness notes

* All three libraries encode the exact same bytes, verified by
  `tests/interop.rs`; reads use `nanonbt`'s encoding as the shared input.
* `simdnbt` has no derived support for `TAG_Byte_Array` / `TAG_Int_Array` /
  `TAG_Long_Array` fields, so those scenarios use its borrow tape plus manual
  extraction (`*_read_manual`) and an owned-tree write path.
* `simdnbt`'s derived `Deserialize` converts the borrowed tape into owned
  fields, so `simdnbt_read` is "parse + convert"; `simdnbt_read_tree_borrow`
  isolates the parse.
* `nanonbt`'s derive currently requires array fields to be borrowed as slices,
  so borrowed variants keep strings and nested lists owned; `fastnbt` can
  borrow strings as `Cow<str>`, and its numbers reflect that.
* `fastnbt`/`simdnbt` byte arrays are iterated element-by-element when
  checksummed, which is also how their `iter()` APIs expose bytes.

## Continuous benchmarking with Bencher

[Bencher](https://bencher.dev) tracks these criterion benchmarks over time,
detects regressions against the branch history, and posts the results as a
comment and a GitHub check on every pull request.

### One-time setup

1. Create a Bencher account and project at <https://bencher.dev>, or run a
   [self-hosted instance](https://bencher.dev/docs/explanation/bencher-self-hosted/).
2. Add the project slug as a repository **variable** named `BENCHER_PROJECT`
   (`Settings → Secrets and variables → Actions → Variables`).
3. Add a project API key as a repository **secret** named `BENCHER_API_KEY`.
4. Make sure the default branch in `.github/workflows/benchmarks-base.yml`
   matches your repository (the examples use `main`).

### Workflows

| Workflow | Trigger | What it does |
| --- | --- | --- |
| [`benchmarks-base.yml`](../.github/workflows/benchmarks-base.yml) | push to `main` | records the baseline history and defines the regression threshold |
| [`benchmarks-pr.yml`](../.github/workflows/benchmarks-pr.yml) | pull request | runs the same benchmarks on the PR, compares with the base branch, comments on the PR, and fails on a regression alert |
| [`benchmarks-pr-closed.yml`](../.github/workflows/benchmarks-pr-closed.yml) | PR closed | archives the PR branch in Bencher |

Until `BENCHER_PROJECT` and `BENCHER_API_KEY` exist, the benchmark jobs still
run the interop tests but skip Bencher tracking with a notice instead of
failing.

The base workflow defines a latency threshold as a 99% t-test prediction
interval over the last 64 runs: a result outside that interval generates an
alert and fails the job. This adapts to the noise of shared GitHub runners;
tighten it (or switch to a percentage threshold) in `benchmarks-base.yml` once
enough history exists. PR runs clone the threshold from their start point.
The PR comment always lists every benchmark's result and its change, so
improvements are visible even when nothing alerts.

Fork pull requests do not receive repository secrets, so `benchmarks-pr.yml`
intentionally skips them. If you accept external pull requests and want
benchmarks for them, use Bencher's two-workflow
[`workflow_run` pattern](https://bencher.dev/docs/how-to/github-actions/#pull-requests-from-forks).

### Running Bencher locally

Install the CLI:

```sh
curl --proto '=https' --tlsv1.2 -sSfL https://bencher.dev/download/install-cli.sh | sh
```

Then track a run from your machine:

```sh
BENCHER_PROJECT=project-abc123 \
BENCHER_API_KEY=... \
benches/bencher-local.sh
```

The script uses the current git branch and a `localhost` testbed by default,
so local runs get their own history and thresholds. `BENCHER_TESTBED` and
`BENCHER_HOST` (for self-hosted Bencher) are also supported.
`BENCHER_DRY_RUN=1` parses the benchmarks and prints the report without
uploading anything, which is handy for validating the setup.

`bencher run` accepts any command, so the same comparison can be made without
the script:

```sh
bencher run \
  --project "$BENCHER_PROJECT" \
  --key "$BENCHER_API_KEY" \
  --branch "$(git rev-parse --abbrev-ref HEAD)" \
  --testbed localhost \
  --adapter rust_criterion \
  "cargo +nightly bench --manifest-path benchmarks/Cargo.toml --locked --bench compare"
```
