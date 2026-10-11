//! Shared harness for the `nanonbt` comparison benchmarks.
//!
//! The crate measures the same logical NBT documents through three crates:
//! `nanonbt`, `fastnbt` and `simdnbt`. Each library has its own model module
//! with identical struct layouts ([`nanonbt_models`], [`fastnbt_models`],
//! [`simdnbt_models`]) so that encoding and decoding can be compared
//! apples-to-apples. [`fixtures`] generates the deterministic input data, and
//! [`meta`] describes the scenarios for the report generator.
//!
//! Run the benchmarks with:
//!
//! ```text
//! cargo bench -p nanonbt-bench --bench compare
//! cargo run -p nanonbt-bench --bin report
//! ```
//!
//! See `benchmarks/README.md` for the full workflow, including the Bencher
//! continuous benchmarking integration.

#![warn(missing_docs)]
// Benchmark and chart code reinterprets integer widths on purpose (checksums,
// fixtures, axes); the pedantic cast lints are noise here. The model types
// contain floats, so `Eq` cannot be derived for all of them.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::derive_partial_eq_without_eq
)]

pub mod fastnbt_models;
pub mod fixtures;
pub mod meta;
pub mod nanonbt_models;
pub mod simdnbt_models;

/// Mixes `value` into a running checksum.
///
/// Every read benchmark walks the decoded value and folds it into a checksum.
/// This keeps the measurements honest: a decoder that only borrows without
/// touching the payload does not get to skip the data's cache traffic, and
/// dead-code elimination cannot remove the decode work.
#[inline]
pub const fn checksum(acc: u64, value: u64) -> u64 {
    acc.rotate_left(7) ^ value.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Folds a float into a checksum through its bit pattern.
#[inline]
pub const fn checksum_f32(acc: u64, value: f32) -> u64 {
    checksum(acc, value.to_bits() as u64)
}

/// Folds a double into a checksum through its bit pattern.
#[inline]
pub const fn checksum_f64(acc: u64, value: f64) -> u64 {
    checksum(acc, value.to_bits())
}

/// Folds a string's bytes into a checksum.
#[inline]
pub fn checksum_str(acc: u64, value: &str) -> u64 {
    let mut acc = checksum(acc, value.len() as u64);
    for byte in value.bytes() {
        acc = checksum(acc, u64::from(byte));
    }
    acc
}
