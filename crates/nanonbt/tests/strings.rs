//! Zero-copy string handling through `Read`, `Write` and the derives.

use std::borrow::Cow;

use nanonbt::{
    Cesu8, Cesu8Buf, DeOpts, FromNBT, Read, Reader, ToNBT, Write, Writer, from_bytes, to_bytes,
};

#[test]
fn read_cesu8_borrows_the_document_bytes() {
    // A `TAG_String` payload: `u16` length 2, then `C0 80`.
    let mut reader = Reader::new(&[0, 2, 0xc0, 0x80], DeOpts::new());
    let text = reader.read_cesu8().unwrap();
    assert!(matches!(text, Cow::Borrowed(_)));
    assert_eq!(text.as_bytes(), &[0xc0, 0x80]);
    assert_eq!(&*text.decode(), "\0");

    // `read_str` still has to own what is not UTF-8.
    let mut reader = Reader::new(&[0, 2, 0xc0, 0x80], DeOpts::new());
    let text = reader.read_str().unwrap();
    assert!(matches!(text, Cow::Owned(_)));
    assert_eq!(&*text, "\0");
}

#[test]
fn read_cesu8_refuses_invalid_bytes() {
    let mut reader = Reader::new(&[0, 2, 0xc0, 0x81], DeOpts::new());
    assert_eq!(
        reader.read_cesu8().unwrap_err().to_string(),
        "invalid nbt string: nonunicode"
    );
}

/// Java's spelling is the only one read: plain UTF-8 with a raw NUL is
/// refused, where fastnbt takes it because the bytes are valid UTF-8.
#[test]
fn read_str_requires_java_spelling() {
    let mut reader = Reader::new(&[0, 3, b'a', 0, b'b'], DeOpts::new());
    assert_eq!(
        reader.read_str().unwrap_err().to_string(),
        "invalid nbt string: nonunicode"
    );
    let document = [
        0x0a, 0x00, 0x00, 0x08, 0x00, 0x01, b'x', 0, 3, b'a', 0, b'b', 0x00,
    ];
    assert!(fastnbt::from_bytes::<fastnbt::Value>(&document).is_ok());

    // The modified spelling of the same text is accepted.
    let mut reader = Reader::new(&[0, 4, b'a', 0xc0, 0x80, b'b'], DeOpts::new());
    assert_eq!(&*reader.read_str().unwrap(), "a\0b");
}

#[test]
fn write_cesu8_writes_the_exact_bytes() {
    let mut out = Vec::new();
    Writer::new(&mut out)
        .write_cesu8(Cesu8::new(&[0xc0, 0x80]).unwrap())
        .unwrap();
    assert_eq!(out, [0, 2, 0xc0, 0x80]);

    // `write_str` canonicalizes, which for a NUL is the same two bytes.
    let mut out = Vec::new();
    Writer::new(&mut out).write_str("\0").unwrap();
    assert_eq!(out, [0, 2, 0xc0, 0x80]);
}

#[test]
fn both_string_writers_refuse_over_65535_bytes() {
    let long = vec![b'a'; 65_536];
    let text = Cesu8::new(&long).unwrap();
    let mut out = Vec::new();
    assert_eq!(
        Writer::new(&mut out)
            .write_cesu8(text)
            .unwrap_err()
            .to_string(),
        "string longer than 65535 bytes"
    );
    let mut out = Vec::new();
    assert_eq!(
        Writer::new(&mut out)
            .write_str(&"a".repeat(65_536))
            .unwrap_err()
            .to_string(),
        "string longer than 65535 bytes"
    );
}

#[derive(FromNBT, ToNBT, PartialEq, Debug)]
struct Strings<'a> {
    borrowed: &'a Cesu8,
    owned: Cesu8Buf,
    cow: Cow<'a, Cesu8>,
}

#[test]
fn cesu8_fields_round_trip_without_allocating() {
    let value = Strings {
        borrowed: Cesu8::new(b"a\xc0\x80b").unwrap(),
        owned: Cesu8Buf::from("c\0d"),
        cow: Cow::Borrowed(Cesu8::new(b"e\xed\xa0\x81\xed\xb0\x81f").unwrap()),
    };
    let bytes = to_bytes(&value).unwrap();
    let back = from_bytes::<Strings<'_>>(&bytes).unwrap();
    assert_eq!(back, value);
    assert!(matches!(back.cow, Cow::Borrowed(_)));
    assert_eq!(&*back.borrowed.decode(), "a\0b");
    // `From<&str>` encodes, so the buffer holds Java's spelling.
    assert_eq!(back.owned.as_bytes(), b"c\xc0\x80d");
}
