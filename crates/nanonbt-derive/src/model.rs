//! The validated description of a type to generate code for.

use std::collections::BTreeMap;

use proc_macro2::Span;
use syn::{Data, DeriveInput, Error, Fields, Generics, Ident, Path, Result, Type, TypePath};

use crate::attrs::{self, ArrayKind};

pub struct Model {
    pub ident: Ident,
    pub krate: Path,
    pub generics: Generics,
    pub shape: Shape,
}

pub enum Shape {
    /// Named fields in declaration order, each with its compound name.
    Struct(Vec<Field>),
    /// A single unnamed field, which reads and writes transparently.
    Newtype(Box<Type>),
    /// Unit variants in declaration order, each with its string.
    Enum(Vec<Variant>),
}

pub struct Field {
    pub ident: Ident,
    pub ty: Type,
    pub name: String,
    /// Where the name was written: the `rename` value, or the field itself.
    pub name_span: Span,
    pub ignore: bool,
    /// `#[nbt(array = "...")]`, if the field has one.
    pub array: Option<Array>,
}

/// How an `array` field holds its elements, which decides how it reads.
pub enum ArrayContainer {
    /// A `Vec<T>`, read through `From<Array>`.
    Vec,
    /// A `[T; N]`, read through `TryFrom<Array>`, which checks the length.
    Fixed,
    /// A `&[T]`, read through the field type's own `FromNBT`.
    Borrowed,
}

/// An `#[nbt(array = "...")]` field: the NBT array it is written as.
pub struct Array {
    pub kind: ArrayKind,
    pub container: ArrayContainer,
    /// The element type, as written: `i64` for a `Vec<i64>`.
    pub element: Type,
}

pub struct Variant {
    pub ident: Ident,
    pub name: String,
    /// Where the name was written: the `rename` value, or the variant.
    pub name_span: Span,
}

