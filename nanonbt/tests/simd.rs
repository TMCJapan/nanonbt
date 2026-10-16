//! Checks that native scalar serialization vectorizes its byte swap.
//!
//! The payload of `TAG_Short`, `TAG_Int`, `TAG_Long`, `TAG_Float` and
//! `TAG_Double` elements is big-endian, so writing native values means
//! swapping every element — in a plain `TAG_List` of any of them and in the
//! `TAG_Int_Array` / `TAG_Long_Array` routes `i32` and `i64` additionally get
//! (NBT has no short or float arrays). `nanonbt` keeps the swap in safe code —
//! the wire types express `to_be_bytes` — which leaves it to LLVM to find the
//! vector instructions. Whether it did is not visible in the source, so these
//! tests pin the generated LLVM IR down with [`ir_assert`]: `assert_ir!`
//! recompiles this test target at `opt-level=3` and matches the IR of the
//! referenced function, so a change that quietly degrades the bulk swap to a
//! per-element loop fails here instead of only showing up in a benchmark.
//!
//! The assertions follow the target features the test crate is compiled
//! with, so a machine that has the feature can pin a wider vector down as
//! well; `RUSTFLAGS` also reaches the IR build that `ir_assert!` runs, which
//! is what keeps the wider assertion meaningful:
//!
//! ```console
//! RUSTFLAGS="-C target-feature=+avx2" cargo test -p nanonbt --test simd
//! ```
//!
//! AVX-512 (`+avx512f,+avx512bw`) additionally needs a CPU that can run the
//! build scripts with those features; on a machine without it, the codegen
//! can still be inspected without running anything, by compiling the test
//! target (which instantiates the serializers) to IR and looking for the
//! widest `llvm.bswap.v*` the target admits.
//!
//! The assertions are limited to the architectures whose baseline has a
//! vector byte swap; everywhere else the test binary compiles empty.
#![cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]

use std::collections::HashMap;
use std::fmt;

use ir_assert::env::EnvSpec;
use ir_assert::{FunctionIr, Predicate, assert_ir};
use nanonbt::route::IntArray;
#[cfg(any(target_arch = "aarch64", target_feature = "ssse3"))]
use nanonbt::route::LongArray;
use nanonbt::{SerializeNbt, SerializeNbtArray};

/// Passes when the function's IR contains a vector byte swap.
///
/// `llvm.bswap.v*` is the intrinsic LLVM generates when it swaps several
/// integers with one instruction instead of one `bswap` per element; the
/// element-wise form is `llvm.bswap.i32` or `llvm.bswap.i64`, which do not
/// match the `v` prefix.
struct VectorByteSwap;

impl fmt::Display for VectorByteSwap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("vector_byte_swap()")
    }
}

impl Predicate for VectorByteSwap {
    fn evaluate(
        &self,
        function: &str,
        functions: &HashMap<String, FunctionIr>,
        _env: &EnvSpec,
    ) -> Result<(), String> {
        let ir = functions
            .get(function)
            .ok_or_else(|| format!("no IR for `{function}`"))?;
        if ir.raw.contains("llvm.bswap.v") {
            Ok(())
        } else {
            Err("the IR holds no vector byte swap (`llvm.bswap.v*`)".to_owned())
        }
    }
}

/// Baseline `x86-64` (SSE2) lowers `<4 x i32>` byte swaps to a
/// `punpck`/`pshuf` sequence and baseline `aarch64` has `rev`, so both
/// vectorize 32-bit chunks without any target features.
#[test]
fn int_array_bulk_swap_is_vectorized() {
    assert_ir!(
        ir_assert::predicate::opt3 & VectorByteSwap,
        <[i32] as SerializeNbtArray<IntArray>>::serialize_nbt_array::<Vec<u8>>,
    );
}

/// A 64-bit element makes the backend pickier: baseline `aarch64` swaps two
/// lanes per `rev64`, while `x86-64` only prefers a vector shuffle from
/// SSSE3 (`pshufb`) upwards — its SSE2 lowering is several shuffles per
/// lane pair, so LLVM keeps a baseline `x86-64` build scalar. With SSSE3
/// compiled in (`+ssse3` up to `+avx512f,+avx512bw`, which also enables
/// SSSE3), the swap vectorizes and this test runs there too.
#[cfg(any(target_arch = "aarch64", target_feature = "ssse3"))]
#[test]
fn long_array_bulk_swap_is_vectorized() {
    assert_ir!(
        ir_assert::predicate::opt3 & VectorByteSwap,
        <[i64] as SerializeNbtArray<LongArray>>::serialize_nbt_array::<Vec<u8>>,
    );
}

/// A plain `TAG_List` of `i32` — no `#[nbt(array)]` attribute — goes through
/// the same bulk swap, which is the element hook `Vec<i32>` calls.
#[test]
fn int_list_bulk_swap_is_vectorized() {
    assert_ir!(
        ir_assert::predicate::opt3 & VectorByteSwap,
        <i32 as SerializeNbt>::serialize_nbt_elements::<Vec<u8>>,
    );
}

/// The 64-bit hook follows the same target-feature rule as the array route.
#[cfg(any(target_arch = "aarch64", target_feature = "ssse3"))]
#[test]
fn long_list_bulk_swap_is_vectorized() {
    assert_ir!(
        ir_assert::predicate::opt3 & VectorByteSwap,
        <i64 as SerializeNbt>::serialize_nbt_elements::<Vec<u8>>,
    );
}

/// A plain `TAG_List` of `i16` goes through the same bulk hook.
#[test]
fn short_list_bulk_swap_is_vectorized() {
    assert_ir!(
        ir_assert::predicate::opt3 & VectorByteSwap,
        <i16 as SerializeNbt>::serialize_nbt_elements::<Vec<u8>>,
    );
}

/// The `f32` list hook swaps the bit patterns of a whole chunk.
#[test]
fn float_list_bulk_swap_is_vectorized() {
    assert_ir!(
        ir_assert::predicate::opt3 & VectorByteSwap,
        <f32 as SerializeNbt>::serialize_nbt_elements::<Vec<u8>>,
    );
}

/// The `f64` hook follows the 64-bit vector-swap rule of the array route.
#[cfg(any(target_arch = "aarch64", target_feature = "ssse3"))]
#[test]
fn double_list_bulk_swap_is_vectorized() {
    assert_ir!(
        ir_assert::predicate::opt3 & VectorByteSwap,
        <f64 as SerializeNbt>::serialize_nbt_elements::<Vec<u8>>,
    );
}
