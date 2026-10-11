//! Encoding routes for NBT arrays and the type-level mapping from element
//! types to routes.

#[cfg(feature = "alloc")]
use alloc::borrow::{Cow, ToOwned};
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::{I32Be, I64Be, Tag};

/// A type-level marker selecting how a sequence is encoded in NBT.
///
/// NBT has three dedicated array tags (`TAG_Byte_Array`, `TAG_Int_Array`,
/// `TAG_Long_Array`) in addition to the generic `TAG_List`. A Rust type such
/// as `Vec<i32>` therefore does not determine its encoding on its own; the
/// encoding is an explicit *route* chosen by a marker type implementing this
/// trait. The derive macro maps `#[nbt(array = int)]` to [`IntArray`] and
/// friends.
pub trait ArrayRoute {
    /// The NBT tag this route writes.
    const TAG: Tag;
}

/// `TAG_Byte_Array` (7): a length-prefixed array of bytes.
pub enum ByteArray {}

/// `TAG_Int_Array` (11): a length-prefixed array of big-endian `i32`s.
pub enum IntArray {}

/// `TAG_Long_Array` (12): a length-prefixed array of big-endian `i64`s.
pub enum LongArray {}

impl ArrayRoute for ByteArray {
    const TAG: Tag = Tag::ByteArray;
}

impl ArrayRoute for IntArray {
    const TAG: Tag = Tag::IntArray;
}

impl ArrayRoute for LongArray {
    const TAG: Tag = Tag::LongArray;
}

/// Maps an element type to its natural array route.
///
/// This is what makes `#[nbt(array)]` (without a kind) work: the element type
/// of the sequence decides between byte, int, and long arrays. Types without a
/// natural route (`f32`, `u16`, ...) do not implement this trait, so the
/// derive rejects them at compile time.
pub trait ArrayElem {
    /// The route inferred for this element type.
    type Route: ArrayRoute;
}

impl ArrayElem for u8 {
    type Route = ByteArray;
}

impl ArrayElem for i8 {
    type Route = ByteArray;
}

impl ArrayElem for i32 {
    type Route = IntArray;
}

impl ArrayElem for I32Be {
    type Route = IntArray;
}

impl ArrayElem for i64 {
    type Route = LongArray;
}

impl ArrayElem for I64Be {
    type Route = LongArray;
}

/// Maps a sequence type to its natural array route.
///
/// The derive macro uses this for `#[nbt(array)]`: the attribute on a field of
/// type `Vec<i32>` expands to `<Vec<i32> as ArrayRouteFor>::Route`.
pub trait ArrayRouteFor {
    /// The route inferred for this sequence type.
    type Route: ArrayRoute;
}

impl<T: ArrayElem> ArrayRouteFor for [T] {
    type Route = T::Route;
}

#[cfg(feature = "alloc")]
impl<T: ArrayElem> ArrayRouteFor for Vec<T> {
    type Route = T::Route;
}

impl<T: ArrayElem> ArrayRouteFor for &[T] {
    type Route = T::Route;
}

#[cfg(feature = "alloc")]
impl<T: ArrayElem> ArrayRouteFor for Cow<'_, [T]>
where
    [T]: ToOwned,
{
    type Route = T::Route;
}
