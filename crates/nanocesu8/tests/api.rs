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

    // Plain text without a NUL or a non-BMP character is modified UTF-8 as
    // it is, so `new` accepts it and both `decode` and the bytes borrow.
    for s in ["", "plain", "é日本", "\u{7ff}\u{800}\u{ffff}"] {
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
fn from_str_encodes_and_borrows() {
    // Already spelled the modified way: the bytes are lent, not copied.
    let borrowed = Cesu8::from_str("plain");
    assert!(matches!(borrowed, Cow::Borrowed(_)));
    assert_eq!(borrowed.as_bytes(), b"plain");
    assert_eq!(borrowed.as_bytes().as_ptr(), "plain".as_ptr());

    // A NUL and a non-BMP character have no modified spelling in common
    // with UTF-8, so those are encoded and owned.
    let nul = Cesu8::from_str("a\0b");
    assert!(matches!(nul, Cow::Owned(_)));
    assert_eq!(nul.as_bytes(), b"a\xc0\x80b");
    let astral = Cesu8::from_str("\u{1f980}");
    assert_eq!(astral.as_bytes(), b"\xed\xa0\xbe\xed\xb6\x80");

    // The bytes decode to the text they came from, and back.
    for s in ["", "plain", "é日本", "a\0b", "\u{1f980}", "日\0\u{1f980}"] {
        let encoded = Cesu8::from_str(s);
        assert_eq!(&*encoded.decode(), s, "{s:?}");
        assert_eq!(encoded.chars().collect::<String>().as_str(), s, "{s:?}");
        assert_eq!(
            Cesu8::from_str(&encoded.decode()).as_bytes(),
            encoded.as_bytes(),
            "{s:?}"
        );
    }
}

#[test]
fn only_the_modified_spelling_is_accepted() {
    // A raw NUL, a four-byte sequence, an overlong form and a lone
    // surrogate are all refused.
    for bytes in [
        &b"\0"[..],
        &[0xf0, 0x9f, 0x98, 0x80],
        &[0xc0, 0x41],
        &[0xe0, 0x80, 0x80],
        &[0xed, 0xa0, 0x81],
        &[0xf4, 0x90, 0x80, 0x80],
    ] {
        assert!(Cesu8::new(bytes).is_err(), "{bytes:02x?}");
        assert!(Cesu8Buf::from_bytes(bytes).is_err(), "{bytes:02x?}");
    }
    assert_eq!(Cesu8::from_str("\0").as_bytes(), b"\xc0\x80");
    assert_eq!(
        Cesu8::from_str("a\0\u{1f980}").as_bytes(),
        b"a\xc0\x80\xed\xa0\xbe\xed\xb6\x80"
    );
}

#[test]
fn decode_bytes_validates_and_decodes_in_one_walk() {
    // Plain ASCII and other UTF-8 already spelled the modified way borrow.
    let borrowed = Cesu8::decode_bytes(b"plain").unwrap();
    assert!(matches!(borrowed, Cow::Borrowed(_)));
    assert_eq!(borrowed, "plain");
    assert_eq!(Cesu8::decode_bytes(b"").unwrap(), "");
    let borrowed = Cesu8::decode_bytes("é日本".as_bytes()).unwrap();
    assert!(matches!(borrowed, Cow::Borrowed(_)));
    assert_eq!(borrowed, "é日本");

    // Modified UTF-8 decodes into an owned string.
    let owned = Cesu8::decode_bytes(MODIFIED).unwrap();
    assert!(matches!(owned, Cow::Owned(_)));
    assert_eq!(owned, MODIFIED_TEXT);

    // The accepted bytes are exactly `Cesu8::new`'s.
    for bytes in [
        &b""[..],
        b"plain",
        b"\x7f",
        "é日本".as_bytes(),
        MODIFIED,
        b"\xc0\x80",
        &b"\0"[..],
        &[0xf0, 0x9f, 0x98, 0x80],
        &[0xc0, 0x41],
        &[0xe0, 0x80, 0x80],
        &[0xed, 0xa0, 0x81],
        &[0xf4, 0x90, 0x80, 0x80],
        &[0xff],
    ] {
        match (Cesu8::new(bytes), Cesu8::decode_bytes(bytes)) {
            (Ok(valid), Ok(text)) => assert_eq!(&*valid.decode(), &*text, "{bytes:02x?}"),
            (Err(_), Err(_)) => {}
            (valid, text) => panic!("disagreement on {bytes:02x?}: {valid:?} vs {text:?}"),
        }
    }
}

#[test]
fn equality_is_byte_equality() {
    let encoded_nul = Cesu8::new(b"\xc0\x80").unwrap();
    assert_eq!(encoded_nul, &*Cesu8Buf::from("\0"));
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
    // Both `From` spellings encode, so a `String` yields the same bytes.
    assert_eq!(Cesu8Buf::from(MODIFIED_TEXT), owned);
    assert_eq!(Cesu8Buf::from(String::from(MODIFIED_TEXT)), owned);
    assert_eq!(owned.into_bytes(), MODIFIED);
}

#[test]
fn buffer_rejects_invalid_bytes() {
    assert!(Cesu8Buf::from_bytes(b"\xc0").is_err());
    assert!(Cesu8Buf::from_vec(vec![0xc0, 0x81]).is_err());
    assert!(Cesu8Buf::from_bytes(b"\0").is_err());
    assert!(Cesu8Buf::from_vec(vec![0xf0, 0x9f, 0x98, 0x80]).is_err());
    assert_eq!(Cesu8Buf::new(), Cesu8Buf::default());
    assert!(Cesu8Buf::new().is_empty());
}

#[test]
fn push_str_keeps_the_buffer_valid() {
    let starts: [&[u8]; 5] = [
        b"",
        b"plain",
        b"a\xc0\x80b",
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
