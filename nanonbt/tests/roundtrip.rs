extern crate alloc;
use std::borrow::Cow;

use nanonbt::route::{ByteArray, IntArray};
use nanonbt::{
    DeserializeNbt, DeserializeNbtArray, DeserializeNbtArrayRef, Error, F32Be, F64Be, I16Be, I32Be,
    I64Be, Read, SerializeNbt, SliceReader, Tag, VecReader, Write, from_bytes, to_bytes,
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
fn native_list_payloads_are_big_endian() {
    // The longer payloads cross a 64-byte chunk, so the bulk conversion and
    // the ragged per-element tail both run; the expected bytes come straight
    // from `to_be_bytes`, which is the spelling the chunk conversion uses.
    macro_rules! check {
        ($tag:expr, $values:expr) => {{
            let values = $values;
            let bytes = to_bytes(&values).unwrap();
            let mut expected = vec![Tag::List.as_u8(), 0, 0, $tag.as_u8()];
            expected.extend_from_slice(&i32::try_from(values.len()).unwrap().to_be_bytes());
            for value in &values {
                expected.extend_from_slice(&value.to_be_bytes());
            }
            assert_eq!(bytes, expected);
        }};
    }

    check!(
        Tag::Short,
        (0u8..40)
            .map(i16::from)
            .chain([-1, i16::MIN, i16::MAX])
            .collect::<Vec<_>>()
    );
    check!(
        Tag::Float,
        (0u8..20)
            .map(f32::from)
            .chain([-0.0, f32::INFINITY, f32::NAN])
            .collect::<Vec<_>>()
    );
    check!(
        Tag::Double,
        (0u8..12)
            .map(f64::from)
            .chain([-0.0, f64::INFINITY, f64::NAN, f64::MIN, f64::MAX])
            .collect::<Vec<_>>()
    );
}

/// Counts sink calls; every [`Write`] method funnels into `write_bytes`.
#[derive(Default)]
struct CountingWriter {
    bytes: Vec<u8>,
    calls: usize,
}

impl Write for CountingWriter {
    fn write_bytes(&mut self, bytes: &[u8]) {
        self.calls += 1;
        self.bytes.extend_from_slice(bytes);
    }
}

#[test]
fn byte_and_wire_lists_are_written_in_one_bulk_call() {
    // These element types are already their wire bytes, so the payload must
    // be a single `write_bytes` instead of one per element: the element tag,
    // the count and the payload make exactly three sink calls, at any length.
    macro_rules! check {
        ($tag:expr, $values:expr, $payload:expr) => {{
            let values = $values;
            let payload: Vec<u8> = $payload;
            let mut writer = CountingWriter::default();
            values.serialize_nbt(&mut writer).unwrap();

            let mut expected = vec![$tag.as_u8()];
            expected.extend_from_slice(&i32::try_from(values.len()).unwrap().to_be_bytes());
            expected.extend_from_slice(&payload);
            assert_eq!(writer.bytes, expected, "{}", stringify!($values));
            assert_eq!(writer.calls, 3, "{}", stringify!($values));
        }};
    }

    check!(Tag::Byte, vec![0u8, 1, 255], vec![0, 1, 255]);
    check!(
        Tag::Byte,
        (0u8..100).collect::<Vec<_>>(),
        (0u8..100).collect::<Vec<_>>()
    );
    check!(Tag::Byte, vec![-128i8, 0, 127], vec![0x80, 0x00, 0x7f]);
    check!(
        Tag::Short,
        vec![I16Be::new(-2), I16Be::new(258)],
        vec![0xff, 0xfe, 0x01, 0x02]
    );
    check!(
        Tag::Int,
        vec![I32Be::new(-2), I32Be::new(258)],
        vec![0xff, 0xff, 0xff, 0xfe, 0x00, 0x00, 0x01, 0x02]
    );
    check!(
        Tag::Long,
        vec![I64Be::new(-2), I64Be::new(258)],
        vec![
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02,
        ]
    );
    check!(
        Tag::Float,
        vec![F32Be::new(1.5)],
        vec![0x3f, 0xc0, 0x00, 0x00]
    );
    check!(
        Tag::Double,
        vec![F64Be::new(-2.25)],
        vec![0xc0, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    );
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

/// A slice writes the same `TAG_List` its `Vec<T>` does, including the empty
/// spelling and the bulk element path.
#[test]
fn slices_serialize_like_vecs() {
    let ints = vec![1i32, -2, 3];
    assert_eq!(to_bytes(&ints).unwrap(), to_bytes(&ints[..]).unwrap());

    let empty: &[i32] = &[];
    assert_eq!(
        to_bytes(empty).unwrap(),
        to_bytes(&Vec::<i32>::new()).unwrap()
    );

    let wires = vec![I32Be::new(7), I32Be::new(-7)];
    assert_eq!(to_bytes(&wires[..]).unwrap(), to_bytes(&wires).unwrap());
}

#[derive(SerializeNbt)]
struct VecList {
    values: Vec<i32>,
}

#[derive(SerializeNbt)]
struct BoxedList {
    values: Box<[i32]>,
}

#[derive(SerializeNbt)]
struct BorrowedList<'de> {
    values: &'de [i32],
}

#[derive(SerializeNbt)]
struct CowList<'de> {
    values: Cow<'de, [i32]>,
}

/// The list impl on `[T]` is what lets the slice containers serialize a field
/// as a plain list without `#[nbt(array)]`.
#[test]
fn slice_containers_serialize_like_vecs() {
    let values = vec![1, -2, 3];
    let expected = to_bytes(&VecList {
        values: values.clone(),
    })
    .unwrap();

    assert_eq!(
        to_bytes(&BoxedList {
            values: values.clone().into_boxed_slice(),
        })
        .unwrap(),
        expected
    );
    assert_eq!(
        to_bytes(&BorrowedList { values: &values }).unwrap(),
        expected
    );
    assert_eq!(
        to_bytes(&CowList {
            values: Cow::Borrowed(&values),
        })
        .unwrap(),
        expected
    );
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

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq, Default)]
struct SkipSerialize {
    #[nbt(skip_serialize)]
    out: i64,
    kept: String,
}

