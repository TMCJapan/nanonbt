//! Stand-ins for std functions that Kani cannot get through in reasonable
//! time, and proofs that the non-trivial ones agree with what they replace.
//!
//! - [`format`] and [`write`] drop formatting. Only error messages are
//!   formatted, and those are never compared.
//! - [`from_utf8`] and [`memchr`] are plain byte loops with the same results
//!   as core's word-at-a-time versions, whose loops depend on pointer
//!   alignment, which Kani leaves symbolic. Both decoders run `from_utf8` on
//!   every string; without the stub a symbolic byte through the two of them
//!   does not finish.

use core::str::Utf8Error;

#[expect(dead_code, reason = "used only by `kani::stub` attributes")]
pub fn format(_: core::fmt::Arguments<'_>) -> String {
    String::new()
}

#[expect(dead_code, reason = "used only by `kani::stub` attributes")]
pub fn write(_: &mut dyn core::fmt::Write, _: core::fmt::Arguments<'_>) -> core::fmt::Result {
    Ok(())
}

/// Every caller in the proven code looks only at whether this is `Ok`, so
/// one fixed error serves for all invalid input.
#[expect(invalid_from_utf8, reason = "an error is the point")]
const INVALID: Utf8Error = match core::str::from_utf8(&[0xff]) {
    Ok(_) => panic!(),
    Err(e) => e,
};

/// UTF-8 validation straight from Table 3-7 of the Unicode Standard.
pub fn from_utf8(v: &[u8]) -> Result<&str, Utf8Error> {
    let mut i = 0;
    while i < v.len() {
        let first = v[i];
        let width = match first {
            0x00..=0x7f => 1,
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => return Err(INVALID),
        };
        if v.len() - i < width {
            return Err(INVALID);
        }
        if width > 1 {
            let second_ok = match (first, v[i + 1]) {
                (0xe0, b) => matches!(b, 0xa0..=0xbf),
                (0xed, b) => matches!(b, 0x80..=0x9f),
                (0xf0, b) => matches!(b, 0x90..=0xbf),
                (0xf4, b) => matches!(b, 0x80..=0x8f),
                (_, b) => matches!(b, 0x80..=0xbf),
            };
            if !second_ok
                || (width > 2 && v[i + 2] & 0xc0 != 0x80)
                || (width > 3 && v[i + 3] & 0xc0 != 0x80)
            {
                return Err(INVALID);
            }
        }
        i += width;
    }
    // SAFETY: every sequence was just checked against Table 3-7.
    Ok(unsafe { core::str::from_utf8_unchecked(v) })
}

pub fn memchr(x: u8, text: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < text.len() {
        if text[i] == x {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// The stubs against the real functions, on every input of each length.
///
/// The lengths cover what the decoder hands them: one byte per string, plus
/// the empty and longer searches core's own code makes. Core's `from_utf8`
/// does not finish in 4 minutes on 16 symbolic bytes, even ASCII ones, so
/// that stub is checked only up to 8.
macro_rules! agreement {
    ($($name:ident: $check:ident($len:literal) unwind $unwind:tt;)*) => {
        $(
            #[kani::proof]
            #[kani::unwind($unwind)]
            fn $name() {
                $check(&kani::any::<[u8; $len]>());
            }
        )*
    };
}

fn utf8(bytes: &[u8]) {
    let ours = from_utf8(bytes).map(str::as_ptr);
    let real = core::str::from_utf8(bytes).map(str::as_ptr);
    assert!(ours.ok() == real.ok(), "from_utf8 disagrees");
}

fn byte_search(text: &[u8]) {
    let x: u8 = kani::any();
    assert!(
        memchr(x, text) == core::slice::memchr::memchr(x, text),
        "memchr disagrees"
    );
}

agreement! {
    utf8_1: utf8(1) unwind 3;
    utf8_2: utf8(2) unwind 4;
    utf8_3: utf8(3) unwind 5;
    utf8_4: utf8(4) unwind 6;
    utf8_8: utf8(8) unwind 10;
    memchr_0: byte_search(0) unwind 2;
    memchr_1: byte_search(1) unwind 3;
    memchr_2: byte_search(2) unwind 4;
    memchr_3: byte_search(3) unwind 5;
    memchr_16: byte_search(16) unwind 18;
    memchr_19: byte_search(19) unwind 21;
    memchr_20: byte_search(20) unwind 22;
}
