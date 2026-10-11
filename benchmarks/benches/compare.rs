//! Criterion benchmarks comparing `nanonbt` with `fastnbt` and `simdnbt`.
//!
//! Every scenario encodes the exact same logical document with all three
//! libraries (asserted by the interop tests) and then measures:
//!
//! * `*_write` — encode into a fresh `Vec`,
//! * `*_write_reuse` — encode into a reused buffer,
//! * `*_read` — decode into the library's natural typed representation,
//! * `*_read_borrowed` — decode into a borrowing representation, where the
//!   library supports one,
//! * `*_read_value` / `*_read_tree_*` — decode into a tree, as a baseline.
//!
//! All reads walk the decoded payload into a checksum so that zero-copy
//! decoders cannot skip data access; see [`nanonbt_bench::checksum`].
//!
//! Run with:
//!
//! ```text
//! cargo bench -p nanonbt-bench --bench compare          # full run
//! NANONBT_BENCH_QUICK=1 cargo bench -p nanonbt-bench --bench compare
//! ```
//!
//! Note: `simdnbt` currently requires a nightly compiler (or
//! `RUSTC_BOOTSTRAP=1`). See `benchmarks/README.md`.

use std::hint::black_box;
use std::time::Duration;

use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, Criterion, Throughput, criterion_group, criterion_main};

use nanonbt_bench::{fastnbt_models, fixtures, nanonbt_models, simdnbt_models};

fn quick_mode() -> bool {
    std::env::var_os("NANONBT_BENCH_QUICK").is_some()
}

fn config() -> Criterion {
    let criterion = Criterion::default();
    if quick_mode() {
        criterion
            .warm_up_time(Duration::from_millis(300))
            .measurement_time(Duration::from_millis(1200))
            .sample_size(30)
    } else {
        criterion
            .warm_up_time(Duration::from_secs(1))
            .measurement_time(Duration::from_secs(3))
            .sample_size(50)
    }
}

fn write_bench<T>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    data: &T,
    f: impl Fn(&T) -> Vec<u8>,
) {
    group.bench_function(name, |b| {
        b.iter(|| black_box(f(black_box(data)).len()));
    });
}

fn write_reuse_bench<T>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    data: &T,
    capacity: usize,
    f: impl Fn(&T, &mut Vec<u8>),
) {
    let mut buffer = Vec::with_capacity(capacity);
    group.bench_function(name, |b| {
        b.iter(|| {
            f(black_box(data), &mut buffer);
            black_box(buffer.len())
        });
    });
}

fn read_bench(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    bytes: &[u8],
    f: impl Fn(&[u8]) -> u64,
) {
    group.bench_function(name, |b| {
        b.iter(|| black_box(f(black_box(bytes))));
    });
}

fn bench_small(c: &mut Criterion) {
    let fixture = fixtures::small();
    let nanonbt = nanonbt_models::small_from_fixture(&fixture);
    let fastnbt = fastnbt_models::small_from_fixture(&fixture);
    let simdnbt = simdnbt_models::small_from_fixture(&fixture);
    let bytes = nanonbt_models::small_write(&nanonbt);

    let mut group = c.benchmark_group("small");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    write_bench(
        &mut group,
        "nanonbt_write",
        &nanonbt,
        nanonbt_models::small_write,
    );
    write_reuse_bench(
        &mut group,
        "nanonbt_write_reuse",
        &nanonbt,
        bytes.len() + 64,
        nanonbt_models::small_write_into,
    );
    write_bench(
        &mut group,
        "fastnbt_write",
        &fastnbt,
        fastnbt_models::small_write,
    );
    write_reuse_bench(
        &mut group,
        "fastnbt_write_reuse",
        &fastnbt,
        bytes.len() + 64,
        fastnbt_models::small_write_into,
    );
    write_bench(
        &mut group,
        "simdnbt_write",
        &simdnbt,
        simdnbt_models::small_write,
    );
    write_reuse_bench(
        &mut group,
        "simdnbt_write_reuse",
        &simdnbt,
        bytes.len() + 64,
        simdnbt_models::small_write_into,
    );

    read_bench(
        &mut group,
        "nanonbt_read",
        &bytes,
        nanonbt_models::small_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read",
        &bytes,
        fastnbt_models::small_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read_borrowed",
        &bytes,
        fastnbt_models::small_read_borrowed,
    );
    read_bench(
        &mut group,
        "fastnbt_read_value",
        &bytes,
        fastnbt_models::value_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read",
        &bytes,
        simdnbt_models::small_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_borrow",
        &bytes,
        simdnbt_models::borrow_tree_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_owned",
        &bytes,
        simdnbt_models::owned_tree_read,
    );
    group.finish();
}

