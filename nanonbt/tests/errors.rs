extern crate alloc;
use nanonbt::{DeserializeNbt, Error, SerializeNbt, Tag, from_bytes, to_bytes};

#[derive(SerializeNbt)]
struct Nested {
    value: i32,
}

#[derive(SerializeNbt)]
struct Everything {
    #[nbt(array = byte)]
    byte_array: Vec<u8>,
    #[nbt(array = int)]
    int_array: Vec<i32>,
    #[nbt(array = long)]
    long_array: Vec<i64>,
    byte: i8,
    short: i16,
    int: i32,
    long: i64,
    float: f32,
    double: f64,
    flag: bool,
    text: String,
    list: Vec<String>,
    nested: Nested,
}

#[derive(DeserializeNbt, Debug, PartialEq)]
struct Empty {}

#[test]
fn unknown_fields_of_every_tag_are_skipped() {
    let value = Everything {
        byte_array: vec![1, 2, 3],
        int_array: vec![4, 5],
        long_array: vec![6, 7],
        byte: -1,
        short: 2,
        int: 3,
        long: 4,
        float: 5.5,
        double: 6.5,
        flag: true,
        text: "ignored".to_owned(),
        list: vec!["a".to_owned(), "b".to_owned()],
        nested: Nested { value: 7 },
    };
    let bytes = to_bytes(&value).unwrap();
    let decoded: Empty = from_bytes(&bytes).unwrap();
    assert_eq!(decoded, Empty {});
}

#[derive(SerializeNbt)]
struct Source {
    a: i32,
}

#[derive(DeserializeNbt, Debug, PartialEq)]
struct Target {
    a: i32,
    b: i64,
}

#[test]
fn missing_fields_are_reported() {
    let bytes = to_bytes(&Source { a: 1 }).unwrap();
    assert_eq!(from_bytes::<Target>(&bytes), Err(Error::MissingField("b")));
}

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct IntOnly {
    a: i32,
}

#[derive(DeserializeNbt, Debug, PartialEq)]
struct StringOnly {
    a: String,
}

#[test]
fn field_tag_mismatches_are_reported() {
    let bytes = to_bytes(&IntOnly { a: 1 }).unwrap();
    assert_eq!(
        from_bytes::<StringOnly>(&bytes),
        Err(Error::InvalidTag {
            expected: Tag::String,
            actual: Tag::Int.as_u8(),
        })
    );
}

#[derive(DeserializeNbt, Debug, PartialEq)]
struct IntArrayHolder {
    #[nbt(array = int)]
    v: Vec<i32>,
}

#[test]
fn negative_array_lengths_are_rejected() {
    let bytes = [
        Tag::Compound.as_u8(),
        0,
        0,
        Tag::IntArray.as_u8(),
        0,
        1,
        b'v',
        0xff,
        0xff,
        0xff,
        0xff,
        Tag::End.as_u8(),
    ];
    assert_eq!(
        from_bytes::<IntArrayHolder>(&bytes),
        Err(Error::InvalidLength)
    );
}

#[derive(DeserializeNbt, Debug, PartialEq)]
struct ByteArrayHolder {
    #[nbt(array = byte)]
    v: Vec<u8>,
}

#[test]
fn oversized_length_prefixes_are_rejected_before_allocating() {
    // An 11-byte document claiming a 2 GiB byte array. The length is checked
    // against the remaining input instead of being reserved, so decoding fails
    // promptly rather than allocating gigabytes.
    let bytes = [
        Tag::Compound.as_u8(),
        0,
        0,
        Tag::ByteArray.as_u8(),
        0,
        1,
        b'v',
        0x7f,
        0xff,
        0xff,
        0xff,
    ];
    assert_eq!(
        from_bytes::<ByteArrayHolder>(&bytes),
        Err(Error::UnexpectedEof)
    );
}

#[test]
fn non_empty_lists_with_end_elements_are_rejected() {
    let bytes = [Tag::List.as_u8(), 0, 0, Tag::End.as_u8(), 0, 0, 0, 5];
    assert!(matches!(
        from_bytes::<Vec<i32>>(&bytes),
        Err(Error::InvalidData(_))
    ));
}

#[test]
fn list_element_tag_mismatches_are_reported() {
    let bytes = [
        Tag::List.as_u8(),
        0,
        0,
        Tag::Long.as_u8(),
        0,
        0,
        0,
        1,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        1,
    ];
    assert_eq!(
        from_bytes::<Vec<i32>>(&bytes),
        Err(Error::InvalidTag {
            expected: Tag::Int,
            actual: Tag::Long.as_u8(),
        })
    );
}

#[test]
fn truncated_payloads_are_rejected() {
    let bytes = [
        Tag::Compound.as_u8(),
        0,
        0,
        Tag::Int.as_u8(),
        0,
        1,
        b'a',
        0,
        0, // two of the four bytes of the value
    ];
    assert_eq!(from_bytes::<IntOnly>(&bytes), Err(Error::UnexpectedEof));
}

#[test]
fn unknown_tag_bytes_are_rejected() {
    assert_eq!(from_bytes::<i32>(&[13]), Err(Error::UnknownTag(13)));
}
