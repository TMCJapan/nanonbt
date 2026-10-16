extern crate alloc;
use nanonbt::route::{ByteArray, IntArray};
use nanonbt::{
    DeserializeNbt, DeserializeNbtArray, DeserializeNbtArrayRef, Error, I32Be, Read, SerializeNbt,
    SliceReader, Tag, VecReader, from_bytes, to_bytes,
};

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct Primitives {
    byte: i8,
    short: i16,
    int: i32,
    long: i64,
    float: f32,
    double: f64,
    flag: bool,
    text: String,
    long_text: String,
}

#[test]
fn primitives_roundtrip() {
    let value = Primitives {
        byte: -3,
        short: 1000,
        int: -7,
        long: 1 << 40,
        float: 1.5,
        double: -2.25,
        flag: true,
        text: "こんにちは".to_owned(),
        long_text: "x".repeat(1000),
    };
    let bytes = to_bytes(&value).unwrap();
    assert_eq!(bytes[0], Tag::Compound.as_u8());
    assert_eq!(&bytes[1..3], &[0, 0], "root name is empty");
    let decoded: Primitives = from_bytes(&bytes).unwrap();
    assert_eq!(value, decoded);
}

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct Collections {
    numbers: Vec<i32>,
    empty: Vec<i8>,
    nested: Vec<Vec<i32>>,
    #[nbt(array = byte)]
    bytes: Vec<u8>,
    #[nbt(array)]
    signed_bytes: Vec<i8>,
    #[nbt(array = int)]
    ints: Vec<i32>,
    #[nbt(array)]
    inferred_ints: Vec<i32>,
    #[nbt(array = long)]
    longs: Vec<i64>,
    #[nbt(array = int)]
    wire_ints: Vec<I32Be>,
}

#[test]
fn collections_roundtrip() {
    let value = Collections {
        numbers: vec![1, -1],
        empty: Vec::new(),
        nested: vec![vec![], vec![7]],
        bytes: vec![0, 128, 255],
        signed_bytes: vec![-128, 0, 127],
        ints: vec![10, -20],
        inferred_ints: vec![30],
        longs: vec![1 << 40, -(1 << 40)],
        wire_ints: vec![I32Be::new(5), I32Be::new(-5)],
    };
    let bytes = to_bytes(&value).unwrap();
    let decoded: Collections = from_bytes(&bytes).unwrap();
    assert_eq!(value, decoded);
}

#[derive(SerializeNbt, DeserializeNbt)]
struct Tiny {
    #[nbt(array = int)]
    v: Vec<i32>,
}

#[test]
fn array_payload_layout_matches_the_spec() {
    let bytes = to_bytes(&Tiny { v: vec![1, -2] }).unwrap();
    #[rustfmt::skip]
    let expected: Vec<u8> = vec![
        Tag::Compound.as_u8(), 0, 0,            // root compound, empty name
        Tag::IntArray.as_u8(), 0, 1, b'v',      // field tag + name
        0, 0, 0, 2,                             // array length
        0, 0, 0, 1,                             // 1
        0xff, 0xff, 0xff, 0xfe,                 // -2
        Tag::End.as_u8(),                       // end of compound
    ];
    assert_eq!(bytes, expected);
}

#[test]
fn list_payload_layout_matches_the_spec() {
    let bytes = to_bytes(&vec![1i32, -2]).unwrap();
    #[rustfmt::skip]
    let expected: Vec<u8> = vec![
        Tag::List.as_u8(), 0, 0,    // root list, empty name
        Tag::Int.as_u8(),           // list element tag
        0, 0, 0, 2,                 // list length
        0, 0, 0, 1,                 // 1
        0xff, 0xff, 0xff, 0xfe,     // -2
    ];
    assert_eq!(bytes, expected);
}

#[test]
fn empty_lists_use_the_end_element_tag() {
    let bytes = to_bytes(&Vec::<i32>::new()).unwrap();
    assert_eq!(
        bytes,
        [Tag::List.as_u8(), 0, 0, Tag::End.as_u8(), 0, 0, 0, 0]
    );
    let decoded: Vec<i32> = from_bytes(&bytes).unwrap();
    assert_eq!(decoded, []);
}

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq, Default)]
struct Skipping {
    #[nbt(skip)]
    cache: i64,
    #[nbt(rename = "n")]
    name: String,
}

#[test]
fn skip_and_rename_work() {
    let value = Skipping {
        cache: 99,
        name: "abc".to_owned(),
    };
    let bytes = to_bytes(&value).unwrap();
    assert!(!bytes.windows(5).any(|window| window == b"cache"));
    let decoded: Skipping = from_bytes(&bytes).unwrap();
    assert_eq!(
        decoded,
        Skipping {
            cache: 0,
            name: "abc".to_owned(),
        }
    );
}

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct Renamed {
    #[nbt(rename = "名前\0\u{1f600}")]
    mixed: i8,
}