fn bench_byte_array(c: &mut Criterion) {
    let fixture = fixtures::byte_array();
    let nanonbt = nanonbt_models::byte_array_from_fixture(&fixture);
    let fastnbt = fastnbt_models::byte_array_from_fixture(&fixture);
    let bytes = nanonbt_models::byte_array_owned(&nanonbt);

    let mut group = c.benchmark_group("byte_array");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    write_bench(
        &mut group,
        "nanonbt_write",
        &nanonbt,
        nanonbt_models::byte_array_owned,
    );
    write_reuse_bench(
        &mut group,
        "nanonbt_write_reuse",
        &nanonbt,
        bytes.len() + 64,
        nanonbt_models::byte_array_write_into,
    );
    write_bench(
        &mut group,
        "fastnbt_write",
        &fastnbt,
        fastnbt_models::byte_array_owned,
    );
    write_reuse_bench(
        &mut group,
        "fastnbt_write_reuse",
        &fastnbt,
        bytes.len() + 64,
        fastnbt_models::byte_array_write_into,
    );
    write_bench(&mut group, "simdnbt_write", &fixture, |data| {
        simdnbt_models::byte_array_write(data)
    });
    write_reuse_bench(
        &mut group,
        "simdnbt_write_reuse",
        &fixture,
        bytes.len() + 64,
        |data, buffer| simdnbt_models::byte_array_write_into(data, buffer),
    );

    read_bench(
        &mut group,
        "nanonbt_read",
        &bytes,
        nanonbt_models::byte_array_read,
    );
    read_bench(
        &mut group,
        "nanonbt_read_borrowed",
        &bytes,
        nanonbt_models::byte_array_read_borrowed,
    );
    read_bench(
        &mut group,
        "fastnbt_read",
        &bytes,
        fastnbt_models::byte_array_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read_borrowed",
        &bytes,
        fastnbt_models::byte_array_read_borrowed,
    );
    read_bench(
        &mut group,
        "fastnbt_read_value",
        &bytes,
        fastnbt_models::value_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_manual",
        &bytes,
        simdnbt_models::byte_array_read_manual,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_borrow",
        &bytes,
        simdnbt_models::borrow_tree_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_owned",
        &bytes,
        simdnbt_models::owned_tree_read,
    );
    group.finish();
}

fn bench_int_array(c: &mut Criterion) {
    let fixture = fixtures::int_array();
    let nanonbt = nanonbt_models::int_array_from_fixture(&fixture);
    let fastnbt = fastnbt_models::int_array_from_fixture(&fixture);
    let bytes = nanonbt_models::int_array_owned(&nanonbt);

    let mut group = c.benchmark_group("int_array");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    write_bench(
        &mut group,
        "nanonbt_write",
        &nanonbt,
        nanonbt_models::int_array_owned,
    );
    write_reuse_bench(
        &mut group,
        "nanonbt_write_reuse",
        &nanonbt,
        bytes.len() + 64,
        nanonbt_models::int_array_write_into,
    );
    write_bench(
        &mut group,
        "fastnbt_write",
        &fastnbt,
        fastnbt_models::int_array_owned,
    );
    write_reuse_bench(
        &mut group,
        "fastnbt_write_reuse",
        &fastnbt,
        bytes.len() + 64,
        fastnbt_models::int_array_write_into,
    );
    write_bench(&mut group, "simdnbt_write", &fixture, |data| {
        simdnbt_models::int_array_write(data)
    });
    write_reuse_bench(
        &mut group,
        "simdnbt_write_reuse",
        &fixture,
        bytes.len() + 64,
        |data, buffer| simdnbt_models::int_array_write_into(data, buffer),
    );

    read_bench(
        &mut group,
        "nanonbt_read",
        &bytes,
        nanonbt_models::int_array_read,
    );
    read_bench(
        &mut group,
        "nanonbt_read_borrowed",
        &bytes,
        nanonbt_models::int_array_read_borrowed,
    );
    read_bench(
        &mut group,
        "fastnbt_read",
        &bytes,
        fastnbt_models::int_array_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read_borrowed",
        &bytes,
        fastnbt_models::int_array_read_borrowed,
    );
    read_bench(
        &mut group,
        "fastnbt_read_value",
        &bytes,
        fastnbt_models::value_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_manual",
        &bytes,
        simdnbt_models::int_array_read_manual,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_borrow",
        &bytes,
        simdnbt_models::borrow_tree_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_owned",
        &bytes,
        simdnbt_models::owned_tree_read,
    );
    group.finish();
}

