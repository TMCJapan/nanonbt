#![allow(clippy::items_after_statements)]
//! `nanocesu8` agrees with the `cesu8` crate fastnbt is built on.

use nanocesu8::Cesu8;

const SAMPLES: &[&str] = &[
    "",
    "plain ascii",
    "a\0b",
    "\0",
    "é日本",
    "\u{7ff}\u{800}\u{ffff}",
    "\u{10000}",
    "M日\u{10401}\u{7f}",
    "\u{10ffff}\0\u{1f600}",
];

const ENCODED: &[&[u8]] = &[
    b"",
    b"plain",
    // Raw NUL and four-byte sequences are valid UTF-8, so they pass.
    b"a\0b",
    &[0xf0, 0x9f, 0x98, 0x80],
    // Modified UTF-8 forms.
    &[0xc0, 0x80],
    &[b'x', 0xc0, 0x80, b'y'],
    &[0xed, 0xa0, 0x81, 0xed, 0xb0, 0x81],
    &[
        0x4d, 0xe6, 0x97, 0xa5, 0xed, 0xa0, 0x81, 0xed, 0xb0, 0x81, 0x7f,
    ],
    // Mixing raw NUL into modified UTF-8.
    &[0x00, 0xc0, 0x80],
    // Mixing a four-byte sequence into modified UTF-8.
    &[0xc0, 0x80, 0xf0, 0x9f, 0x98, 0x80],
    // Broken forms.
    &[0xc0],
    &[0xc0, 0x81],
    &[0xc1, 0x80],
    &[0x80],
    &[0xed, 0xa0, 0x81],
    &[0xed, 0xa0, 0x81, 0x41],
    &[0xed, 0xa0, 0x81, 0xed, 0xa0, 0x81],
    &[0xed, 0xb0, 0x81, 0xed, 0xa0, 0x81],
    &[0xe0, 0x80, 0x80],
    &[0xe0, 0x9f, 0xbf, 0xc0, 0x80],
    &[0xed, 0xbf, 0xbf, 0xc0, 0x80],
    &[0xc2, 0x41, 0xc0, 0x80],
    &[0xf4, 0x90, 0x80, 0x80],
    &[0xff],
];

#[test]
fn decoding_matches_java_cesu8() {
    for bytes in ENCODED {
        assert_eq!(
            nanocesu8::from_java_cesu8(bytes).ok().as_deref(),
            cesu8::from_java_cesu8(bytes).ok().as_deref(),
            "{bytes:02x?}"
        );
    }
}

#[test]
fn encoding_matches_java_cesu8() {
    for text in SAMPLES {
        assert_eq!(
            &*nanocesu8::to_java_cesu8(text),
            &*cesu8::to_java_cesu8(text),
            "{text:?}"
        );
    }
}

#[test]
fn validation_and_iteration_match_java_cesu8() {
    for bytes in ENCODED {
        let ours = Cesu8::new(bytes);
        let theirs = cesu8::from_java_cesu8(bytes);
        assert_eq!(ours.is_ok(), theirs.is_ok(), "{bytes:02x?}");
        if let (Ok(ours), Ok(theirs)) = (ours, theirs) {
            assert_eq!(&*ours.decode(), &*theirs, "{bytes:02x?}");
            assert_eq!(ours.chars().collect::<String>().as_str(), &*theirs);
        }
    }
}
