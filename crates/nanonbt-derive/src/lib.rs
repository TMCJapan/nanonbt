//! `#[derive(FromNBT)]` and `#[derive(ToNBT)]` for nanonbt.
//!
//! ```ignore
//! #[derive(FromNBT, ToNBT)]
//! struct Player {
//!     health: i32,
//!     #[nbt(rename = "Name")]
//!     name: String,
//!     #[nbt(ignore)]
//!     cached: u64,
//! }
//! ```
//!
//! Container attributes:
//! * `crate = path`: where nanonbt is (default `::nanonbt`).
//!
//! Field attributes:
//! * `ignore`: leave the field out; it reads as `Default::default()`.
//! * `rename = "name"`: the compound entry's name (default: the field name).
//!   The name is encoded as modified UTF-8 when the macro runs, so writing
//!   and reading it converts nothing at run time, even when the name holds a
//!   NUL or a non-BMP character, whose modified spelling is not its UTF-8.
//! * `array = "byte" | "int" | "long"`: write a `Vec<T>`, `[T; N]` or `&[T]`
//!   field as an NBT array of that kind, and read it back as one, instead of
//!   the list a sequence writes by default. `T` is `i8` or `u8` for `"byte"`,
//!   `i32` or `u32` for `"int"`, and `i64` or `u64` for `"long"`. A borrowed
//!   `&[T]` reads through its own `FromNBT`, which byte slices have and wider
//!   integers do not, their bytes being big-endian: use `Vec<T>` or `[T; N]`
//!   to read ints and longs.
//!
//! Variant attributes:
//! * `rename = "name"`: the string an enum variant reads and writes as.
//!
//! Names must be unique: two fields, or two variants, that read and write as
//! the same name are rejected, since a read could not tell them apart. An
//! `ignore`d field is left out of the compound, so its name never clashes.
//!
//! With the `hashify` feature (enable `nanonbt/hashify`), a read dispatches
//! the names it sees through
//! [`hashify`](https://crates.io/crates/hashify)'s perfect hash lookups
//! instead of a `match`. The arms are the same expressions either way, so
//! an arm can still `?` its error.
//!
//! Supported shapes are named-field structs, single-field tuple structs
//! (transparent), and enums of unit variants. A field of type `Option<T>` is
//! left out of the compound when it is `None`; `T` needs no `Option`
//! implementation of its own.

mod attrs;
mod codegen;
mod model;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Implements [`nanonbt::ToNBT`] for a struct, newtype or unit enum.
#[proc_macro_derive(ToNBT, attributes(nbt))]
pub fn derive_to_nbt(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match model::Model::new(&input) {
        Ok(model) => codegen::to_nbt(&model).into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Implements [`nanonbt::FromNBT`] for a struct, newtype or unit enum.
#[proc_macro_derive(FromNBT, attributes(nbt))]
pub fn derive_from_nbt(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match model::Model::new(&input) {
        Ok(model) => codegen::from_nbt(&model).into(),
        Err(error) => error.to_compile_error().into(),
    }
}