fn bench_long_array(c: &mut Criterion) {
    let fixture = fixtures::long_array();
    let nanonbt = nanonbt_models::long_array_from_fixture(&fixture);
    let fastnbt = fastnbt_models::long_array_from_fixture(&fixture);
    let bytes = nanonbt_models::long_array_owned(&nanonbt);

    let mut group = c.benchmark_group("long_array");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    write_bench(
        &mut group,
        "nanonbt_write",
        &nanonbt,
        nanonbt_models::long_array_owned,
    );
    write_reuse_bench(
        &mut group,
        "nanonbt_write_reuse",
        &nanonbt,
        bytes.len() + 64,
        nanonbt_models::long_array_write_into,
    );
    write_bench(
        &mut group,
        "fastnbt_write",
        &fastnbt,
        fastnbt_models::long_array_owned,
    );
    write_reuse_bench(
        &mut group,
        "fastnbt_write_reuse",
        &fastnbt,
        bytes.len() + 64,
        fastnbt_models::long_array_write_into,
    );
    write_bench(&mut group, "simdnbt_write", &fixture, |data| {
        simdnbt_models::long_array_write(data)
    });
    write_reuse_bench(
        &mut group,
        "simdnbt_write_reuse",
        &fixture,
        bytes.len() + 64,
        |data, buffer| simdnbt_models::long_array_write_into(data, buffer),
    );

    read_bench(
        &mut group,
        "nanonbt_read",
        &bytes,
        nanonbt_models::long_array_read,
    );
    read_bench(
        &mut group,
        "nanonbt_read_borrowed",
        &bytes,
        nanonbt_models::long_array_read_borrowed,
    );
    read_bench(
        &mut group,
        "fastnbt_read",
        &bytes,
        fastnbt_models::long_array_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read_borrowed",
        &bytes,
        fastnbt_models::long_array_read_borrowed,
    );
    read_bench(
        &mut group,
        "fastnbt_read_value",
        &bytes,
        fastnbt_models::value_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_manual",
        &bytes,
        simdnbt_models::long_array_read_manual,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_borrow",
        &bytes,
        simdnbt_models::borrow_tree_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_owned",
        &bytes,
        simdnbt_models::owned_tree_read,
    );
    group.finish();
}

fn bench_strings(c: &mut Criterion) {
    let fixture = fixtures::strings();
    let nanonbt = nanonbt_models::strings_from_fixture(&fixture);
    let fastnbt = fastnbt_models::strings_from_fixture(&fixture);
    let simdnbt = simdnbt_models::strings_from_fixture(&fixture);
    let bytes = nanonbt_models::strings_write(&nanonbt);

    let mut group = c.benchmark_group("strings");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    write_bench(
        &mut group,
        "nanonbt_write",
        &nanonbt,
        nanonbt_models::strings_write,
    );
    write_reuse_bench(
        &mut group,
        "nanonbt_write_reuse",
        &nanonbt,
        bytes.len() + 64,
        nanonbt_models::strings_write_into,
    );
    write_bench(
        &mut group,
        "fastnbt_write",
        &fastnbt,
        fastnbt_models::strings_write,
    );
    write_reuse_bench(
        &mut group,
        "fastnbt_write_reuse",
        &fastnbt,
        bytes.len() + 64,
        fastnbt_models::strings_write_into,
    );
    write_bench(
        &mut group,
        "simdnbt_write",
        &simdnbt,
        simdnbt_models::strings_write,
    );
    write_reuse_bench(
        &mut group,
        "simdnbt_write_reuse",
        &simdnbt,
        bytes.len() + 64,
        simdnbt_models::strings_write_into,
    );

    read_bench(
        &mut group,
        "nanonbt_read",
        &bytes,
        nanonbt_models::strings_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read",
        &bytes,
        fastnbt_models::strings_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read_borrowed",
        &bytes,
        fastnbt_models::strings_read_borrowed,
    );
    read_bench(
        &mut group,
        "fastnbt_read_value",
        &bytes,
        fastnbt_models::value_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read",
        &bytes,
        simdnbt_models::strings_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_borrow",
        &bytes,
        simdnbt_models::borrow_tree_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_owned",
        &bytes,
        simdnbt_models::owned_tree_read,
    );
    group.finish();
}

