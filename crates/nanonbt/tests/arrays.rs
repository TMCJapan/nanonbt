//! The `Write` and `Read` array methods, and the traits behind array fields.

use std::alloc::{GlobalAlloc, Layout, System};
use std::borrow::Cow;
use std::cell::Cell;

use nanonbt::{
    ByteArray, Cesu8, DeOpts, FromNBT, I32Be, I64Be, IntArray, LongArray, Read, Reader,
    TAG_INT_ARRAY, ToNBT, U32Be, U64Be, Write, Writer,
};

/// Counts the allocations each thread makes, so that a test can assert an
/// array field is written and read without copying it.
struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|count| count.set(count.get() + 1));
        // SAFETY: the caller upholds the `GlobalAlloc` contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the pointer came from `alloc` with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.with(|count| count.set(count.get() + 1));
        // SAFETY: the pointer came from `alloc` with this layout.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn write_array_methods_write_the_length_then_the_elements() {
    let mut out = Vec::new();
    Writer::new(&mut out)
        .write_byte_array([-1_i8, 0, 1])
        .unwrap();
    assert_eq!(out, [0, 0, 0, 3, 0xff, 0x00, 0x01]);

    let mut out = Vec::new();
    Writer::new(&mut out)
        .write_int_array([I32Be::new(1), I32Be::new(-1)])
        .unwrap();
    assert_eq!(out, [0, 0, 0, 2, 0, 0, 0, 1, 0xff, 0xff, 0xff, 0xff]);

    let mut out = Vec::new();
    Writer::new(&mut out)
        .write_long_array([I64Be::new(1), I64Be::new(i64::MIN)])
        .unwrap();
    assert_eq!(
        out,
        [
            0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 0x80, 0, 0, 0, 0, 0, 0, 0
        ]
    );
}

#[test]
fn write_array_methods_take_every_spelling() {
    let mut expected = Vec::new();
    Writer::new(&mut expected)
        .write_int_array([I32Be::new(-1), I32Be::new(2)])
        .unwrap();
    let mut signed = Vec::new();
    Writer::new(&mut signed)
        .write_int_array(vec![-1_i32, 2])
        .unwrap();
    let mut unsigned = Vec::new();
    Writer::new(&mut unsigned)
        .write_int_array(vec![u32::MAX, 2])
        .unwrap();
    let mut unsigned_be = Vec::new();
    Writer::new(&mut unsigned_be)
        .write_int_array([U32Be::new(u32::MAX), U32Be::new(2)])
        .unwrap();
    assert_eq!(signed, expected);
    assert_eq!(unsigned, expected);
    assert_eq!(unsigned_be, expected);

    let mut expected = Vec::new();
    Writer::new(&mut expected)
        .write_long_array([I64Be::new(-1)])
        .unwrap();
    let mut signed = Vec::new();
    Writer::new(&mut signed)
        .write_long_array(vec![-1_i64])
        .unwrap();
    let mut unsigned = Vec::new();
    Writer::new(&mut unsigned)
        .write_long_array(vec![u64::MAX])
        .unwrap();
    let mut unsigned_be = Vec::new();
    Writer::new(&mut unsigned_be)
        .write_long_array([U64Be::new(u64::MAX)])
        .unwrap();
    assert_eq!(signed, expected);
    assert_eq!(unsigned, expected);
    assert_eq!(unsigned_be, expected);

    let mut expected = Vec::new();
    Writer::new(&mut expected)
        .write_byte_array([-1_i8])
        .unwrap();
    let mut signed = Vec::new();
    Writer::new(&mut signed)
        .write_byte_array(vec![-1_i8])
        .unwrap();
    let mut unsigned = Vec::new();
    Writer::new(&mut unsigned)
        .write_byte_array(vec![u8::MAX])
        .unwrap();
    assert_eq!(signed, expected);
    assert_eq!(unsigned, expected);
}

