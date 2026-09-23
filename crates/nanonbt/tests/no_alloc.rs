//! Renamed names are read and written without allocating.
//!
//! A name with a NUL or a non-BMP character has a different modified UTF-8
//! spelling than its `str`, which used to be decoded into an owned `String`
//! when reading and encoded into a `Vec` when writing. The derive encodes
//! the name when the macro runs and matches the raw bytes instead; this
//! counts allocations so a conversion cannot come back unnoticed.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use nanonbt::{FromNBT, ToNBT, Writer, from_bytes, to_bytes};

/// Counts every allocation in the process.
struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every method forwards to `System` with the same arguments, after
// counting the call.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// The allocations `f` makes. Only meaningful while one test runs, which is
/// why this file holds exactly the test below.
fn allocations(f: impl FnOnce()) -> usize {
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    f();
    ALLOCATIONS.load(Ordering::Relaxed) - before
}

#[derive(FromNBT, ToNBT, PartialEq, Debug)]
enum Kind {
    #[nbt(rename = "😀")]
    Face,
}

#[derive(FromNBT, ToNBT, PartialEq, Debug)]
struct Renamed {
    #[nbt(rename = "😀")]
    face: i32,
    #[nbt(rename = "名前\0\u{1f600}")]
    mixed: i8,
    kind: Kind,
}

#[test]
fn renamed_names_are_read_and_written_without_allocating() {
    let value = Renamed {
        face: 7,
        mixed: -1,
        kind: Kind::Face,
    };
    let bytes = to_bytes(&value).unwrap();
    assert_eq!(from_bytes::<Renamed>(&bytes).unwrap(), value);

    let read = allocations(|| {
        let back = from_bytes::<Renamed>(&bytes).unwrap();
        assert_eq!(back, value);
    });
    assert_eq!(read, 0, "reading a renamed struct allocated");

    // The output is preallocated, so only the conversion itself could
    // allocate while the struct is written.
    let mut out = Vec::with_capacity(bytes.len());
    let written = allocations(|| {
        let mut writer = Writer::new(&mut out);
        value.write(&mut writer).unwrap();
    });
    assert_eq!(written, 0, "writing a renamed struct allocated");
}
