//! `nanonbt` against `fastnbt`, `simdnbt` and `pumpkin-nbt`, over structs.
//!
//! Every entry parses the same bytes into its own struct, and `write` encodes
//! that struct back. The documents are built and serialized by the owned
//! `nanonbt` derive model, so every implementation reads the same bytes.
//! Besides the three structs there are four array shapes — a byte, int and
//! long array and a short list — half a million elements each, so the array
//! paths are measured on their own.
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
//! for a rough pass. A single entry or document can be selected, as in
//! `cargo +nightly bench -- nanonbt-borrow` or
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

/// The name a document goes by in the report.
///
/// The [`Doc`](documents::Doc) documents and the [`Array`](documents::Array)
/// shapes both label their entries, so the helpers take either.
pub trait Label: Copy {
    /// The name in the report.
    fn label(self) -> &'static str;
}

impl Label for documents::Doc {
    fn label(self) -> &'static str {
        self.name()
    }
}

impl Label for documents::Array {
    fn label(self) -> &'static str {
        self.name()
    }
}

/// Runs one parse benchmark: `parse` decodes `bytes` into a `T` on every
/// iteration.
pub fn bench_parse<'a, T>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    label: impl Label,
    bytes: &'a [u8],
    parse: impl Fn(&'a [u8]) -> T,
) {
    group.bench_function(BenchmarkId::new(name, label.label()), |b| {
        b.iter(|| parse(black_box(bytes)));
    });
}

/// Runs one write benchmark: `parse` sets `T` up once, outside the timing, and
/// `write` encodes it on every iteration.
///
/// Throughput is the length `write` produces, which for a borrowed struct is
/// not always the length of the input.
pub fn bench_write<'a, T, O: AsRef<[u8]>>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    label: impl Label,
    bytes: &'a [u8],
    parse: impl Fn(&'a [u8]) -> T,
    write: impl Fn(&T) -> O,
) {
    let value = parse(bytes);
    group.throughput(Throughput::Bytes(write(&value).as_ref().len() as u64));
    group.bench_function(BenchmarkId::new(name, label.label()), |b| {
        b.iter(|| write(black_box(&value)));
    });
}

/// Keeps the suite, many benchmarks long, around five minutes.
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
        targets::nanonbt::parse_serde(&mut group, input.doc, &input.bytes);
        targets::nanonbt::parse_derive(&mut group, input.doc, &input.bytes);
        targets::nanonbt::parse_borrow(&mut group, input.doc, &input.bytes);
        #[cfg(feature = "fastnbt")]
        targets::fastnbt::parse(&mut group, input.doc, &input.bytes);
        #[cfg(feature = "simdnbt")]
        {
            targets::simdnbt::parse_borrow(&mut group, input.doc, &input.bytes);
            targets::simdnbt::parse_owned(&mut group, input.doc, &input.bytes);
        }
        #[cfg(feature = "pumpkin-nbt")]
        targets::pumpkin::parse(&mut group, input.doc, &input.bytes);
    }
    // The array entries are many and their iterations are milliseconds long,
    // so a shorter measurement keeps the suite near its five minutes.
    group
        .sample_size(30)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1));
    for input in &arrays {
        group.throughput(Throughput::Bytes(input.bytes.len() as u64));
        targets::nanonbt::parse_array(&mut group, input.kind, &input.bytes);
        #[cfg(feature = "fastnbt")]
        targets::fastnbt::parse_array(&mut group, input.kind, &input.bytes);
        #[cfg(feature = "simdnbt")]
        targets::simdnbt::parse_array(&mut group, input.kind, &input.bytes);
        #[cfg(feature = "pumpkin-nbt")]
        targets::pumpkin::parse_array(&mut group, input.kind, &input.bytes);
    }
    group.finish();
}

fn write(c: &mut Criterion) {
    let inputs = documents::inputs();
    let arrays = documents::array_inputs();
    let mut group = c.benchmark_group("write");
    configure(&mut group);
    for input in &inputs {
        targets::nanonbt::write_serde(&mut group, input.doc, &input.bytes);
        targets::nanonbt::write_derive(&mut group, input.doc, &input.bytes);
        targets::nanonbt::write_borrow(&mut group, input.doc, &input.bytes);
        #[cfg(feature = "fastnbt")]
        targets::fastnbt::write(&mut group, input.doc, &input.bytes);
        #[cfg(feature = "simdnbt")]
        {
            targets::simdnbt::write_borrow(&mut group, input.doc, &input.bytes);
            targets::simdnbt::write_owned(&mut group, input.doc, &input.bytes);
        }
        #[cfg(feature = "pumpkin-nbt")]
        targets::pumpkin::write(&mut group, input.doc, &input.bytes);
    }
    group
        .sample_size(30)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1));
    for input in &arrays {
        targets::nanonbt::write_array(&mut group, input.kind, &input.bytes);
        #[cfg(feature = "fastnbt")]
        targets::fastnbt::write_array(&mut group, input.kind, &input.bytes);
        #[cfg(feature = "simdnbt")]
        targets::simdnbt::write_array(&mut group, input.kind, &input.bytes);
        #[cfg(feature = "pumpkin-nbt")]
        targets::pumpkin::write_array(&mut group, input.kind, &input.bytes);
    }
    group.finish();
}

criterion_group!(benches, parse, write);
criterion_main!(benches);