#[test]
fn read_array_methods_borrow_from_the_input() {
    let bytes = [0x00, 0x00, 0x00, 0x01, 0xff, 0xff, 0xff, 0xfe];
    let mut reader = Reader::new(&bytes, DeOpts::default());
    assert_eq!(reader.read_len().unwrap(), 1);
    let ints = reader.read_int_array(1).unwrap();
    assert!(matches!(ints, Cow::Borrowed(_)));
    assert_eq!(ints[0].get(), -2);

    let bytes = [0x00, 0x00, 0x00, 0x02, 0xff, 0x01];
    let mut reader = Reader::new(&bytes, DeOpts::default());
    assert_eq!(reader.read_len().unwrap(), 2);
    let bytes = reader.read_byte_array(2).unwrap();
    assert!(matches!(bytes, Cow::Borrowed(_)));
    assert_eq!(&*bytes, &[-1, 1]);

    let bytes = [
        0x00, 0x00, 0x00, 0x01, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let mut reader = Reader::new(&bytes, DeOpts::default());
    assert_eq!(reader.read_len().unwrap(), 1);
    let longs = reader.read_long_array(1).unwrap();
    assert!(matches!(longs, Cow::Borrowed(_)));
    assert_eq!(longs[0].get(), i64::MIN);
}

#[test]
fn array_traits_write_and_read_either_spelling() {
    let mut out = Vec::new();
    let mut writer = Writer::new(&mut out);
    let name = Cesu8::new(b"data").unwrap();
    <[u64] as LongArray<u64>>::write_entry(&[1, u64::MAX], name, &mut writer).unwrap();

    let mut reader = Reader::new(&out, DeOpts::default());
    let tag = reader.read_tag().unwrap();
    reader.skip_name().unwrap();
    let read = <[I64Be] as LongArray<I64Be>>::read(tag, &mut reader).unwrap();
    assert_eq!(
        read.iter().map(|value| value.get()).collect::<Vec<_>>(),
        vec![1, u64::MAX.cast_signed()]
    );

    // A tag of another array kind is refused.
    let mut reader = Reader::new(&out, DeOpts::default());
    reader.read_tag().unwrap();
    reader.skip_name().unwrap();
    assert!(<[I64Be] as LongArray<I64Be>>::read(TAG_INT_ARRAY, &mut reader).is_err());
}

#[test]
fn array_traits_cover_every_element_spelling() {
    let name = Cesu8::new(b"x").unwrap();

    let mut signed = Vec::new();
    <[i32] as IntArray<i32>>::write_entry(&[-1, 2], name, &mut Writer::new(&mut signed)).unwrap();
    let mut unsigned = Vec::new();
    <[u32] as IntArray<u32>>::write_entry(&[u32::MAX, 2], name, &mut Writer::new(&mut unsigned))
        .unwrap();
    let mut signed_be = Vec::new();
    <[I32Be] as IntArray<I32Be>>::write_entry(
        &[I32Be::new(-1), I32Be::new(2)],
        name,
        &mut Writer::new(&mut signed_be),
    )
    .unwrap();
    let mut unsigned_be = Vec::new();
    <[U32Be] as IntArray<U32Be>>::write_entry(
        &[U32Be::new(u32::MAX), U32Be::new(2)],
        name,
        &mut Writer::new(&mut unsigned_be),
    )
    .unwrap();
    assert_eq!(signed, unsigned);
    assert_eq!(signed, signed_be);
    assert_eq!(signed, unsigned_be);

    let mut signed = Vec::new();
    <[i64] as LongArray<i64>>::write_entry(&[-1, 2], name, &mut Writer::new(&mut signed)).unwrap();
    let mut unsigned = Vec::new();
    <[u64] as LongArray<u64>>::write_entry(&[u64::MAX, 2], name, &mut Writer::new(&mut unsigned))
        .unwrap();
    let mut signed_be = Vec::new();
    <[I64Be] as LongArray<I64Be>>::write_entry(
        &[I64Be::new(-1), I64Be::new(2)],
        name,
        &mut Writer::new(&mut signed_be),
    )
    .unwrap();
    let mut unsigned_be = Vec::new();
    <[U64Be] as LongArray<U64Be>>::write_entry(
        &[U64Be::new(u64::MAX), U64Be::new(2)],
        name,
        &mut Writer::new(&mut unsigned_be),
    )
    .unwrap();
    assert_eq!(signed, unsigned);
    assert_eq!(signed, signed_be);
    assert_eq!(signed, unsigned_be);

    let mut signed = Vec::new();
    <[i8] as ByteArray<i8>>::write_entry(&[-1, 0], name, &mut Writer::new(&mut signed)).unwrap();
    let mut unsigned = Vec::new();
    <[u8] as ByteArray<u8>>::write_entry(&[u8::MAX, 0], name, &mut Writer::new(&mut unsigned))
        .unwrap();
    assert_eq!(signed, unsigned);
}

#[test]
fn array_fields_are_written_without_allocating() {
    #[derive(ToNBT)]
    struct Holder {
        #[nbt(array = "long")]
        data: Vec<U64Be>,
    }

    let holder = Holder {
        data: (0..10_000).map(U64Be::new).collect(),
    };
    let mut out = Vec::with_capacity(100_000);
    let mut writer = Writer::new(&mut out);

    let before = ALLOCATIONS.with(Cell::get);
    ToNBT::write(&holder, &mut writer).unwrap();
    assert_eq!(ALLOCATIONS.with(Cell::get), before);

    // The entry and the payload are all that was written: the tag, the name,
    // the length, 10 000 longs, and the compound's End tag.
    assert_eq!(out.len(), 1 + 2 + 4 + 4 + 10_000 * 8 + 1);
}

#[test]
fn array_fields_are_read_with_one_allocation() {
    #[derive(ToNBT)]
    struct Source {
        #[nbt(array = "long")]
        data: Vec<u64>,
    }

    #[derive(FromNBT)]
    struct Target {
        #[nbt(array = "long")]
        data: Vec<u64>,
    }

    let bytes = nanonbt::to_bytes(&Source {
        data: vec![1; 10_000],
    })
    .unwrap();

    let before = ALLOCATIONS.with(Cell::get);
    let target = nanonbt::from_bytes::<Target>(&bytes).unwrap();
    assert_eq!(ALLOCATIONS.with(Cell::get) - before, 1);
    assert_eq!(target.data.len(), 10_000);
}

/// The big-endian spelling reads into a `Vec` with one allocation too.
#[test]
fn big_endian_array_fields_are_read_with_one_allocation() {
    #[derive(ToNBT)]
    struct Source {
        #[nbt(array = "int")]
        data: Vec<u32>,
    }

    #[derive(FromNBT)]
    struct Target {
        #[nbt(array = "int")]
        data: Vec<I32Be>,
    }

    let bytes = nanonbt::to_bytes(&Source {
        data: vec![1; 10_000],
    })
    .unwrap();

    let before = ALLOCATIONS.with(Cell::get);
    let target = nanonbt::from_bytes::<Target>(&bytes).unwrap();
    assert_eq!(ALLOCATIONS.with(Cell::get) - before, 1);
    assert_eq!(target.data.len(), 10_000);
}

/// A numeric list's payload is read in one go under the `simd` feature, so
/// the `Vec` is built once instead of being grown element by element.
#[test]
#[cfg(feature = "simd")]
fn numeric_lists_are_read_with_one_allocation() {
    #[derive(ToNBT)]
    struct Source {
        data: Vec<i16>,
    }

    #[derive(FromNBT)]
    struct Target {
        data: Vec<i16>,
    }

    let bytes = nanonbt::to_bytes(&Source {
        data: (0..10_000).map(|i| i16::try_from(i).unwrap()).collect(),
    })
    .unwrap();

    let before = ALLOCATIONS.with(Cell::get);
    let target = nanonbt::from_bytes::<Target>(&bytes).unwrap();
    assert_eq!(ALLOCATIONS.with(Cell::get) - before, 1);
    assert_eq!(target.data.len(), 10_000);
}

/// A numeric list's payload is swapped in a stack buffer, so writing one
/// takes no allocation either.
#[test]
#[cfg(feature = "simd")]
fn numeric_list_fields_are_written_without_allocating() {
    #[derive(ToNBT)]
    struct Holder {
        data: Vec<u16>,
    }

    let holder = Holder {
        data: vec![1; 10_000],
    };
    let mut out = Vec::with_capacity(100_000);
    let mut writer = Writer::new(&mut out);

    let before = ALLOCATIONS.with(Cell::get);
    ToNBT::write(&holder, &mut writer).unwrap();
    assert_eq!(ALLOCATIONS.with(Cell::get), before);

    // The tag, the name, the element tag, the length, the elements, and the
    // compound's End tag.
    assert_eq!(out.len(), 1 + 2 + 4 + 1 + 4 + 10_000 * 2 + 1);
}