#[test]
fn renamed_names_use_modified_utf8_on_the_wire() {
    let value = Renamed { mixed: -1 };
    let bytes = to_bytes(&value).unwrap();
    // "名前" is its UTF-8 bytes, a NUL is the two bytes C0 80 and U+1F600 is
    // the surrogate pair ED A0 BD ED B8 80, as modified UTF-8 spells them.
    let name = b"\xe5\x90\x8d\xe5\x89\x8d\xc0\x80\xed\xa0\xbd\xed\xb8\x80";
    assert!(
        bytes.windows(name.len()).any(|window| window == name),
        "the name must be written as modified UTF-8"
    );
    assert!(
        !bytes.windows(4).any(|window| window == b"\xf0\x9f\x98\x80"),
        "the name must not be written as UTF-8"
    );
    assert_eq!(from_bytes::<Renamed>(&bytes).unwrap(), value);
}

#[derive(DeserializeNbt, Debug, PartialEq)]
struct Face {
    #[nbt(rename = "😀")]
    face: i32,
}

#[test]
fn names_that_are_not_modified_utf8_are_rejected() {
    // The emoji written as its plain UTF-8, which modified UTF-8 refuses.
    // The rename is longer than it, so the name is read and validated
    // rather than skipped.
    let mut bytes = vec![Tag::Compound.as_u8(), 0, 0, Tag::Int.as_u8(), 0, 4];
    bytes.extend_from_slice("\u{1f600}".as_bytes());
    bytes.extend_from_slice(&[0, 0, 0, 1, Tag::End.as_u8()]);
    assert_eq!(from_bytes::<Face>(&bytes), Err(Error::InvalidUtf8));
}

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct Wrapper<T> {
    value: T,
    count: i32,
}

#[test]
fn generic_structs_roundtrip() {
    let ints = Wrapper {
        value: 1i64 << 40,
        count: 2,
    };
    let decoded: Wrapper<i64> = from_bytes(&to_bytes(&ints).unwrap()).unwrap();
    assert_eq!(ints, decoded);

    let strings = Wrapper {
        value: vec!["a".to_owned(), "b".to_owned()],
        count: 2,
    };
    let decoded: Wrapper<Vec<String>> = from_bytes(&to_bytes(&strings).unwrap()).unwrap();
    assert_eq!(strings, decoded);
}

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct Meters(#[nbt(array = int)] Vec<i32>);

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct Name(String);

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct Marker;

#[test]
fn newtype_and_unit_structs_roundtrip() {
    let meters = Meters(vec![1, 2, 3]);
    assert_eq!(
        to_bytes(&meters).unwrap()[0],
        Tag::IntArray.as_u8(),
        "newtype arrays are transparent"
    );
    let decoded: Meters = from_bytes(&to_bytes(&meters).unwrap()).unwrap();
    assert_eq!(meters, decoded);

    let name = Name("nanonbt".to_owned());
    let decoded: Name = from_bytes(&to_bytes(&name).unwrap()).unwrap();
    assert_eq!(name, decoded);

    let marker = Marker;
    assert_eq!(
        to_bytes(&marker).unwrap(),
        [Tag::Compound.as_u8(), 0, 0, Tag::End.as_u8()]
    );
    let _: Marker = from_bytes(&to_bytes(&marker).unwrap()).unwrap();
}

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct Line {
    start: Point,
    end: Point,
}

#[test]
fn nested_owned_structs_roundtrip() {
    let line = Line {
        start: Point { x: 1, y: 2 },
        end: Point { x: -3, y: 4 },
    };
    let decoded: Line = from_bytes(&to_bytes(&line).unwrap()).unwrap();
    assert_eq!(line, decoded);
}

#[test]
fn vec_reader_supports_owned_decoding() {
    let bytes = to_bytes(&Tiny { v: vec![3, 4] }).unwrap();
    let mut reader = VecReader::new(bytes);

    assert_eq!(reader.read_u8().unwrap(), Tag::Compound.as_u8());
    nanonbt::read_string(&mut reader).unwrap();
    let decoded = Tiny::deserialize_nbt(&mut reader).unwrap();
    assert_eq!(decoded.v, [3, 4]);

    // The same payload can be decoded through the owned array traits.
    let payload = [0, 0, 0, 1, 0, 0, 0, 9];
    let mut reader = VecReader::new(payload.to_vec());
    let ints =
        <Vec<i32> as DeserializeNbtArray<IntArray>>::deserialize_nbt_array(&mut reader).unwrap();
    assert_eq!(ints, [9]);

    let payload = [0, 0, 0, 3, 1, 2, 3];
    let mut reader = VecReader::new(payload.to_vec());
    let bytes =
        <Vec<u8> as DeserializeNbtArray<ByteArray>>::deserialize_nbt_array(&mut reader).unwrap();
    assert_eq!(bytes, [1, 2, 3]);
}

#[test]
fn borrowed_array_decoding_matches_owned() {
    #[derive(SerializeNbt)]
    struct Ints(#[nbt(array = int)] Vec<i32>);

    let bytes = to_bytes(&Ints(vec![8, -8])).unwrap();
    let mut reader = SliceReader::new(&bytes);
    reader.skip(3).unwrap(); // root tag + empty root name
    let borrowed =
        <&[I32Be] as DeserializeNbtArrayRef<IntArray>>::deserialize_nbt_array_ref(&mut reader)
            .unwrap();
    assert_eq!(borrowed.len(), 2);
    assert_eq!(borrowed[0].get(), 8);
    assert_eq!(borrowed[1].get(), -8);
    let start = bytes.as_ptr() as usize;
    assert!((start..start + bytes.len()).contains(&(borrowed.as_ptr() as usize)));
}
