extern crate alloc;
use std::borrow::Cow;

use nanonbt::{Error, I16Be, I32Be, I64Be, Tag, from_slice, to_bytes};

#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Debug, PartialEq)]
struct Raw<'de> {
    id: i32,
    #[nbt(array = byte)]
    pixels: &'de [u8],
    #[nbt(array = int)]
    ints: &'de [I32Be],
    #[nbt(array = long)]
    longs: Cow<'de, [I64Be]>,
}

#[derive(nanonbt::DeserializeNbt, Debug, PartialEq)]
struct RawOwned {
    id: i32,
    #[nbt(array = byte)]
    pixels: Vec<u8>,
    #[nbt(array = int)]
    ints: Vec<I32Be>,
    #[nbt(array = long)]
    longs: Vec<I64Be>,
}

#[test]
fn zero_copy_borrows_point_into_the_input() {
    let value = Raw {
        id: 7,
        pixels: &[1, 2, 3],
        ints: &[I32Be::new(-1), I32Be::new(5)],
        longs: Cow::Owned(vec![I64Be::new(9), I64Be::new(-9)]),
    };
    let bytes = to_bytes(&value).unwrap();
    let decoded: Raw<'_> = from_slice(&bytes).unwrap();

    assert_eq!(decoded.id, 7);
    assert_eq!(decoded.pixels, &[1, 2, 3]);
    assert_eq!(decoded.ints[0].get(), -1);
    assert_eq!(decoded.ints[1].get(), 5);
    let longs: Vec<i64> = decoded.longs.iter().map(|long| long.get()).collect();
    assert_eq!(longs, [9, -9]);
    assert!(
        matches!(decoded.longs, Cow::Borrowed(_)),
        "Cow fields decode as borrowed"
    );

    let start = bytes.as_ptr() as usize;
    let range = start..start + bytes.len();
    assert!(
        range.contains(&(decoded.pixels.as_ptr() as usize)),
        "pixels should borrow from the input buffer"
    );
    assert!(
        range.contains(&(decoded.ints.as_ptr() as usize)),
        "ints should borrow from the input buffer"
    );
    assert!(
        range.contains(&(decoded.longs.as_ptr() as usize)),
        "longs should borrow from the input buffer"
    );

    // The borrowed view matches an owned decode of the same bytes.
    let owned: RawOwned = nanonbt::from_bytes(&bytes).unwrap();
    assert_eq!(owned.id, decoded.id);
    assert_eq!(owned.pixels, decoded.pixels);
    assert_eq!(owned.ints, decoded.ints);
    assert_eq!(owned.longs, decoded.longs.as_ref());
}

#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Debug, PartialEq)]
struct Inner<'de> {
    #[nbt(array = byte)]
    data: &'de [u8],
}

#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Debug, PartialEq)]
struct Outer<'de> {
    version: i32,
    inner: Inner<'de>,
    #[nbt(array = int)]
    trailing: &'de [I32Be],
}

#[test]
fn nested_borrowing_structs_work() {
    let value = Outer {
        version: 2,
        inner: Inner { data: &[9, 8, 7] },
        trailing: &[I32Be::new(1), I32Be::new(2)],
    };
    let bytes = to_bytes(&value).unwrap();
    let decoded: Outer<'_> = from_slice(&bytes).unwrap();
    assert_eq!(decoded, value);

    let range = bytes.as_ptr() as usize..bytes.as_ptr() as usize + bytes.len();
    assert!(range.contains(&(decoded.inner.data.as_ptr() as usize)));
    assert!(range.contains(&(decoded.trailing.as_ptr() as usize)));
}

#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Debug, PartialEq)]
struct BorrowedNewtype<'de>(#[nbt(array = long)] &'de [I64Be]);

#[test]
fn borrowed_newtypes_work() {
    let value = BorrowedNewtype(&[I64Be::new(1), I64Be::new(2)]);
    let bytes = to_bytes(&value).unwrap();
    let decoded: BorrowedNewtype<'_> = from_slice(&bytes).unwrap();
    assert_eq!(decoded.0[1].get(), 2);
    let range = bytes.as_ptr() as usize..bytes.as_ptr() as usize + bytes.len();
    assert!(range.contains(&(decoded.0.as_ptr() as usize)));
}

#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Debug, PartialEq)]
struct Mixed<'de> {
    title: String,
    #[nbt(array)]
    inferred: &'de [i8],
    #[nbt(array = byte)]
    pixels: Cow<'de, [u8]>,
}

#[test]
fn owned_fields_inside_borrowing_structs() {
    let value = Mixed {
        title: "mixed".to_owned(),
        inferred: &[-1, 0, 1],
        pixels: Cow::Borrowed(&[4, 5]),
    };
    let bytes = to_bytes(&value).unwrap();
    let decoded: Mixed<'_> = from_slice(&bytes).unwrap();
    assert_eq!(decoded.title, "mixed");
    assert_eq!(decoded.inferred, &[-1, 0, 1]);
    assert_eq!(decoded.pixels.as_ref(), &[4, 5]);
    assert!(matches!(decoded.pixels, Cow::Borrowed(_)));
}

