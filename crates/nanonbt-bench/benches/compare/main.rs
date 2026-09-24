//! `nanonbt` against `fastnbt`, `simdnbt` and `pumpkin-nbt`, over structs.
//!
//! Every entry parses the same bytes into its own struct, and `write` encodes
//! that struct back. The documents are built and serialized by the owned
//! `nanonbt` derive model, so every implementation reads the same bytes.
//! Besides the five structs there are four array shapes — a byte, int and
//! long array and a short list — half a million elements each, so the array
//! paths are measured on their own.
//!
//! A third group, `skip`, measures the other side of parsing: each of its
//! eleven documents holds a small `kept` entry and one `skipped` entry of a
//! hundred thousand elements, and the targets declare `kept` alone, so
//! everything else must be passed over. `nanonbt` skips without decoding, the
//! serde side through serde's `IgnoredAny` and the derive through
//! `Read::skip`; `fastnbt` through the same `IgnoredAny`; `simdnbt` and
//! `pumpkin-nbt` cannot skip, so their entries read the whole document and
//! look up `kept` afterwards. Six of the shapes are lists whose elements all
//! take the same number of bytes — byte, short, int, long, float and double —
//! which a structural skip can pass over in one step; the rest are the three
//! arrays, a list of strings and a list of compounds. `nanonbt-borrow` has no
//! skip entry: with only an `i32` to read, its struct would compile to the
//! owned one.
//!
//! `short-names` and `long-names` are the same compound of 64 `i32` fields
//! with three- and sixty-four-byte keys, so the pair isolates what the names
//! themselves cost each target. Their keys are drawn by the `random_names`
//! attribute macro from a fixed seed, so they never appear in this source and
//! every target still reads the same bytes. The name pair has no
//! `nanonbt-borrow` entry: with nothing to borrow, its struct would compile
//! to the owned one.
//!
//! The array documents give every element both spellings on the `nanonbt`
//! side, owned and borrowed: bytes lend as `&[u8]`/`&[i8]`, and the wider
//! elements as the big-endian wrappers, `&[U16Be]` through `&[I64Be]`. The
//! other libraries appear with the array types they have: `fastnbt` owns its
//! `ByteArray`/`IntArray`/`LongArray` and borrows from the input,
//! `simdnbt-borrow` and `simdnbt-owned` read a tape or a tree (their borrowed
//! int and long arrays copy), and `pumpkin-nbt` reads and writes through its
//! accessors. NBT has no short array, so 16-bit elements are lists, and
//! `fastnbt` has no borrowed list to read them with.
//!
//! `nanonbt` has three entries: `nanonbt-serde` uses the `serde` feature,
//! `nanonbt-derive` the owned `FromNBT`/`ToNBT` derive, and `nanonbt-borrow` a
//! derived struct that borrows strings and arrays from the input. The borrowed
//! struct writes those borrowed arrays as lists, one byte longer than the
//! array tags it read, so its throughput is the length it produces.
//!
//! This crate is outside the root workspace because `simdnbt` uses nightly
//! features, and the rest of the repository pins a stable toolchain. Run it
//! with `cd crates/nanonbt-bench && cargo +nightly bench`, or add `-- --quick`
//! for a rough pass. Add `-- --noplot` to skip the per-entry charts, which
//! take most of the wall time. A single entry or document can be selected, as
//! in `cargo +nightly bench -- nanonbt-borrow` or
//! `cargo +nightly bench -- player`. Add `--features nanonbt/simd` to run the
//! `nanonbt` entries on the vectorized paths. All three of
//! `fastnbt`, `pumpkin-nbt`, and `simdnbt` are enabled by default;
//! `pumpkin-nbt` is always listed last in the report.
//!
//! Three entries need a note. `nanonbt-borrow` (above) changes the encoding of
//! borrowed arrays. `simdnbt-borrow` keeps a tape over the input and decodes
//! strings lazily; its `int_array` and `long_array` accessors copy, so those
//! fields own their data where the rest borrows. `simdnbt-owned` reads the
//! tree first and copies what its accessors will not lend.

// Struct fields are named after their NBT keys, so that neither `serde` nor
// the `nanonbt` derive needs a rename; the derive's generated bindings then
// carry those names too.
#![allow(non_snake_case)]

use std::{hint::black_box, time::Duration};

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};

mod documents;
mod targets;

/// Runs one parse benchmark: `parse` decodes `bytes` into a `T` on every
/// iteration.
pub fn bench_parse<'a, T, S: std::fmt::Display>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    label: S,
    bytes: &'a [u8],
    parse: impl Fn(&'a [u8]) -> T,
) {
    group.bench_function(BenchmarkId::new(name, label), |b| {
        b.iter(|| parse(black_box(bytes)));
    });
}

/// Runs one write benchmark: `parse` sets `T` up once, outside the timing, and
/// `write` encodes it on every iteration.
///
/// Throughput is the length `write` produces, which for a borrowed struct is
/// not always the length of the input.
pub fn bench_write<'a, T, O: AsRef<[u8]>, S: std::fmt::Display>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    label: S,
    bytes: &'a [u8],
    parse: impl Fn(&'a [u8]) -> T,
    write: impl Fn(&T) -> O,
) {
    let value = parse(bytes);
    group.throughput(Throughput::Bytes(write(&value).as_ref().len() as u64));
    group.bench_function(BenchmarkId::new(name, label), |b| {
        b.iter(|| write(black_box(&value)));
    });
}

