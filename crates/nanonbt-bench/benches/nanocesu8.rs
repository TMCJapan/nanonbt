//! `nanocesu8` alone, counted under callgrind.
//!
//! `crates/nanocesu8/benches/compare.rs` compares `nanocesu8`, `cesu8` and
//! `simd_cesu8` by wall time; this target runs the same operations on the
//! same inputs but measures the `nanocesu8` implementation alone. The
//! reference crates do not change with the pull request, so their counts
//! would be the same on both sides of the report's comparison; only the
//! crate's own entries can move.
//!
//! The four groups mirror the criterion suite: `decode` is `Cesu8::new`
//! followed by `decode`, the pair a caller uses; `encode` is
//! `Cesu8::from_str`; `reject` refuses malformed bytes; and `validate` is
//! `Cesu8::new` alone, which validates without decoding.
//!
//! Every entry is named `<kind>_nanocesu8_<id>`, the naming contract
//! `examples/bench-summary.rs` reads: `decode_nanocesu8_ascii_small` is
//! `decode/nanocesu8/ascii-small`. The id is the criterion suite's input
//! name with `_` where it has `/`.
//!
//! The callgrind jobs of `.github/workflows/bench.yml` run this target next
//! to the comparison suite under the same `--features simd`, so the entries
//! follow the vectorized paths there. Run it locally with
//! `cd crates/nanonbt-bench && cargo +nightly bench --bench nanocesu8`; a
//! recent valgrind and a matching `iai-callgrind-runner` must be on the
//! path.

use std::hint::black_box;

use iai_callgrind::{Callgrind, LibraryBenchmarkConfig, library_benchmark_group, main};
use nanocesu8::{Cesu8, Cesu8Buf};

/// `small` is a typical `NBT` string; `large` is past where SIMD pays off.
/// The same pair as the criterion suite.
const SMALL: usize = 32;
const LARGE: usize = 16 * 1024;

/// One input shape: the unit the text repeats and whether the canonical text
/// is already modified UTF-8, which decides the spelling the decode entries
/// borrow or decode.
#[derive(Clone, Copy)]
struct Shape {
    unit: &'static str,
    utf8: bool,
}

// The six shapes of the criterion suite, named the way the entry ids spell
// them.
const ASCII: Shape = Shape {
    unit: "a",
    utf8: true,
};
const UNICODE: Shape = Shape {
    unit: "aé日",
    utf8: true,
};
const NUL: Shape = Shape {
    unit: "a\0b",
    utf8: false,
};
const MUTF8: Shape = Shape {
    unit: "a\0\u{10401}日",
    utf8: false,
};
const NULLS: Shape = Shape {
    unit: "\0",
    utf8: false,
};
const SURROGATES: Shape = Shape {
    unit: "\u{10401}",
    utf8: false,
};

/// Repeats the shape's unit until the text is at least `len` bytes.
fn repeat(shape: Shape, len: usize) -> String {
    shape.unit.repeat(len.div_ceil(shape.unit.len()))
}

/// The decode bytes of one shape at `len` bytes: the text itself where that
/// is modified UTF-8 too, its encoded spelling otherwise.
fn bytes(shape: Shape, len: usize) -> Vec<u8> {
    let text = repeat(shape, len);
    if shape.utf8 {
        text.into_bytes()
    } else {
        Cesu8Buf::from(text.as_str()).into_bytes()
    }
}

/// Bytes that end in a bad sequence, the `reject` group's input.
fn invalid(len: usize) -> Vec<u8> {
    let mut bytes = repeat(ASCII, len).into_bytes();
    bytes[len - 1] = 0xff;
    bytes
}

/// One `decode` entry: `Cesu8::new` then `decode`, the pair a caller uses.
macro_rules! decode_bench {
    ($name:ident, $id:ident, $shape:expr, $len:expr) => {
        #[::iai_callgrind::library_benchmark]
        #[bench::$id(args = (bytes($shape, $len)))]
        fn $name(bytes: Vec<u8>) {
            let _ = black_box(Cesu8::new(&bytes).unwrap().decode());
        }
    };
}

