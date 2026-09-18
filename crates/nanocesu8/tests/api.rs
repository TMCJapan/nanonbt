//! The `Cesu8`, `Cesu8Buf` and `Chars` API.

use std::{borrow::Cow, collections::HashMap, mem};

use nanocesu8::{Cesu8, Cesu8Buf};

/// `a`, NUL as `C0 80`, and U+10401 as a surrogate pair.
const MODIFIED: &[u8] = &[b'a', 0xc0, 0x80, 0xed, 0xa0, 0x81, 0xed, 0xb0, 0x81];
const MODIFIED_TEXT: &str = "a\0\u{10401}";

#[test]
fn layout_is_transparent() {
    let text = Cesu8::new(MODIFIED).unwrap();
    assert_eq!(mem::size_of_val(text), text.len());
    assert_eq!(text.as_bytes().as_ptr(), MODIFIED.as_ptr());
    assert_eq!(text.len(), MODIFIED.len());
    assert!(!text.is_empty());
    assert!(Cesu8::new(b"").unwrap().is_empty());
}

#[test]
fn chars_decode_without_allocating() {
    let text = Cesu8::new(MODIFIED).unwrap();
    assert_eq!(
        text.chars().collect::<Vec<char>>(),
        ['a', '\0', '\u{10401}']
    );
    assert_eq!(&*text.decode(), MODIFIED_TEXT);

    // Whole-input UTF-8: raw NUL and four-byte sequences pass, and both
    // `decode` and the bytes borrow.
    for s in ["", "plain", "a\0b", "🦀", "日\0🦀"] {
        let text = Cesu8::new(s.as_bytes()).unwrap();
        assert_eq!(text.chars().collect::<String>().as_str(), s);
        assert!(matches!(text.decode(), Cow::Borrowed(_)));
    }

    // Whole-input modified UTF-8 has to decode.
    assert!(matches!(text.decode(), Cow::Owned(_)));

    let mut chars = text.chars();
    assert_eq!(chars.clone().count(), 3);
    assert_eq!(chars.next(), Some('a'));
    assert_eq!(chars.next(), Some('\0'));
    assert_eq!(chars.next(), Some('\u{10401}'));
    assert_eq!(chars.next(), None);
    assert_eq!(chars.next(), None);
}

#[test]
fn equality_is_byte_equality() {
    let raw_nul = Cesu8::new(b"\0").unwrap();
    let encoded_nul = Cesu8::new(b"\xc0\x80").unwrap();
    assert_ne!(raw_nul, encoded_nul);
    assert_eq!(&*raw_nul.decode(), &*encoded_nul.decode());
    assert_eq!(*encoded_nul, b"\xc0\x80"[..]);
    assert_eq!(b"\xc0\x80"[..], *encoded_nul);

    let mut map: HashMap<Cesu8Buf, i32> = HashMap::new();
    map.insert(Cesu8Buf::from("key"), 1);
    assert_eq!(map.get(Cesu8::new(b"key").unwrap()), Some(&1));
}

#[test]
fn owned_round_trips() {
    let text = Cesu8::new(MODIFIED).unwrap();
    let owned: Cesu8Buf = text.to_owned();
    assert_eq!(owned.as_bytes(), MODIFIED);
    assert_eq!(owned.as_cesu8(), text);
    assert_eq!(&*owned.decode(), MODIFIED_TEXT);

    let cow: Cow<'_, Cesu8> = Cow::Borrowed(text);
    assert_eq!(cow.len(), MODIFIED.len());
    let cow: Cow<'_, Cesu8> = Cow::Owned(owned.clone());
    assert_eq!(cow.as_bytes(), MODIFIED);

    let bytes: Vec<u8> = owned.clone().into();
    assert_eq!(bytes, MODIFIED);
    assert_eq!(Cesu8Buf::from_vec(bytes).unwrap(), owned);
    assert_eq!(Cesu8Buf::from_bytes(MODIFIED).unwrap(), owned);
    // From a `String` keeps the raw UTF-8 spelling, so the bytes differ from
    // the canonical modified UTF-8 above while the text does not.
    let from_string = Cesu8Buf::from(String::from(MODIFIED_TEXT));
    assert_eq!(&*from_string.decode(), MODIFIED_TEXT);
    assert_ne!(from_string, owned);
    assert_eq!(owned.into_bytes(), MODIFIED);
}

#[test]
fn buffer_rejects_invalid_bytes() {
    assert!(Cesu8Buf::from_bytes(b"\xc0").is_err());
    assert!(Cesu8Buf::from_vec(vec![0xc0, 0x81]).is_err());
    assert_eq!(Cesu8Buf::new(), Cesu8Buf::default());
    assert!(Cesu8Buf::new().is_empty());
}

#[test]
fn push_str_keeps_the_buffer_valid() {
    let starts: [&[u8]; 5] = [
        b"",
        b"a\0b",
        b"a\xf0\x9f\xa6\x80b",
        b"\xc0\x80",
        b"\xed\xa0\x81\xed\xb0\x81",
    ];
    let appends = ["", "x", "\0", "🦀", "a\0\u{10401}"];
    for start in starts {
        for append in appends {
            let mut buf = Cesu8Buf::from_bytes(start).unwrap();
            let before = buf.decode().into_owned();
            buf.push_str(append);
            let valid = Cesu8::new(buf.as_bytes()).expect("still valid");
            let expected = format!("{before}{append}");
            assert_eq!(&*valid.decode(), expected);
            assert_eq!(&*buf.decode(), expected);
        }
    }
}