/// Keeps the groups, many benchmarks long, around ten minutes; `--noplot`
/// removes the per-entry charts, which are most of that time.
pub fn configure(group: &mut BenchmarkGroup<'_, WallTime>) {
    group
        .sample_size(50)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
}

fn parse(c: &mut Criterion) {
    let inputs = documents::inputs();
    let arrays = documents::array_inputs();
    let mut group = c.benchmark_group("parse");
    configure(&mut group);
    for input in &inputs {
        // The `bench_parse` entries record this length; the `write` group sets
        // its own per entry.
        group.throughput(Throughput::Bytes(input.bytes.len() as u64));
        targets::nanonbt::parse(
            &mut group,
            documents::BenchInput::Doc(input.doc, &input.bytes),
        );
        #[cfg(feature = "fastnbt")]
        targets::fastnbt::parse(
            &mut group,
            documents::BenchInput::Doc(input.doc, &input.bytes),
        );
        #[cfg(feature = "simdnbt")]
        targets::simdnbt::parse(
            &mut group,
            documents::BenchInput::Doc(input.doc, &input.bytes),
        );
        #[cfg(feature = "pumpkin-nbt")]
        targets::pumpkin::parse(
            &mut group,
            documents::BenchInput::Doc(input.doc, &input.bytes),
        );
    }
    // The array entries are many and their iterations are milliseconds long,
    // so a shorter measurement keeps the suite's array half near two minutes.
    group
        .sample_size(30)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1));
    for input in &arrays {
        group.throughput(Throughput::Bytes(input.bytes.len() as u64));
        targets::nanonbt::parse(
            &mut group,
            documents::BenchInput::Array(input.kind, &input.bytes),
        );
        #[cfg(feature = "fastnbt")]
        targets::fastnbt::parse(
            &mut group,
            documents::BenchInput::Array(input.kind, &input.bytes),
        );
        #[cfg(feature = "simdnbt")]
        targets::simdnbt::parse(
            &mut group,
            documents::BenchInput::Array(input.kind, &input.bytes),
        );
        #[cfg(feature = "pumpkin-nbt")]
        targets::pumpkin::parse(
            &mut group,
            documents::BenchInput::Array(input.kind, &input.bytes),
        );
    }
    group.finish();
}

fn write(c: &mut Criterion) {
    let inputs = documents::inputs();
    let arrays = documents::array_inputs();
    let mut group = c.benchmark_group("write");
    configure(&mut group);
    for input in &inputs {
        targets::nanonbt::write(
            &mut group,
            documents::BenchInput::Doc(input.doc, &input.bytes),
        );
        #[cfg(feature = "fastnbt")]
        targets::fastnbt::write(
            &mut group,
            documents::BenchInput::Doc(input.doc, &input.bytes),
        );
        #[cfg(feature = "simdnbt")]
        targets::simdnbt::write(
            &mut group,
            documents::BenchInput::Doc(input.doc, &input.bytes),
        );
        #[cfg(feature = "pumpkin-nbt")]
        targets::pumpkin::write(
            &mut group,
            documents::BenchInput::Doc(input.doc, &input.bytes),
        );
    }
    group
        .sample_size(30)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1));
    for input in &arrays {
        targets::nanonbt::write(
            &mut group,
            documents::BenchInput::Array(input.kind, &input.bytes),
        );
        #[cfg(feature = "fastnbt")]
        targets::fastnbt::write(
            &mut group,
            documents::BenchInput::Array(input.kind, &input.bytes),
        );
        #[cfg(feature = "simdnbt")]
        targets::simdnbt::write(
            &mut group,
            documents::BenchInput::Array(input.kind, &input.bytes),
        );
        #[cfg(feature = "pumpkin-nbt")]
        targets::pumpkin::write(
            &mut group,
            documents::BenchInput::Array(input.kind, &input.bytes),
        );
    }
    group.finish();
}

fn skip(c: &mut Criterion) {
    let skips = documents::skip_inputs();
    let mut group = c.benchmark_group("skip");
    // A skip is short work, but the entries that cannot skip read the whole
    // document, and a compound or string walk reallocates per element, so
    // their iterations are milliseconds long. Ten samples over two seconds
    // hold the slowest of those without the "unable to complete" warning; the
    // entries that finish in nanoseconds still run millions of iterations,
    // and the confidence interval comes from resampling, not the sample count.
    group
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2));
    for input in &skips {
        group.throughput(Throughput::Bytes(input.bytes.len() as u64));
        targets::nanonbt::skip(&mut group, input.kind, &input.bytes);
        #[cfg(feature = "fastnbt")]
        targets::fastnbt::skip(&mut group, input.kind, &input.bytes);
        #[cfg(feature = "simdnbt")]
        targets::simdnbt::skip(&mut group, input.kind, &input.bytes);
        #[cfg(feature = "pumpkin-nbt")]
        targets::pumpkin::skip(&mut group, input.kind, &input.bytes);
    }
    group.finish();
}

criterion_group!(benches, parse, write, skip);
criterion_main!(benches);
