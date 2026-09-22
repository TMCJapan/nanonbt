//! [Kani] proofs that nanocesu8 agrees with the `cesu8` crate 1.1.0.
//!
//! Everything here is `#[cfg(kani)]`; outside `cargo kani` this crate is
//! empty.
//!
//! # What is proven
//!
//! - `cesu8`: [`nanocesu8::Cesu8::new`] and its `decode` agree with the
//!   `cesu8` crate's `from_java_cesu8` on every single byte: the same text
//!   whenever both accept, and the only disagreement, a raw NUL, is the
//!   `cesu8` crate's alone. nanocesu8's decoder does not panic on any.
//! - `stubs`: the stand-ins that the harness runs with agree with the real
//!   functions on every input: `core::slice::memchr::memchr` at each length
//!   the harness gives it, and `core::str::from_utf8` up to 8 bytes.
//!
//! Error messages are never compared, which is what makes stubbing out
//! `core::fmt` sound.
//!
//! # What is not, and why
//!
//! CBMC, which Kani runs, only finishes when little is symbolic and little
//! lives on the heap. These did not finish, each tried with the stubs, with
//! 5 GB and 3 to 4 minutes:
//!
//! - A symbolic `char` through the encoder: the `cesu8` crate's encoder
//!   times out on its own, and nanocesu8's encoder and decoder together
//!   exhaust 5 GB.
//! - Decoding two symbolic bytes or more: both decoders together, or
//!   nanocesu8's on three, exhaust 5 GB; nanocesu8's alone on two times out.
//!
//! # Running
//!
//! Kani 0.67.0 builds with rustc 1.93 and refuses the workspace's
//! `rust-version`, so lower it first, in a scratch copy or a CI checkout.
//!
//! ```sh
//! sed -i 's/^rust-version = .*/rust-version = "1.93"/' Cargo.toml
//! cargo kani -p nanocesu8-kani -Z stubbing -Z unstable-options \
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
mod stubs;