#[test]
fn skip_serialize_omits_the_field_and_keeps_the_read_arm() {
    let value = SkipSerialize {
        out: 99,
        kept: "abc".to_owned(),
    };
    let bytes = to_bytes(&value).unwrap();
    assert!(!bytes.windows(3).any(|window| window == b"out"));

    // `skip_serialize` alone leaves the field required on read, so this still
    // asks for a key the write above did not emit.
    assert_eq!(
        from_bytes::<SkipSerialize>(&bytes).unwrap_err(),
        Error::MissingField("out")
    );

    // When the bytes do carry the key, only `skip_deserialize` ignores it.
    let mut carried = bytes;
    let end = carried.len() - 1;
    carried.splice(
        end..end,
        [
            Tag::Long.as_u8(),
            0,
            3,
            b'o',
            b'u',
            b't',
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            7,
        ],
    );
    assert_eq!(
        from_bytes::<SkipSerialize>(&carried).unwrap(),
        SkipSerialize {
            out: 7,
            kept: "abc".to_owned(),
        }
    );
}

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq, Default)]
struct SkipDeserialize {
    #[nbt(skip_deserialize)]
    hidden: i64,
    kept: String,
}

#[test]
fn skip_deserialize_writes_the_field_but_ignores_it_on_read() {
    let value = SkipDeserialize {
        hidden: 99,
        kept: "abc".to_owned(),
    };
    let bytes = to_bytes(&value).unwrap();
    // The field is on the wire and is skipped as an unknown key when read.
    assert!(bytes.windows(6).any(|window| window == b"hidden"));
    assert_eq!(
        from_bytes::<SkipDeserialize>(&bytes).unwrap(),
        SkipDeserialize {
            hidden: 0,
            kept: "abc".to_owned(),
        }
    );
}