const DEFAULTS: &[I32Be] = &[I32Be::new(3), I32Be::new(4)];

const fn default_slice<'a>() -> &'a [I32Be] {
    DEFAULTS
}

fn default_i32s() -> Vec<i32> {
    vec![3, 4]
}

const fn default_cow<'a>() -> Cow<'a, [I32Be]> {
    Cow::Borrowed(DEFAULTS)
}

const fn default_text<'a>() -> &'a str {
    "fallback"
}

#[derive(nanonbt::DeserializeNbt, Debug, PartialEq)]
struct DefaultedBorrowed<'de> {
    // Owned fields work through `Default` and the function form alike.
    #[nbt(default)]
    owned: Vec<i32>,
    #[nbt(default = "default_i32s")]
    owned_by_function: Vec<i32>,
    // Borrowed fields use the same fallback, built without borrowing the input.
    #[nbt(array = byte, default)]
    trait_bytes: Vec<u8>,
    #[nbt(array = int, default = "default_slice")]
    borrowed_slice: &'de [I32Be],
    #[nbt(array = int, default = "default_cow")]
    borrowed_cow: Cow<'de, [I32Be]>,
    #[nbt(default = "default_text")]
    text: &'de str,
}

/// Only the keys above are absent from this document.
#[derive(nanonbt::SerializeNbt)]
struct WithoutDefaults {}

#[test]
fn defaults_fill_absent_keys_in_borrowing_structs() {
    let bytes = to_bytes(&WithoutDefaults {}).unwrap();
    let decoded: DefaultedBorrowed<'_> = from_slice(&bytes).unwrap();
    assert_eq!(decoded.owned, Vec::<i32>::new());
    assert_eq!(decoded.owned_by_function, [3, 4]);
    assert_eq!(decoded.trait_bytes, Vec::<u8>::new());
    assert_eq!(decoded.borrowed_slice, DEFAULTS);
    assert_eq!(decoded.borrowed_cow, DEFAULTS);
    assert_eq!(decoded.text, "fallback");
}

#[derive(nanonbt::SerializeNbt)]
struct WithDefaults {
    owned: Vec<i32>,
    owned_by_function: Vec<i32>,
    #[nbt(array = byte)]
    trait_bytes: Vec<u8>,
    #[nbt(array = int)]
    borrowed_slice: Vec<I32Be>,
    #[nbt(array = int)]
    borrowed_cow: Vec<I32Be>,
    text: &'static str,
}

#[test]
fn defaults_keep_borrowed_values_that_are_present() {
    let value = WithDefaults {
        owned: vec![1, 2],
        owned_by_function: vec![5],
        trait_bytes: vec![1, 2],
        borrowed_slice: vec![I32Be::new(6)],
        borrowed_cow: vec![I32Be::new(7)],
        text: "present",
    };
    let bytes = to_bytes(&value).unwrap();
    let decoded: DefaultedBorrowed<'_> = from_slice(&bytes).unwrap();
    assert_eq!(decoded.owned, [1, 2]);
    assert_eq!(decoded.owned_by_function, [5]);
    assert_eq!(decoded.trait_bytes, [1, 2]);
    assert_eq!(decoded.borrowed_slice, [I32Be::new(6)]);
    assert_eq!(decoded.borrowed_cow.as_ref(), [I32Be::new(7)]);
    assert_eq!(decoded.text, "present");

    let range = bytes.as_ptr() as usize..bytes.as_ptr() as usize + bytes.len();
    assert!(range.contains(&(decoded.borrowed_slice.as_ptr() as usize)));
    assert!(range.contains(&(decoded.borrowed_cow.as_ptr() as usize)));
    assert!(range.contains(&(decoded.text.as_ptr() as usize)));
}

/// Nested borrowing structs take a default as well.
#[derive(nanonbt::DeserializeNbt, Debug, PartialEq)]
struct DefaultedOuter<'de> {
    #[nbt(default = "default_inner")]
    inner: Inner<'de>,
    #[nbt(array = int, default)]
    trailing: &'de [I32Be],
}

const EMPTY_INNER: Inner<'static> = Inner { data: &[] };

const fn default_inner() -> Inner<'static> {
    EMPTY_INNER
}

#[test]
fn nested_borrowing_fields_accept_defaults() {
    let bytes = to_bytes(&WithoutDefaults {}).unwrap();
    let decoded: DefaultedOuter<'_> = from_slice(&bytes).unwrap();
    assert_eq!(decoded.inner, EMPTY_INNER);
    assert_eq!(decoded.trailing, []);
}

/// Writes every field as an int, the wrong tag for each borrowed field of
/// [`BorrowedDefaults`].
#[derive(nanonbt::SerializeNbt)]
struct AllInts {
    ints: i32,
    text: i32,
    after: i32,
}

#[derive(nanonbt::DeserializeNbt, Debug, PartialEq)]
struct BorrowedDefaults<'de> {
    #[nbt(array = int, default)]
    ints: &'de [I32Be],
    #[nbt(default)]
    text: &'de str,
    after: i32,
    #[nbt(self)]
    raw: &'de [u8],
}