/// One `encode` entry: encoding the canonical text.
macro_rules! encode_bench {
    ($name:ident, $id:ident, $shape:expr, $len:expr) => {
        #[::iai_callgrind::library_benchmark]
        #[bench::$id(args = (repeat($shape, $len)))]
        fn $name(text: String) {
            let _ = black_box(Cesu8::from_str(&text));
        }
    };
}

/// One `reject` entry: refusing bytes that end in a bad sequence.
macro_rules! reject_bench {
    ($name:ident, $id:ident, $len:expr) => {
        #[::iai_callgrind::library_benchmark]
        #[bench::$id(args = (invalid($len)))]
        fn $name(bytes: Vec<u8>) {
            let _ = black_box(Cesu8::new(&bytes).ok());
        }
    };
}

/// One `validate` entry: `Cesu8::new` alone.
macro_rules! validate_bench {
    ($name:ident, $id:ident, $shape:expr, $len:expr) => {
        #[::iai_callgrind::library_benchmark]
        #[bench::$id(args = (bytes($shape, $len)))]
        fn $name(bytes: Vec<u8>) {
            let _ = black_box(Cesu8::new(&bytes));
        }
    };
}

decode_bench!(decode_nanocesu8_ascii_small, ascii_small, ASCII, SMALL);
decode_bench!(
    decode_nanocesu8_unicode_small,
    unicode_small,
    UNICODE,
    SMALL
);
decode_bench!(decode_nanocesu8_nul_small, nul_small, NUL, SMALL);
decode_bench!(decode_nanocesu8_mutf8_small, mutf8_small, MUTF8, SMALL);
decode_bench!(decode_nanocesu8_nulls_small, nulls_small, NULLS, SMALL);
decode_bench!(
    decode_nanocesu8_surrogates_small,
    surrogates_small,
    SURROGATES,
    SMALL
);
decode_bench!(decode_nanocesu8_ascii_large, ascii_large, ASCII, LARGE);
decode_bench!(
    decode_nanocesu8_unicode_large,
    unicode_large,
    UNICODE,
    LARGE
);
decode_bench!(decode_nanocesu8_nul_large, nul_large, NUL, LARGE);
decode_bench!(decode_nanocesu8_mutf8_large, mutf8_large, MUTF8, LARGE);
decode_bench!(decode_nanocesu8_nulls_large, nulls_large, NULLS, LARGE);
decode_bench!(
    decode_nanocesu8_surrogates_large,
    surrogates_large,
    SURROGATES,
    LARGE
);

encode_bench!(encode_nanocesu8_ascii_small, ascii_small, ASCII, SMALL);
encode_bench!(
    encode_nanocesu8_unicode_small,
    unicode_small,
    UNICODE,
    SMALL
);
encode_bench!(encode_nanocesu8_nul_small, nul_small, NUL, SMALL);
encode_bench!(encode_nanocesu8_mutf8_small, mutf8_small, MUTF8, SMALL);
encode_bench!(encode_nanocesu8_nulls_small, nulls_small, NULLS, SMALL);
encode_bench!(
    encode_nanocesu8_surrogates_small,
    surrogates_small,
    SURROGATES,
    SMALL
);
encode_bench!(encode_nanocesu8_ascii_large, ascii_large, ASCII, LARGE);
encode_bench!(
    encode_nanocesu8_unicode_large,
    unicode_large,
    UNICODE,
    LARGE
);
encode_bench!(encode_nanocesu8_nul_large, nul_large, NUL, LARGE);
encode_bench!(encode_nanocesu8_mutf8_large, mutf8_large, MUTF8, LARGE);
encode_bench!(encode_nanocesu8_nulls_large, nulls_large, NULLS, LARGE);
encode_bench!(
    encode_nanocesu8_surrogates_large,
    surrogates_large,
    SURROGATES,
    LARGE
);