fn bench_compounds(c: &mut Criterion) {
    let fixture = fixtures::compounds();
    let nanonbt = nanonbt_models::compounds_from_fixture(&fixture);
    let fastnbt = fastnbt_models::compounds_from_fixture(&fixture);
    let simdnbt = simdnbt_models::compounds_from_fixture(&fixture);
    let bytes = nanonbt_models::compounds_write(&nanonbt);

    let mut group = c.benchmark_group("compounds");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    write_bench(
        &mut group,
        "nanonbt_write",
        &nanonbt,
        nanonbt_models::compounds_write,
    );
    write_reuse_bench(
        &mut group,
        "nanonbt_write_reuse",
        &nanonbt,
        bytes.len() + 64,
        nanonbt_models::compounds_write_into,
    );
    write_bench(
        &mut group,
        "fastnbt_write",
        &fastnbt,
        fastnbt_models::compounds_write,
    );
    write_reuse_bench(
        &mut group,
        "fastnbt_write_reuse",
        &fastnbt,
        bytes.len() + 64,
        fastnbt_models::compounds_write_into,
    );
    write_bench(
        &mut group,
        "simdnbt_write",
        &simdnbt,
        simdnbt_models::compounds_write,
    );
    write_reuse_bench(
        &mut group,
        "simdnbt_write_reuse",
        &simdnbt,
        bytes.len() + 64,
        simdnbt_models::compounds_write_into,
    );

    read_bench(
        &mut group,
        "nanonbt_read",
        &bytes,
        nanonbt_models::compounds_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read",
        &bytes,
        fastnbt_models::compounds_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read_borrowed",
        &bytes,
        fastnbt_models::compounds_read_borrowed,
    );
    read_bench(
        &mut group,
        "fastnbt_read_value",
        &bytes,
        fastnbt_models::value_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read",
        &bytes,
        simdnbt_models::compounds_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_borrow",
        &bytes,
        simdnbt_models::borrow_tree_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_owned",
        &bytes,
        simdnbt_models::owned_tree_read,
    );
    group.finish();
}

fn bench_many_fields(c: &mut Criterion) {
    let fixture = fixtures::many_fields();
    let nanonbt = nanonbt_models::many_fields_from_fixture(&fixture);
    let fastnbt = fastnbt_models::many_fields_from_fixture(&fixture);
    let simdnbt = simdnbt_models::many_fields_from_fixture(&fixture);
    let bytes = nanonbt_models::many_fields_write(&nanonbt);

    let mut group = c.benchmark_group("many_fields");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    write_bench(
        &mut group,
        "nanonbt_write",
        &nanonbt,
        nanonbt_models::many_fields_write,
    );
    write_reuse_bench(
        &mut group,
        "nanonbt_write_reuse",
        &nanonbt,
        bytes.len() + 64,
        nanonbt_models::many_fields_write_into,
    );
    write_bench(
        &mut group,
        "fastnbt_write",
        &fastnbt,
        fastnbt_models::many_fields_write,
    );
    write_reuse_bench(
        &mut group,
        "fastnbt_write_reuse",
        &fastnbt,
        bytes.len() + 64,
        fastnbt_models::many_fields_write_into,
    );
    write_bench(
        &mut group,
        "simdnbt_write",
        &simdnbt,
        simdnbt_models::many_fields_write,
    );
    write_reuse_bench(
        &mut group,
        "simdnbt_write_reuse",
        &simdnbt,
        bytes.len() + 64,
        simdnbt_models::many_fields_write_into,
    );

    read_bench(
        &mut group,
        "nanonbt_read",
        &bytes,
        nanonbt_models::many_fields_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read",
        &bytes,
        fastnbt_models::many_fields_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read_value",
        &bytes,
        fastnbt_models::value_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read",
        &bytes,
        simdnbt_models::many_fields_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_borrow",
        &bytes,
        simdnbt_models::borrow_tree_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_owned",
        &bytes,
        simdnbt_models::owned_tree_read,
    );
    group.finish();
}

