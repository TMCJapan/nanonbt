# nanonbt
nbt for all.

## Benchmarks

`benchmarks/` compares nanonbt against [fastnbt](https://crates.io/crates/fastnbt)
and [simdnbt](https://crates.io/crates/simdnbt) across typed structs, borrowed
vs. owned decoding, and read/write paths, then renders grouped and relative bar
charts:

```sh
benches/run.sh                # run every scenario, then generate the charts
benches/run.sh --quick        # fewer samples while iterating
benches/run.sh small chunk    # only some criterion groups
```

Results land in `target/nanonbt-bench-report/index.html` (plus `report.md`).
Continuous benchmarking on GitHub Actions is wired through
[Bencher](https://bencher.dev): pull requests get a benchmark comment and a
check that fails on regressions. See [benchmarks/README.md](benchmarks/README.md)
for the scenario list and setup instructions.

> The comparison suite needs a nightly compiler (or `RUSTC_BOOTSTRAP=1`)
> because simdnbt uses nightly-only features, so it lives outside the Cargo
> workspace and has its own lockfile.