/// `skip` is the alias for both halves: it writes and reads exactly like a
/// struct that names both flags on the same key.
#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq, Default)]
struct SkipBoth {
    #[nbt(skip_serialize, skip_deserialize)]
    #[nbt(rename = "cache")]
    both: i64,
    #[nbt(rename = "n")]
    kept: String,
}

#[test]
fn skip_is_the_alias_for_both_flags() {
    let alias = Skipping {
        cache: 99,
        name: "abc".to_owned(),
    };
    let split = SkipBoth {
        both: 99,
        kept: "abc".to_owned(),
    };
    let bytes = to_bytes(&split).unwrap();
    assert_eq!(to_bytes(&alias).unwrap(), bytes);
    assert_eq!(
        from_bytes::<SkipBoth>(&bytes).unwrap(),
        SkipBoth {
            both: 0,
            kept: "abc".to_owned(),
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

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct Defaulted {
    required: i32,
    #[nbt(default)]
    optional: String,
    #[nbt(default)]
    count: i32,
}

#[derive(SerializeNbt)]
struct OnlyRequired {
    required: i32,
}

#[test]
fn default_fills_absent_keys_with_the_trait() {
    let bytes = to_bytes(&OnlyRequired { required: 7 }).unwrap();
    assert_eq!(
        from_bytes::<Defaulted>(&bytes).unwrap(),
        Defaulted {
            required: 7,
            optional: String::new(),
            count: 0,
        }
    );
}

#[test]
fn default_keeps_values_that_are_present() {
    let value = Defaulted {
        required: 1,
        optional: "kept".to_owned(),
        count: 3,
    };
    let decoded: Defaulted = from_bytes(&to_bytes(&value).unwrap()).unwrap();
    assert_eq!(decoded, value);
}

#[derive(DeserializeNbt, Debug, PartialEq)]
struct DefaultedByFunction {
    #[nbt(default = "loud")]
    text: String,
    #[nbt(default = "crate::forty_two")]
    number: i32,
}

fn loud() -> String {
    "loud".to_owned()
}

pub const fn forty_two() -> i32 {
    42
}

#[derive(SerializeNbt)]
struct Nothing;

#[derive(SerializeNbt)]
struct Present {
    text: String,
    number: i32,
}

#[test]
fn default_function_is_called_for_absent_keys() {
    let bytes = to_bytes(&Nothing).unwrap();
    assert_eq!(
        from_bytes::<DefaultedByFunction>(&bytes).unwrap(),
        DefaultedByFunction {
            text: "loud".to_owned(),
            number: 42,
        }
    );

    let bytes = to_bytes(&Present {
        text: "quiet".to_owned(),
        number: 1,
    })
    .unwrap();
    assert_eq!(
        from_bytes::<DefaultedByFunction>(&bytes).unwrap(),
        DefaultedByFunction {
            text: "quiet".to_owned(),
            number: 1,
        }
    );
}

/// `skip_serialize` takes the key off the wire but leaves it on the read
/// path, so `default` is exactly what fills it back; the two compose.
#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct DefaultedAndUnwritten {
    #[nbt(skip_serialize, default = "loud")]
    text: String,
    #[nbt(skip_serialize, default = "crate::forty_two")]
    number: i32,
}

#[test]
fn default_fills_keys_that_skip_serialize_leaves_out() {
    let value = DefaultedAndUnwritten {
        text: "ignored".to_owned(),
        number: 9,
    };
    let bytes = to_bytes(&value).unwrap();
    assert!(!bytes.windows(4).any(|window| window == b"text"));
    assert_eq!(
        from_bytes::<DefaultedAndUnwritten>(&bytes).unwrap(),
        DefaultedAndUnwritten {
            text: "loud".to_owned(),
            number: 42,
        }
    );
}