fn bench_deep(c: &mut Criterion) {
    let fixture = fixtures::deep();
    let nanonbt = nanonbt_models::deep_from_fixture(&fixture);
    let fastnbt = fastnbt_models::deep_from_fixture(&fixture);
    let simdnbt = simdnbt_models::deep_from_fixture(&fixture);
    let bytes = nanonbt_models::deep_write(&nanonbt);

    let mut group = c.benchmark_group("deep");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    write_bench(
        &mut group,
        "nanonbt_write",
        &nanonbt,
        nanonbt_models::deep_write,
    );
    write_reuse_bench(
        &mut group,
        "nanonbt_write_reuse",
        &nanonbt,
        bytes.len() + 64,
        nanonbt_models::deep_write_into,
    );
    write_bench(
        &mut group,
        "fastnbt_write",
        &fastnbt,
        fastnbt_models::deep_write,
    );
    write_reuse_bench(
        &mut group,
        "fastnbt_write_reuse",
        &fastnbt,
        bytes.len() + 64,
        fastnbt_models::deep_write_into,
    );
    write_bench(
        &mut group,
        "simdnbt_write",
        &simdnbt,
        simdnbt_models::deep_write,
    );
    write_reuse_bench(
        &mut group,
        "simdnbt_write_reuse",
        &simdnbt,
        bytes.len() + 64,
        simdnbt_models::deep_write_into,
    );

    read_bench(
        &mut group,
        "nanonbt_read",
        &bytes,
        nanonbt_models::deep_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read",
        &bytes,
        fastnbt_models::deep_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read_borrowed",
        &bytes,
        fastnbt_models::deep_read_borrowed,
    );
    read_bench(
        &mut group,
        "fastnbt_read_value",
        &bytes,
        fastnbt_models::value_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read",
        &bytes,
        simdnbt_models::deep_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_borrow",
        &bytes,
        simdnbt_models::borrow_tree_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_owned",
        &bytes,
        simdnbt_models::owned_tree_read,
    );
    group.finish();
}

fn bench_chunk(c: &mut Criterion) {
    let fixture = fixtures::chunk();
    let nanonbt = nanonbt_models::chunk_from_fixture(&fixture);
    let fastnbt = fastnbt_models::chunk_from_fixture(&fixture);
    let bytes = nanonbt_models::chunk_write(&nanonbt);

    let mut group = c.benchmark_group("chunk");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    write_bench(
        &mut group,
        "nanonbt_write",
        &nanonbt,
        nanonbt_models::chunk_write,
    );
    write_reuse_bench(
        &mut group,
        "nanonbt_write_reuse",
        &nanonbt,
        bytes.len() + 64,
        nanonbt_models::chunk_write_into,
    );
    write_bench(
        &mut group,
        "fastnbt_write",
        &fastnbt,
        fastnbt_models::chunk_write,
    );
    write_reuse_bench(
        &mut group,
        "fastnbt_write_reuse",
        &fastnbt,
        bytes.len() + 64,
        fastnbt_models::chunk_write_into,
    );
    write_bench(&mut group, "simdnbt_write", &fixture, |data| {
        simdnbt_models::chunk_write(data)
    });
    write_reuse_bench(
        &mut group,
        "simdnbt_write_reuse",
        &fixture,
        bytes.len() + 64,
        simdnbt_models::chunk_write_into,
    );

    read_bench(
        &mut group,
        "nanonbt_read",
        &bytes,
        nanonbt_models::chunk_read,
    );
    read_bench(
        &mut group,
        "nanonbt_read_borrowed",
        &bytes,
        nanonbt_models::chunk_read_borrowed,
    );
    read_bench(
        &mut group,
        "fastnbt_read",
        &bytes,
        fastnbt_models::chunk_read,
    );
    read_bench(
        &mut group,
        "fastnbt_read_borrowed",
        &bytes,
        fastnbt_models::chunk_read_borrowed,
    );
    read_bench(
        &mut group,
        "fastnbt_read_value",
        &bytes,
        fastnbt_models::value_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_manual",
        &bytes,
        simdnbt_models::chunk_read_manual,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_borrow",
        &bytes,
        simdnbt_models::borrow_tree_read,
    );
    read_bench(
        &mut group,
        "simdnbt_read_tree_owned",
        &bytes,
        simdnbt_models::owned_tree_read,
    );
    group.finish();
}

criterion_group! {
    name = benches;
    config = config();
    targets =
        bench_small,
        bench_byte_array,
        bench_int_array,
        bench_long_array,
        bench_strings,
        bench_compounds,
        bench_many_fields,
        bench_deep,
        bench_chunk,
}

criterion_main!(benches);
