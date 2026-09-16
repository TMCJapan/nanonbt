//! [Kani] proofs that nanonbt agrees with fastnbt 2.6.3.
//!
//! Everything here is `#[cfg(kani)]`; outside `cargo kani` this crate is
//! empty.
//!
//! # What is proven
//!
//! - `ser`: for each of 48 shapes, `nanonbt::to_bytes` and
//!   `fastnbt::to_bytes` both succeed with the same bytes, or both fail, for
//!   every value. A shape is a serde type whose structure (fields, lengths,
//!   names) is fixed and whose values (numbers, bools, chars, the choice of
//!   `Some` or `None`, of a unit or tuple variant) are symbolic. Each
//!   harness also asserts which outcome it is, so that no shape passes by
//!   both crates refusing it. The shapes cover every primitive, `Option` in
//!   and between entries, `Vec`s of 0 to 2 scalars, arrays, lists of lists,
//!   a struct in a struct, unit and tuple variants, `serialize_bytes`, an
//!   array token after an entry, and the refusals: roots that are not
//!   compounds, units, newtype and struct variants, a non-string key,
//!   `None` in a list. Separately, for `ByteArray`, `IntArray` and
//!   `LongArray` of 0 to 2 symbolic elements, in an entry and at the root,
//!   nanonbt's type and fastnbt's type through nanonbt give what fastnbt's
//!   type through fastnbt gives.
//! - `de`: for each of 37 documents, written by hand with fixed tags,
//!   names and lengths and symbolic payloads, `nanonbt::from_bytes` and
//!   `fastnbt::from_bytes` read the same value or both refuse it, and which
//!   of the two it is. The documents cover every scalar tag into its own
//!   Rust type and into converting ones (`bool` from each integer tag,
//!   `i64` from Int, `u8`, `u16`, `u32` and `i8` from signed tags, where
//!   the outcome depends on the symbolic value), lists of 0 to 2 Ints and
//!   the list of End that old chunks use for an empty one, both crates'
//!   array types from arrays of 0 to 2 elements, read through both crates,
//!   and the same arrays into a `Vec`, which both refuse, an int array of 4
//!   as `i128` and `u128`, a compound in a compound, network NBT, entries
//!   the struct has no field for, and `Option` present and absent.
//! - `value`: for scalars, `nanonbt::to_value` and `fastnbt::to_value` make
//!   the same value, and `from_value` reads their own `Value` into the same
//!   `T`: every scalar variant into its own type and into converting ones,
//!   including `char` from Int and the `as`-cast `u8` from a negative Byte,
//!   and an int array of 4 as `i128` and `u128` both ways. Where fastnbt
//!   panics instead (`to_value` of `None`, of a unit, a unit struct or a
//!   newtype variant) only nanonbt is checked: it returns an error.
//! - `cesu8`: nanonbt's decoder agrees with the `cesu8` crate's on every
//!   single byte.
//! - `stubs`: the stand-ins that all other harnesses run with agree with
//!   the real functions on every input: `core::slice::memchr::memchr` at
//!   each length the harnesses give it, `core::str::from_utf8` up to 8
//!   bytes, past which it only sees the array tokens, valid `&'static str`s.
//!
//! Error messages are never compared, which is what makes stubbing out
//! `core::fmt` sound.
//!
//! # What is not, and why
//!
//! CBMC, which Kani runs, only finishes when little is symbolic and little
//! lives on the heap. No harness here takes more than 80 seconds or 1.5 GB.
//! These did not finish at all, each tried with the stubs, with 5 to 8 GB
//! and 3 to 4 minutes:
//!
//! - Anything but the listed shapes at the listed lengths.
//! - A `String` field, even of one symbolic ASCII character, and a list of
//!   compounds, even a `Vec` of one struct: each crate alone finishes in
//!   under 30 seconds, but comparing their outputs exhausts 5 to 8 GB.
//!   `[Inner; 2]` times out instead.
//! - A `BTreeMap`, even of two one-byte keys.
//! - A symbolic `char` through CESU-8: the `cesu8` crate's encoder times
//!   out on its own, and nanonbt's encoder and decoder together exhaust
//!   5 GB.
//! - Decoding two symbolic bytes or more: both decoders together, or
//!   nanonbt's on three, exhaust 5 GB; nanonbt's alone on two times out.
//! - `Value::Compound`, whose fastnbt form is a `HashMap`, and
//!   `Value::List`: a `Vec` of either crate's `Value` does not finish in
//!   3 minutes, in `to_value` or in `from_value`.
//! - In `de`, symbolic tags, lengths or string contents, which are out of
//!   reach the way they are in `ser`; documents there are written by hand
//!   with fixed structure.
//! - The divergences nanonbt documents, such as strings over 65535 bytes,
//!   which no shape here reaches.
//!
//! # Running
//!
//! Kani 0.67.0 builds with rustc 1.93 and refuses the workspace's
//! `rust-version`, so lower it first, in a scratch copy or a CI checkout.
//! All 141 harnesses take about 19 minutes with four jobs:
//!
//! ```sh
//! sed -i 's/^rust-version = .*/rust-version = "1.93"/' Cargo.toml
//! cargo kani -p nanonbt-kani -Z stubbing -Z unstable-options \
//!     --harness-timeout 300 --output-format terse -j 4
//! ```
//!
//! [Kani]: https://model-checking.github.io/kani/

#![cfg_attr(kani, feature(slice_internals))]
#![cfg_attr(
    kani,
    expect(
        internal_features,
        reason = "to check the `memchr` stub against core's"
    )
)]

/// Declares proof harnesses with every stub of [`stubs`] applied.
#[cfg(kani)]
macro_rules! proofs {
    ($($(#[doc = $doc:literal])* fn $name:ident() unwind $unwind:tt $body:block)*) => {
        $(
            $(#[doc = $doc])*
            #[kani::proof]
            #[kani::unwind($unwind)]
            #[kani::stub(alloc::fmt::format, crate::stubs::format)]
            #[kani::stub(core::fmt::write, crate::stubs::write)]
            #[kani::stub(core::str::from_utf8, crate::stubs::from_utf8)]
            #[kani::stub(core::slice::memchr::memchr, crate::stubs::memchr)]
            fn $name() $body
        )*
    };
}

#[cfg(kani)]
mod cesu8;
#[cfg(kani)]
mod de;
#[cfg(kani)]
mod same;
#[cfg(kani)]
mod ser;
#[cfg(kani)]
mod stubs;
#[cfg(kani)]
mod value;