impl Model {
    pub fn new(input: &DeriveInput) -> Result<Self> {
        let krate = attrs::container(&input.attrs)?
            .krate
            .unwrap_or_else(|| syn::parse_quote!(::nanonbt));
        let shape = match &input.data {
            Data::Struct(data) => match &data.fields {
                Fields::Named(named) => {
                    let fields = named
                        .named
                        .iter()
                        .map(|field| {
                            let attrs = attrs::field(&field.attrs)?;
                            let ident = field.ident.clone().expect("named field");
                            if option_inner(&field.ty).and_then(option_inner).is_some() {
                                return Err(Error::new_spanned(
                                    &field.ty,
                                    "nested `Option` fields are not supported",
                                ));
                            }
                            let (name, name_span) = attrs.rename.as_ref().map_or_else(
                                || (unraw(&ident), ident.span()),
                                |rename| (rename.value(), rename.span()),
                            );
                            let array = attrs
                                .array
                                .map(|kind| {
                                    array_of(kind, option_inner(&field.ty).unwrap_or(&field.ty))
                                })
                                .transpose()?;
                            Ok(Field {
                                ident,
                                ty: field.ty.clone(),
                                name,
                                name_span,
                                ignore: attrs.ignore,
                                array,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    Shape::Struct(fields)
                }
                Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
                    Shape::Newtype(Box::new(unnamed.unnamed[0].ty.clone()))
                }
                _ => {
                    return Err(Error::new_spanned(
                        &input.ident,
                        "expected a struct with named fields or a single-field tuple struct",
                    ));
                }
            },
            Data::Enum(data) => {
                let variants = data
                    .variants
                    .iter()
                    .map(|variant| {
                        if !matches!(variant.fields, Fields::Unit) {
                            return Err(Error::new_spanned(variant, "expected a unit variant"));
                        }
                        let attrs = attrs::variant(&variant.attrs)?;
                        let ident = variant.ident.clone();
                        let (name, name_span) = attrs.rename.as_ref().map_or_else(
                            || (ident.to_string(), ident.span()),
                            |rename| (rename.value(), rename.span()),
                        );
                        Ok(Variant {
                            ident,
                            name,
                            name_span,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Shape::Enum(variants)
            }
            Data::Union(_) => {
                return Err(Error::new_spanned(
                    &input.ident,
                    "expected a struct or an enum",
                ));
            }
        };
        check_names(&shape)?;
        Ok(Self {
            ident: input.ident.clone(),
            krate,
            generics: input.generics.clone(),
            shape,
        })
    }
}

/// The field name without a raw-identifier prefix.
fn unraw(ident: &Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").unwrap_or(&name).to_owned()
}

/// Refuses two fields, or two variants, that share the NBT name they read
/// and write as: a read could not tell them apart, one arm being
/// unreachable, and `hashify`'s lookups reject the duplicate outright.
fn check_names(shape: &Shape) -> Result<()> {
    match shape {
        Shape::Struct(fields) => unique_names(
            "field",
            fields
                .iter()
                .filter(|field| !field.ignore)
                .map(|field| (field.name_span, &field.ident, field.name.as_str())),
        ),
        Shape::Enum(variants) => unique_names(
            "variant",
            variants
                .iter()
                .map(|variant| (variant.name_span, &variant.ident, variant.name.as_str())),
        ),
        Shape::Newtype(_) => Ok(()),
    }
}

/// The duplicate check: a name already seen is refused at the place the
/// second one was written.
fn unique_names<'a>(
    kind: &str,
    entries: impl Iterator<Item = (Span, &'a Ident, &'a str)>,
) -> Result<()> {
    let mut seen: BTreeMap<&'a str, &'a Ident> = BTreeMap::new();
    for (span, ident, name) in entries {
        if let Some(first) = seen.get(name) {
            return Err(Error::new(
                span,
                format!("duplicate NBT name `{name}`: already used by {kind} `{first}`"),
            ));
        }
        seen.insert(name, ident);
    }
    Ok(())
}

/// Describes an `#[nbt(array = "...")]` field from the type it is written as.
fn array_of(kind: ArrayKind, ty: &Type) -> Result<Array> {
    let (container, element) = match ty {
        Type::Array(array) => (ArrayContainer::Fixed, (*array.elem).clone()),
        Type::Reference(reference) => match &*reference.elem {
            Type::Slice(slice) => (ArrayContainer::Borrowed, (*slice.elem).clone()),
            elem => return Err(unsupported_array_field(elem)),
        },
        Type::Path(path) => match vec_element(path) {
            Some(element) => (ArrayContainer::Vec, element.clone()),
            None => return Err(unsupported_array_field(ty)),
        },
        _ => return Err(unsupported_array_field(ty)),
    };
    check_element(kind, &element)?;
    Ok(Array {
        kind,
        container,
        element,
    })
}

/// The integer primitives and big-endian wrappers that name an NBT array's
/// elements.
const ARRAY_ELEMENTS: [&str; 10] = [
    "i8", "u8", "i32", "u32", "i64", "u64", "I32Be", "U32Be", "I64Be", "U64Be",
];

/// The other primitives, which no NBT array holds.
const OTHER_PRIMITIVES: [&str; 9] = [
    "i16", "u16", "i128", "u128", "f32", "f64", "bool", "char", "String",
];

/// The element type names an NBT array can hold, by kind.
const fn expected_elements(kind: ArrayKind) -> &'static [&'static str] {
    match kind {
        ArrayKind::Byte => &["i8", "u8"],
        ArrayKind::Int => &["i32", "u32", "I32Be", "U32Be"],
        ArrayKind::Long => &["i64", "u64", "I64Be", "U64Be"],
    }
}

/// Refuses a known primitive that the kind cannot hold. A name that might be
/// an alias or a generic parameter is left to the generated bounds.
fn check_element(kind: ArrayKind, element: &Type) -> Result<()> {
    let Some(name) = element_name(element) else {
        return Ok(());
    };
    let expected = expected_elements(kind);
    if expected.contains(&name.as_str()) {
        return Ok(());
    }
    if ARRAY_ELEMENTS.contains(&name.as_str()) || OTHER_PRIMITIVES.contains(&name.as_str()) {
        let expected = expected.join(", ");
        return Err(Error::new_spanned(
            element,
            format!(
                "an `array = \"{}\"` field holds {expected} elements, not `{name}`",
                kind.name()
            ),
        ));
    }
    Ok(())
}

/// A primitive type name written as the last segment of a plain path.
fn element_name(element: &Type) -> Option<String> {
    let Type::Path(path) = element else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if !matches!(segment.arguments, syn::PathArguments::None) {
        return None;
    }
    Some(segment.ident.to_string())
}

fn unsupported_array_field(ty: &Type) -> Error {
    Error::new_spanned(
        ty,
        "an `array` field must be a `Vec<T>`, a `[T; N]` or a `&[T]`",
    )
}

/// `Vec<T>` written as a path's last segment, if it is one.
fn vec_element(path: &TypePath) -> Option<&Type> {
    let segment = path.path.segments.last()?;
    if segment.ident != "Vec" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    match args.args.first()? {
        syn::GenericArgument::Type(element) => Some(element),
        _ => None,
    }
}

/// `Option<T>` written as a path's last segment, if it is one.
pub fn option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    match args.args.first()? {
        syn::GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(source: &str) -> Result<Model> {
        Model::new(&syn::parse_str::<DeriveInput>(source).expect("parse"))
    }

    #[test]
    fn duplicate_renames_are_refused() {
        let error = model(
            "struct S {
                #[nbt(rename = \"x\")]
                a: u8,
                #[nbt(rename = \"x\")]
                b: u8,
            }",
        )
        .err()
        .expect("duplicate names");
        assert_eq!(
            error.to_string(),
            "duplicate NBT name `x`: already used by field `a`"
        );
    }

    #[test]
    fn a_default_name_and_a_rename_clash() {
        let error = model("struct S { a: u8, #[nbt(rename = \"a\")] b: u8 }")
            .err()
            .expect("duplicate names");
        assert_eq!(
            error.to_string(),
            "duplicate NBT name `a`: already used by field `a`"
        );
    }

    #[test]
    fn ignored_fields_reserve_no_name() {
        model("struct S { #[nbt(ignore)] a: u8, #[nbt(rename = \"a\")] b: u8 }")
            .expect("ignored names do not clash");
    }

    #[test]
    fn duplicate_variant_renames_are_refused() {
        let error = model("enum E { #[nbt(rename = \"x\")] A, #[nbt(rename = \"x\")] B }")
            .err()
            .expect("duplicate names");
        assert_eq!(
            error.to_string(),
            "duplicate NBT name `x`: already used by variant `A`"
        );
    }
}
