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
//! struct writes every borrowed array back as the array it read, so its
//! throughput is the length of the input; only the short list, which has no
//! NBT array, writes one byte longer than it read.
//!
//! Every entry is one `#[library_benchmark]` function named
//! `<kind>_<target>_<id>` — `parse_nanonbt_serde_small` is
//! `parse/nanonbt-serde/small`. `iai-callgrind` runs each under callgrind and
//! files the instruction count under that function name and the id its
//! `#[bench]` attribute carries, so two runs count exactly the same work
//! wherever they run and compare without a noise band.
//! `examples/bench-summary.rs` rebuilds the report ids from those two names.
//!
//! The `Bench` workflow runs the suite once per platform and posts one
//! comment with a section per platform: callgrind on Linux x86_64 and
//! aarch64, wasmi fuel on wasm32, and a run-every-entry smoke test on Windows
//! and macOS; the two callgrind jobs also run the `nanocesu8` target of
//! `benches/nanocesu8.rs` in the same pass. See
//! `.github/workflows/bench.yml` and `examples/bench-summary.rs`. The same entry files also compile into the
//! crate's library, whose runtime registry drives the wasm and smoke
//! runners; see `src/lib.rs`. Run the iai side locally with
//! `cd crates/nanonbt-bench && cargo +nightly bench`; a recent valgrind and a
//! matching `iai-callgrind-runner` must be on the path.
//! Add `--features nanonbt/simd` to run the `nanonbt` entries on the
//! vectorized paths. The derived entries send their names through `hashify`,
//! which is on by default so the name pair measures the lookup;
//! `--no-default-features --features fastnbt,pumpkin-nbt,simdnbt` turns it
//! back into a plain `match`. All three of `fastnbt`, `pumpkin-nbt`, and
//! `simdnbt` are enabled by default.
//!
//! Two entries need a note. `simdnbt-borrow` keeps a tape over the input and
//! decodes strings lazily; its `int_array` and `long_array` accessors copy, so
//! those fields own their data where the rest borrows. `simdnbt-owned` reads
//! the tree first and copies what its accessors will not lend.

// Struct fields are named after their NBT keys, so that neither `serde` nor
// the `nanonbt` derive needs a rename; the derive's generated bindings then
// carry those names too.
#![allow(non_snake_case)]

use iai_callgrind::{Callgrind, LibraryBenchmarkConfig, main};

#[allow(unused_imports)]
use crate::macros::empty_group;

mod documents;
mod macros;
mod targets;

#[cfg(feature = "fastnbt")]
use targets::fastnbt::fastnbt_entries;
#[cfg(not(feature = "fastnbt"))]
empty_group!(fastnbt_entries);

#[cfg(feature = "simdnbt")]
use targets::simdnbt::simdnbt_entries;
#[cfg(not(feature = "simdnbt"))]
empty_group!(simdnbt_entries);

#[cfg(feature = "pumpkin-nbt")]
use targets::pumpkin::pumpkin_entries;
#[cfg(not(feature = "pumpkin-nbt"))]
empty_group!(pumpkin_entries);

use targets::nanonbt::nanonbt_entries;

/// `--cache-sim=no` turns off callgrind's cache simulation, which the report
/// does not use: the instruction count stays exact and the run gets much
/// faster.
fn config() -> LibraryBenchmarkConfig {
    let mut config = LibraryBenchmarkConfig::default();
    config.tool(Callgrind::with_args(["--cache-sim=no"]));
    config
}

main!(
    config = config();
    library_benchmark_groups =
        nanonbt_entries, fastnbt_entries, simdnbt_entries, pumpkin_entries
);