reject_bench!(reject_nanocesu8_invalid_small, invalid_small, SMALL);
reject_bench!(reject_nanocesu8_invalid_large, invalid_large, LARGE);

validate_bench!(validate_nanocesu8_ascii_small, ascii_small, ASCII, SMALL);
validate_bench!(
    validate_nanocesu8_unicode_small,
    unicode_small,
    UNICODE,
    SMALL
);
validate_bench!(validate_nanocesu8_nul_small, nul_small, NUL, SMALL);
validate_bench!(validate_nanocesu8_mutf8_small, mutf8_small, MUTF8, SMALL);
validate_bench!(validate_nanocesu8_nulls_small, nulls_small, NULLS, SMALL);
validate_bench!(
    validate_nanocesu8_surrogates_small,
    surrogates_small,
    SURROGATES,
    SMALL
);
validate_bench!(validate_nanocesu8_ascii_large, ascii_large, ASCII, LARGE);
validate_bench!(
    validate_nanocesu8_unicode_large,
    unicode_large,
    UNICODE,
    LARGE
);
validate_bench!(validate_nanocesu8_nul_large, nul_large, NUL, LARGE);
validate_bench!(validate_nanocesu8_mutf8_large, mutf8_large, MUTF8, LARGE);
validate_bench!(validate_nanocesu8_nulls_large, nulls_large, NULLS, LARGE);
validate_bench!(
    validate_nanocesu8_surrogates_large,
    surrogates_large,
    SURROGATES,
    LARGE
);

library_benchmark_group!(
    name = decode_entries;
    benchmarks =
        decode_nanocesu8_ascii_small,
        decode_nanocesu8_unicode_small,
        decode_nanocesu8_nul_small,
        decode_nanocesu8_mutf8_small,
        decode_nanocesu8_nulls_small,
        decode_nanocesu8_surrogates_small,
        decode_nanocesu8_ascii_large,
        decode_nanocesu8_unicode_large,
        decode_nanocesu8_nul_large,
        decode_nanocesu8_mutf8_large,
        decode_nanocesu8_nulls_large,
        decode_nanocesu8_surrogates_large
);

library_benchmark_group!(
    name = encode_entries;
    benchmarks =
        encode_nanocesu8_ascii_small,
        encode_nanocesu8_unicode_small,
        encode_nanocesu8_nul_small,
        encode_nanocesu8_mutf8_small,
        encode_nanocesu8_nulls_small,
        encode_nanocesu8_surrogates_small,
        encode_nanocesu8_ascii_large,
        encode_nanocesu8_unicode_large,
        encode_nanocesu8_nul_large,
        encode_nanocesu8_mutf8_large,
        encode_nanocesu8_nulls_large,
        encode_nanocesu8_surrogates_large
);

library_benchmark_group!(
    name = reject_entries;
    benchmarks =
        reject_nanocesu8_invalid_small,
        reject_nanocesu8_invalid_large
);

library_benchmark_group!(
    name = validate_entries;
    benchmarks =
        validate_nanocesu8_ascii_small,
        validate_nanocesu8_unicode_small,
        validate_nanocesu8_nul_small,
        validate_nanocesu8_mutf8_small,
        validate_nanocesu8_nulls_small,
        validate_nanocesu8_surrogates_small,
        validate_nanocesu8_ascii_large,
        validate_nanocesu8_unicode_large,
        validate_nanocesu8_nul_large,
        validate_nanocesu8_mutf8_large,
        validate_nanocesu8_nulls_large,
        validate_nanocesu8_surrogates_large
);

/// `--cache-sim=no` turns off callgrind's cache simulation, which the report
/// does not use: the instruction count stays exact and the run gets much
/// faster. The same config as the comparison bench.
fn config() -> LibraryBenchmarkConfig {
    let mut config = LibraryBenchmarkConfig::default();
    config.tool(Callgrind::with_args(["--cache-sim=no"]));
    config
}

main!(
    config = config();
    library_benchmark_groups =
        decode_entries, encode_entries, reject_entries, validate_entries
);