#[test]
fn borrowed_defaults_skip_keys_written_with_the_wrong_tag() {
    let bytes = to_bytes(&AllInts {
        ints: 1,
        text: 2,
        after: 42,
    })
    .unwrap();
    let decoded: BorrowedDefaults<'_> = from_slice(&bytes).unwrap();
    assert_eq!(
        decoded,
        BorrowedDefaults {
            ints: &[],
            text: "",
            after: 42,
            raw: &bytes[3..],
        }
    );
}

#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Debug, PartialEq)]
struct ShortLists<'de> {
    lists: Vec<&'de [I16Be]>,
}

#[derive(nanonbt::DeserializeNbt, Debug, PartialEq)]
struct ShortListsOwned {
    lists: Vec<Vec<i16>>,
}

#[test]
fn nested_short_lists_borrow_from_the_input() {
    let first = [I16Be::new(1), I16Be::new(-2)];
    let last = [I16Be::new(i16::MAX)];
    let value = ShortLists {
        lists: vec![&first, &[], &last],
    };
    let bytes = to_bytes(&value).unwrap();
    let decoded: ShortLists<'_> = from_slice(&bytes).unwrap();
    assert_eq!(decoded, value);

    let range = bytes.as_ptr() as usize..bytes.as_ptr() as usize + bytes.len();
    assert!(range.contains(&(decoded.lists[0].as_ptr() as usize)));
    assert!(range.contains(&(decoded.lists[2].as_ptr() as usize)));

    let owned: ShortListsOwned = nanonbt::from_bytes(&bytes).unwrap();
    assert_eq!(owned.lists, [vec![1, -2], vec![], vec![i16::MAX]]);
}

/// A root `TAG_List` with an empty name, followed by `payload`.
fn root_list(payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![Tag::List.as_u8(), 0, 0];
    bytes.extend_from_slice(payload);
    bytes
}

#[test]
fn empty_short_lists_pass_whatever_their_element_tag() {
    for element in [Tag::End, Tag::Short, Tag::Byte] {
        let bytes = root_list(&[element.as_u8(), 0, 0, 0, 0]);
        assert_eq!(from_slice::<&[I16Be]>(&bytes), Ok(&[][..]));
    }
}

#[test]
fn non_empty_short_lists_with_end_elements_are_rejected() {
    let bytes = root_list(&[Tag::End.as_u8(), 0, 0, 0, 1]);
    assert!(matches!(
        from_slice::<&[I16Be]>(&bytes),
        Err(Error::InvalidData(_))
    ));
}

#[test]
fn short_list_element_tag_mismatches_are_reported() {
    let bytes = root_list(&[Tag::Int.as_u8(), 0, 0, 0, 1, 0, 0, 0, 1]);
    assert_eq!(
        from_slice::<&[I16Be]>(&bytes),
        Err(Error::InvalidTag {
            expected: Tag::Short,
            actual: Tag::Int.as_u8(),
        })
    );
}

#[test]
fn truncated_short_lists_are_rejected() {
    // Two shorts announced, one present.
    let bytes = root_list(&[Tag::Short.as_u8(), 0, 0, 0, 2, 0, 1]);
    assert_eq!(from_slice::<&[I16Be]>(&bytes), Err(Error::UnexpectedEof));
}

#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Debug, PartialEq)]
struct BareShortList<'de> {
    shorts: &'de [I16Be],
    #[nbt(default)]
    absent: &'de [I16Be],
}

/// Only `shorts` is written, so `absent` exercises the default.
#[derive(nanonbt::SerializeNbt)]
struct ShortsOnly<'a> {
    shorts: &'a [I16Be],
}

#[test]
fn bare_short_list_fields_borrow_without_an_array_attribute() {
    let shorts = [I16Be::new(3), I16Be::new(-4)];
    let bytes = to_bytes(&ShortsOnly { shorts: &shorts }).unwrap();
    let decoded: BareShortList<'_> = from_slice(&bytes).unwrap();
    assert_eq!(decoded.shorts, shorts);
    assert_eq!(decoded.absent, []);

    let range = bytes.as_ptr() as usize..bytes.as_ptr() as usize + bytes.len();
    assert!(range.contains(&(decoded.shorts.as_ptr() as usize)));

    let reencoded = to_bytes(&decoded).unwrap();
    assert_eq!(
        from_slice::<BareShortList<'_>>(&reencoded).unwrap(),
        decoded
    );
}

#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Debug, PartialEq)]
struct CowShortList<'de> {
    shorts: Cow<'de, [I16Be]>,
}

#[test]
fn cow_short_list_fields_decode_as_borrowed() {
    let value = CowShortList {
        shorts: Cow::Owned(vec![I16Be::new(5), I16Be::new(6)]),
    };
    let bytes = to_bytes(&value).unwrap();
    let decoded: CowShortList<'_> = from_slice(&bytes).unwrap();
    assert_eq!(decoded, value);
    assert!(matches!(decoded.shorts, Cow::Borrowed(_)));
}
