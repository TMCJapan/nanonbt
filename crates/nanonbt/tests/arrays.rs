//! The array types, their conversions, and the writes that must not copy.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use nanonbt::{
    ArrayOf, ByteArray, DeOpts, FromNBT, IntArray, LongArray, Read, Reader, TAG_INT_ARRAY, ToNBT,
    Writer,
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
fn arrays_convert_from_slices_of_either_spelling() {
    assert_eq!(
        ByteArray::from(&[0u8, 255][..]),
        ByteArray::new(vec![0, -1])
    );
    assert_eq!(ByteArray::from(&[-1i8, 0][..]), ByteArray::new(vec![-1, 0]));
    assert_eq!(IntArray::from(&[u32::MAX][..]), IntArray::new(vec![-1]));
    assert_eq!(
        IntArray::from(&[i32::MIN][..]),
        IntArray::new(vec![i32::MIN])
    );
    assert_eq!(LongArray::from(&[u64::MAX][..]), LongArray::new(vec![-1]));
    assert_eq!(
        LongArray::from(&[i64::MIN][..]),
        LongArray::new(vec![i64::MIN])
    );
}

#[test]
fn arrays_convert_to_vectors_of_either_spelling() {
    assert_eq!(Vec::<u8>::from(ByteArray::new(vec![-1, 0])), vec![255, 0]);
    assert_eq!(Vec::<i8>::from(ByteArray::new(vec![-1, 0])), vec![-1, 0]);
    assert_eq!(Vec::<u32>::from(IntArray::new(vec![-1])), vec![u32::MAX]);
    assert_eq!(Vec::<i32>::from(IntArray::new(vec![-1])), vec![-1]);
    assert_eq!(Vec::<u64>::from(LongArray::new(vec![-1])), vec![u64::MAX]);
    assert_eq!(Vec::<i64>::from(LongArray::new(vec![-1])), vec![-1]);
}

#[test]
fn fixed_arrays_check_their_length() {
    assert_eq!(
        <[u32; 2]>::try_from(IntArray::new(vec![1, 2])).unwrap(),
        [1, 2]
    );
    assert_eq!(
        <[i64; 2]>::try_from(LongArray::new(vec![1, 2])).unwrap(),
        [1, 2]
    );
    assert!(<[u8; 3]>::try_from(ByteArray::new(vec![1, 2])).is_err());
    assert_eq!(
        <[i8; 3]>::try_from(ByteArray::new(vec![1, 2]))
            .unwrap_err()
            .to_string(),
        "sequence has a different length than the target array"
    );
}

#[test]
fn array_of_writes_and_reads_either_spelling() {
    let mut out = Vec::new();
    let mut writer = Writer::new(&mut out);
    <LongArray as ArrayOf<u64>>::write_entry(&[1, u64::MAX], "data", &mut writer).unwrap();

    // The same document the array type writes for the same values.
    let mut expected = Vec::new();
    let mut writer = Writer::new(&mut expected);
    ToNBT::write_entry(&LongArray::new(vec![1, -1]), "data", &mut writer).unwrap();
    assert_eq!(out, expected);

    let mut reader = Reader::new(&out, DeOpts::default());
    let tag = reader.read_tag().unwrap();
    reader.skip_name().unwrap();
    assert_eq!(
        <LongArray as ArrayOf<u64>>::read(tag, &mut reader).unwrap(),
        vec![1, u64::MAX]
    );

    // A tag of another array kind is refused.
    let mut reader = Reader::new(&out, DeOpts::default());
    reader.read_tag().unwrap();
    reader.skip_name().unwrap();
    assert!(<LongArray as ArrayOf<u64>>::read(TAG_INT_ARRAY, &mut reader).is_err());
}

#[test]
fn array_fields_are_written_without_allocating() {
    #[derive(ToNBT)]
    struct Holder {
        #[nbt(array = "long")]
        data: Vec<u64>,
    }

    let holder = Holder {
        data: vec![1; 10_000],
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
