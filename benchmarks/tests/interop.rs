//! Cross-library interop and correctness checks.
//!
//! The benchmarks are only meaningful if all three libraries encode the exact
//! same document and decode it back into the same values. These tests assert
//! both:
//!
//! 1. `nanonbt`, `fastnbt` and `simdnbt` produce byte-identical NBT for every
//!    fixture, and
//! 2. decoding each library's output with any of the three libraries yields
//!    the fixture values again.

use nanonbt_bench::{fastnbt_models, fixtures, nanonbt_models, simdnbt_models};

fn assert_bytes_match(scenario: &str, nanonbt: &[u8], fastnbt: &[u8], simdnbt: &[u8]) {
    assert_eq!(
        nanonbt, fastnbt,
        "{scenario}: nanonbt and fastnbt produced different NBT"
    );
    assert_eq!(
        nanonbt, simdnbt,
        "{scenario}: nanonbt and simdnbt produced different NBT"
    );
}

#[test]
fn small_roundtrips() {
    let fixture = fixtures::small();
    let nanonbt = nanonbt_models::small_from_fixture(&fixture);
    let fastnbt = fastnbt_models::small_from_fixture(&fixture);
    let simdnbt = simdnbt_models::small_from_fixture(&fixture);

    let bytes = nanonbt_models::small_write(&nanonbt);
    assert_bytes_match(
        "small",
        &bytes,
        &fastnbt_models::small_write(&fastnbt),
        &simdnbt_models::small_write(&simdnbt),
    );

    let decoded_nanonbt: nanonbt_models::Small = nanonbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_nanonbt, nanonbt);

    let decoded_fastnbt: fastnbt_models::Small = fastnbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_fastnbt, fastnbt);

    let decoded_simdnbt: simdnbt_models::Small = simdnbt_models::typed_read(&bytes);
    assert_eq!(decoded_simdnbt, simdnbt);
}

#[test]
fn byte_array_roundtrips() {
    let fixture = fixtures::byte_array();
    let nanonbt = nanonbt_models::byte_array_from_fixture(&fixture);
    let fastnbt = fastnbt_models::byte_array_from_fixture(&fixture);

    let bytes = nanonbt_models::byte_array_owned(&nanonbt);
    assert_bytes_match(
        "byte_array",
        &bytes,
        &fastnbt_models::byte_array_owned(&fastnbt),
        &simdnbt_models::byte_array_write(&fixture),
    );

    let decoded_nanonbt: nanonbt_models::ByteArrayOwned = nanonbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_nanonbt.data, fixture);

    let decoded_nanonbt_borrowed: nanonbt_models::ByteArrayBorrowed<'_> =
        nanonbt::from_slice(&bytes).unwrap();
    assert_eq!(decoded_nanonbt_borrowed.data, fixture);

    let decoded_fastnbt: fastnbt_models::ByteArrayOwned = fastnbt::from_bytes(&bytes).unwrap();
    let fastnbt_bytes: Vec<u8> = decoded_fastnbt
        .data
        .iter()
        .copied()
        .map(i8::cast_unsigned)
        .collect();
    assert_eq!(fastnbt_bytes, fixture);

    let decoded_fastnbt_borrowed: fastnbt_models::ByteArrayBorrowed<'_> =
        fastnbt::from_bytes(&bytes).unwrap();
    let fastnbt_borrowed_bytes: Vec<u8> = decoded_fastnbt_borrowed
        .data
        .iter()
        .map(i8::cast_unsigned)
        .collect();
    assert_eq!(fastnbt_borrowed_bytes, fixture);
}

#[test]
fn int_array_roundtrips() {
    let fixture = fixtures::int_array();
    let nanonbt = nanonbt_models::int_array_from_fixture(&fixture);
    let fastnbt = fastnbt_models::int_array_from_fixture(&fixture);

    let bytes = nanonbt_models::int_array_owned(&nanonbt);
    assert_bytes_match(
        "int_array",
        &bytes,
        &fastnbt_models::int_array_owned(&fastnbt),
        &simdnbt_models::int_array_write(&fixture),
    );

    let decoded_nanonbt: nanonbt_models::IntArrayOwned = nanonbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_nanonbt.data, fixture);

    let decoded_nanonbt_borrowed: nanonbt_models::IntArrayBorrowed<'_> =
        nanonbt::from_slice(&bytes).unwrap();
    let values: Vec<i32> = decoded_nanonbt_borrowed
        .data
        .iter()
        .map(|item| item.get())
        .collect();
    assert_eq!(values, fixture);

    let decoded_fastnbt: fastnbt_models::IntArrayOwned = fastnbt::from_bytes(&bytes).unwrap();
    assert_eq!(&*decoded_fastnbt.data, fixture.as_slice());

    let decoded_fastnbt_borrowed: fastnbt_models::IntArrayBorrowed<'_> =
        fastnbt::from_bytes(&bytes).unwrap();
    let values: Vec<i32> = decoded_fastnbt_borrowed.data.iter().collect();
    assert_eq!(values, fixture);

    let parsed = simdnbt::borrow::read(&mut std::io::Cursor::new(&bytes[..]))
        .unwrap()
        .unwrap();
    assert_eq!(parsed.int_array("data").unwrap(), fixture);
}

