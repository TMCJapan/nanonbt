//! `nanocesu8` against the `cesu8` crate fastnbt is built on.
//!
//! The `cesu8` crate accepts plain UTF-8 with a raw NUL or a four-byte
//! sequence as well, so it is the more permissive of the two: everything
//! this crate accepts it accepts too, as the same text, and the two
//! disagree only about those two spellings.

use std::borrow::Cow;

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

/// Modified UTF-8, the only spelling this crate accepts.
const MODIFIED: &[&[u8]] = &[
    b"",
    b"plain",
    "é日本".as_bytes(),
    "\u{7ff}\u{800}\u{ffff}".as_bytes(),
    &[0xc0, 0x80],
    &[b'x', 0xc0, 0x80, b'y'],
    &[0xed, 0xa0, 0x81, 0xed, 0xb0, 0x81],
    &[
        0x4d, 0xe6, 0x97, 0xa5, 0xed, 0xa0, 0x81, 0xed, 0xb0, 0x81, 0x7f,
    ],
];

/// Plain UTF-8 that is not modified UTF-8: only the `cesu8` crate takes it.
const PLAIN: &[&[u8]] = &[
    b"\0",
    b"a\0b",
    &[0xf0, 0x9f, 0x98, 0x80],
    &[b'x', 0xf0, 0x9f, 0x98, 0x80, b'y'],
];

/// Neither crate accepts these.
const BROKEN: &[&[u8]] = &[
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
    // Mixing a raw NUL or a four-byte sequence into modified UTF-8.
    &[0x00, 0xc0, 0x80],
    &[0xc0, 0x80, 0xf0, 0x9f, 0x98, 0x80],
    &[0xff],
];

#[test]
fn decoding_matches_java_cesu8() {
    for bytes in MODIFIED {
        let ours = Cesu8::new(bytes).unwrap_or_else(|_| panic!("must accept: {bytes:02x?}"));
        let theirs = cesu8::from_java_cesu8(bytes).expect("both accept");
        assert_eq!(&*ours.decode(), &*theirs, "{bytes:02x?}");
        assert_eq!(
            ours.chars().collect::<String>().as_str(),
            &*theirs,
            "{bytes:02x?}"
        );
    }
}

#[test]
fn plain_utf8_is_refused_where_java_cesu8_accepts_it() {
    for bytes in PLAIN {
        assert!(Cesu8::new(bytes).is_err(), "{bytes:02x?}");
        assert!(
            matches!(cesu8::from_java_cesu8(bytes), Ok(Cow::Borrowed(_))),
            "{bytes:02x?}"
        );
    }
}

#[test]
fn broken_forms_are_refused_by_both() {
    for bytes in BROKEN {
        assert!(Cesu8::new(bytes).is_err(), "{bytes:02x?}");
        assert!(cesu8::from_java_cesu8(bytes).is_err(), "{bytes:02x?}");
    }
}

#[test]
fn encoding_matches_java_cesu8() {
    for text in SAMPLES {
        assert_eq!(
            Cesu8::from_str(text).as_bytes(),
            &*cesu8::to_java_cesu8(text),
            "{text:?}"
        );
    }
}
