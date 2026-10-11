//! `nanocesu8` against the `cesu8` and `simd_cesu8` crates.
//!
//! `decode` and `encode` run the three implementations on the same inputs,
//! `reject` times refusing malformed bytes, and `validate` is `nanocesu8`
//! alone, whose `Cesu8::new` validates without decoding. The `nanocesu8`
//! decode entry is `Cesu8::new` followed by `decode`, the pair a caller
//! uses.
//!
//! Run with `cargo bench -p nanocesu8 --bench compare`, or add `-- --quick`
//! for a rough pass. A single group or input can be selected, as in
//! `cargo bench -p nanocesu8 --bench compare -- decode/mutf8`. Add
//! `--features simd` to run the `nanocesu8` entries on the vectorized paths.

use std::{hint::black_box, time::Duration};

use criterion::{
    BatchSize, BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};

/// `small` is a typical `NBT` string; `large` is past where SIMD pays off.
const SIZES: [(&str, usize); 2] = [("small", 32), ("large", 16 * 1024)];

/// Texts that exercise the borrow path (modified UTF-8 that is also UTF-8)
/// and the decode path (modified UTF-8 that is not). The flag picks the
/// spelling of the decode input; the text is encoded canonically for the
/// encode benchmarks.
const SHAPES: [(&str, &str, bool); 6] = [
    ("ascii", "a", true),
    ("unicode", "aé日", true),
    ("nul", "a\0b", false),
    ("mutf8", "a\0\u{10401}日", false),
    ("nulls", "\0", false),
    ("surrogates", "\u{10401}", false),
];

struct Input {
    name: String,
    text: String,
    bytes: Vec<u8>,
}

/// Repeats `unit` until the text is at least `len` bytes.
fn repeat(unit: &str, len: usize) -> String {
    unit.repeat(len.div_ceil(unit.len()))
}

fn inputs() -> Vec<Input> {
    SIZES
        .into_iter()
        .flat_map(|(size, len)| {
            SHAPES.into_iter().map(move |(shape, unit, utf8)| {
                let text = repeat(unit, len);
                let bytes = if utf8 {
                    text.as_bytes().to_vec()
                } else {
                    nanocesu8::Cesu8Buf::from(text.as_str()).into_bytes()
                };
                Input {
                    name: format!("{shape}/{size}"),
                    text,
                    bytes,
                }
            })
        })
        .collect()
}

/// Keeps the suite, many benchmarks long, around five minutes.
fn configure(group: &mut BenchmarkGroup<'_, WallTime>) {
    group
        .sample_size(50)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
}

fn bench_decode(group: &mut BenchmarkGroup<'_, WallTime>, name: &str, bytes: &[u8]) {
    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function(BenchmarkId::new("nanocesu8", name), |b| {
        b.iter_batched(
            || bytes,
            |bytes| nanocesu8::Cesu8::new(black_box(bytes)).unwrap().decode(),
            BatchSize::SmallInput,
        );
    });
    group.bench_function(BenchmarkId::new("cesu8", name), |b| {
        b.iter_batched(
            || bytes,
            |bytes| cesu8::from_java_cesu8(black_box(bytes)).unwrap(),
            BatchSize::SmallInput,
        );
    });
    group.bench_function(BenchmarkId::new("simd_cesu8", name), |b| {
        b.iter_batched(
            || bytes,
            |bytes| simd_cesu8::mutf8::decode(black_box(bytes)).unwrap(),
            BatchSize::SmallInput,
        );
    });
}

fn bench_encode(group: &mut BenchmarkGroup<'_, WallTime>, name: &str, text: &str) {
    group.throughput(Throughput::Bytes(text.len() as u64));
    group.bench_function(BenchmarkId::new("nanocesu8", name), |b| {
        b.iter_batched(
            || text,
            |text| nanocesu8::Cesu8::from_str(black_box(text)),
            BatchSize::SmallInput,
        );
    });
    group.bench_function(BenchmarkId::new("cesu8", name), |b| {
        b.iter_batched(
            || text,
            |text| cesu8::to_java_cesu8(black_box(text)),
            BatchSize::SmallInput,
        );
    });
    group.bench_function(BenchmarkId::new("simd_cesu8", name), |b| {
        b.iter_batched(
            || text,
            |text| simd_cesu8::mutf8::encode(black_box(text)),
            BatchSize::SmallInput,
        );
    });
}

fn bench_reject(group: &mut BenchmarkGroup<'_, WallTime>, name: &str, bytes: &[u8]) {
    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function(BenchmarkId::new("nanocesu8", name), |b| {
        b.iter_batched(
            || bytes,
            |bytes| nanocesu8::Cesu8::new(black_box(bytes)).ok(),
            BatchSize::SmallInput,
        );
    });
    group.bench_function(BenchmarkId::new("cesu8", name), |b| {
        b.iter_batched(
            || bytes,
            |bytes| cesu8::from_java_cesu8(black_box(bytes)).ok(),
            BatchSize::SmallInput,
        );
    });
    group.bench_function(BenchmarkId::new("simd_cesu8", name), |b| {
        b.iter_batched(
            || bytes,
            |bytes| simd_cesu8::mutf8::decode(black_box(bytes)).ok(),
            BatchSize::SmallInput,
        );
    });
}

fn bench_validate(group: &mut BenchmarkGroup<'_, WallTime>, name: &str, bytes: &[u8]) {
    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function(BenchmarkId::new("Cesu8::new", name), |b| {
        b.iter_batched(
            || bytes,
            |bytes| nanocesu8::Cesu8::new(black_box(bytes)),
            BatchSize::SmallInput,
        );
    });
}

fn decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");
    configure(&mut group);
    for input in inputs() {
        bench_decode(&mut group, &input.name, &input.bytes);
    }
    group.finish();
}

fn encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");
    configure(&mut group);
    for input in inputs() {
        bench_encode(&mut group, &input.name, &input.text);
    }
    group.finish();
}

fn reject(c: &mut Criterion) {
    let mut group = c.benchmark_group("reject");
    configure(&mut group);
    for (size, len) in SIZES {
        let mut bytes = repeat("a", len).into_bytes();
        bytes[len - 1] = 0xff;
        bench_reject(&mut group, &format!("invalid/{size}"), &bytes);
    }
    group.finish();
}

fn validate(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate");
    configure(&mut group);
    for input in inputs() {
        bench_validate(&mut group, &input.name, &input.bytes);
    }
    group.finish();
}

criterion_group!(benches, decode, encode, reject, validate);
criterion_main!(benches);