#[test]
fn long_array_roundtrips() {
    let fixture = fixtures::long_array();
    let nanonbt = nanonbt_models::long_array_from_fixture(&fixture);
    let fastnbt = fastnbt_models::long_array_from_fixture(&fixture);

    let bytes = nanonbt_models::long_array_owned(&nanonbt);
    assert_bytes_match(
        "long_array",
        &bytes,
        &fastnbt_models::long_array_owned(&fastnbt),
        &simdnbt_models::long_array_write(&fixture),
    );

    let decoded_nanonbt: nanonbt_models::LongArrayOwned = nanonbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_nanonbt.data, fixture);

    let decoded_nanonbt_borrowed: nanonbt_models::LongArrayBorrowed<'_> =
        nanonbt::from_slice(&bytes).unwrap();
    let values: Vec<i64> = decoded_nanonbt_borrowed
        .data
        .iter()
        .map(|item| item.get())
        .collect();
    assert_eq!(values, fixture);

    let decoded_fastnbt: fastnbt_models::LongArrayOwned = fastnbt::from_bytes(&bytes).unwrap();
    assert_eq!(&*decoded_fastnbt.data, fixture.as_slice());

    let decoded_fastnbt_borrowed: fastnbt_models::LongArrayBorrowed<'_> =
        fastnbt::from_bytes(&bytes).unwrap();
    let values: Vec<i64> = decoded_fastnbt_borrowed.data.iter().collect();
    assert_eq!(values, fixture);

    let parsed = simdnbt::borrow::read(&mut std::io::Cursor::new(&bytes[..]))
        .unwrap()
        .unwrap();
    assert_eq!(parsed.long_array("data").unwrap(), fixture);
}

#[test]
fn strings_roundtrips() {
    let fixture = fixtures::strings();
    let nanonbt = nanonbt_models::strings_from_fixture(&fixture);
    let fastnbt = fastnbt_models::strings_from_fixture(&fixture);
    let simdnbt = simdnbt_models::strings_from_fixture(&fixture);

    let bytes = nanonbt_models::strings_write(&nanonbt);
    assert_bytes_match(
        "strings",
        &bytes,
        &fastnbt_models::strings_write(&fastnbt),
        &simdnbt_models::strings_write(&simdnbt),
    );

    let decoded_nanonbt: nanonbt_models::Strings = nanonbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_nanonbt, nanonbt);

    let decoded_fastnbt: fastnbt_models::Strings = fastnbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_fastnbt, fastnbt);

    let decoded_fastnbt_borrowed: fastnbt_models::StringsBorrowed<'_> =
        fastnbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_fastnbt_borrowed.big, fixture.big.as_str());
    assert_eq!(decoded_fastnbt_borrowed.words.len(), fixture.words.len());

    let decoded_simdnbt: simdnbt_models::Strings = simdnbt_models::typed_read(&bytes);
    assert_eq!(decoded_simdnbt, simdnbt);
}

#[test]
fn compounds_roundtrip() {
    let fixture = fixtures::compounds();
    let nanonbt = nanonbt_models::compounds_from_fixture(&fixture);
    let fastnbt = fastnbt_models::compounds_from_fixture(&fixture);
    let simdnbt = simdnbt_models::compounds_from_fixture(&fixture);

    let bytes = nanonbt_models::compounds_write(&nanonbt);
    assert_bytes_match(
        "compounds",
        &bytes,
        &fastnbt_models::compounds_write(&fastnbt),
        &simdnbt_models::compounds_write(&simdnbt),
    );

    let decoded_nanonbt: nanonbt_models::CompoundList = nanonbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_nanonbt, nanonbt);

    let decoded_fastnbt: fastnbt_models::CompoundList = fastnbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_fastnbt, fastnbt);

    let decoded_fastnbt_borrowed: fastnbt_models::CompoundListBorrowed<'_> =
        fastnbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_fastnbt_borrowed.items.len(), fixture.len());
    assert_eq!(
        decoded_fastnbt_borrowed.items[0].name,
        fixture[0].name.as_str()
    );

    let decoded_simdnbt: simdnbt_models::CompoundList = simdnbt_models::typed_read(&bytes);
    assert_eq!(decoded_simdnbt, simdnbt);
}

#[test]
fn many_fields_roundtrip() {
    let fixture = fixtures::many_fields();
    let nanonbt = nanonbt_models::many_fields_from_fixture(&fixture);
    let fastnbt = fastnbt_models::many_fields_from_fixture(&fixture);
    let simdnbt = simdnbt_models::many_fields_from_fixture(&fixture);

    let bytes = nanonbt_models::many_fields_write(&nanonbt);
    assert_bytes_match(
        "many_fields",
        &bytes,
        &fastnbt_models::many_fields_write(&fastnbt),
        &simdnbt_models::many_fields_write(&simdnbt),
    );

    let decoded_nanonbt: nanonbt_models::ManyFields = nanonbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_nanonbt, nanonbt);

    let decoded_fastnbt: fastnbt_models::ManyFields = fastnbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_fastnbt, fastnbt);

    let decoded_simdnbt: simdnbt_models::ManyFields = simdnbt_models::typed_read(&bytes);
    assert_eq!(decoded_simdnbt, simdnbt);
}

