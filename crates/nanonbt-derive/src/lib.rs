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
//!
//! Variant attributes:
//! * `rename = "name"`: the string an enum variant reads and writes as.
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