#[test]
fn deep_roundtrip() {
    let fixture = fixtures::deep();
    let nanonbt = nanonbt_models::deep_from_fixture(&fixture);
    let fastnbt = fastnbt_models::deep_from_fixture(&fixture);
    let simdnbt = simdnbt_models::deep_from_fixture(&fixture);

    let bytes = nanonbt_models::deep_write(&nanonbt);
    assert_bytes_match(
        "deep",
        &bytes,
        &fastnbt_models::deep_write(&fastnbt),
        &simdnbt_models::deep_write(&simdnbt),
    );

    let decoded_nanonbt: nanonbt_models::Deep1 = nanonbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_nanonbt, nanonbt);

    let decoded_fastnbt: fastnbt_models::Deep1 = fastnbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_fastnbt, fastnbt);

    let decoded_simdnbt: simdnbt_models::Deep1 = simdnbt_models::typed_read(&bytes);
    assert_eq!(decoded_simdnbt, simdnbt);
}

#[test]
fn chunk_roundtrips() {
    let fixture = fixtures::chunk();
    let nanonbt = nanonbt_models::chunk_from_fixture(&fixture);
    let fastnbt = fastnbt_models::chunk_from_fixture(&fixture);

    let bytes = nanonbt_models::chunk_write(&nanonbt);
    assert_bytes_match(
        "chunk",
        &bytes,
        &fastnbt_models::chunk_write(&fastnbt),
        &simdnbt_models::chunk_write(&fixture),
    );

    let decoded_nanonbt: nanonbt_models::Chunk = nanonbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_nanonbt.block_states, fixture.block_states);
    assert_eq!(decoded_nanonbt.biomes, fixture.biomes);
    assert_eq!(decoded_nanonbt.block_light, fixture.block_light);
    assert_eq!(decoded_nanonbt.sky_light, fixture.sky_light);
    assert_eq!(decoded_nanonbt.palette.len(), fixture.palette.len());
    assert_eq!(decoded_nanonbt.palette[0].name, fixture.palette[0].name);

    let decoded_nanonbt_borrowed: nanonbt_models::ChunkBorrowed<'_> =
        nanonbt::from_slice(&bytes).unwrap();
    assert_eq!(decoded_nanonbt_borrowed.y, fixture.y);
    assert_eq!(decoded_nanonbt_borrowed.biomes, fixture.biomes);
    assert_eq!(decoded_nanonbt_borrowed.block_light, fixture.block_light);
    assert_eq!(decoded_nanonbt_borrowed.sky_light, fixture.sky_light);
    let states: Vec<i64> = decoded_nanonbt_borrowed
        .block_states
        .iter()
        .map(|state| state.get())
        .collect();
    assert_eq!(states, fixture.block_states);

    let decoded_fastnbt: fastnbt_models::Chunk = fastnbt::from_bytes(&bytes).unwrap();
    assert_eq!(
        &*decoded_fastnbt.block_states,
        fixture.block_states.as_slice()
    );
    assert_eq!(
        decoded_fastnbt
            .biomes
            .iter()
            .map(|&b| b.cast_unsigned())
            .collect::<Vec<_>>(),
        fixture.biomes
    );
    assert_eq!(decoded_fastnbt.palette.len(), fixture.palette.len());

    let decoded_fastnbt_borrowed: fastnbt_models::ChunkBorrowed<'_> =
        fastnbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded_fastnbt_borrowed.y, fixture.y);
    assert_eq!(
        decoded_fastnbt_borrowed
            .biomes
            .iter()
            .map(i8::cast_unsigned)
            .collect::<Vec<_>>(),
        fixture.biomes
    );
    let states: Vec<i64> = decoded_fastnbt_borrowed.block_states.iter().collect();
    assert_eq!(states, fixture.block_states);

    let parsed = simdnbt::borrow::read(&mut std::io::Cursor::new(&bytes[..]))
        .unwrap()
        .unwrap();
    assert_eq!(parsed.byte("y"), Some(fixture.y));
    assert_eq!(parsed.byte_array("biomes"), Some(fixture.biomes.as_slice()));
    assert_eq!(
        parsed.long_array("block_states"),
        Some(fixture.block_states.clone())
    );
    let palette = parsed
        .list("palette")
        .and_then(|list| list.compounds())
        .unwrap();
    assert_eq!(palette.len(), fixture.palette.len());
    assert_eq!(
        palette.first().unwrap().string("name").unwrap().to_str(),
        fixture.palette[0].name.as_str()
    );
}

#[test]
fn fastnbt_value_matches_model() {
    let fixture = fixtures::small();
    let model = fastnbt_models::small_from_fixture(&fixture);
    let bytes = fastnbt_models::small_write(&model);

    let expected = fastnbt::to_value(&model).unwrap();
    let decoded: fastnbt::Value = fastnbt::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, expected);
}
