//! `#[derive(SerializeNbt)]` and `#[derive(DeserializeNbt)]` for `nanonbt`.
//!
//! Named-field structs become `TAG_Compound`s. On decode, fields are matched
//! by name, so field order does not matter and unknown fields are skipped.
//!
//! # Field attributes
//!
//! * `#[nbt(array)]` — encode/decode the field as an NBT array, inferring the
//!   route from the element type (`Vec<i32>` becomes `TAG_Int_Array`).
//! * `#[nbt(array = byte | int | long)]` — force a `TAG_Byte_Array`,
//!   `TAG_Int_Array` or `TAG_Long_Array` route.
//! * `#[nbt(rename = "name")]` — use a different compound key.
//! * `#[nbt(skip)]` — omit the field when writing and fill it with
//!   [`Default::default()`] when reading.
//!
//! # Borrowed fields
//!
//! A struct with a borrowed field such as `&'de [u8]`, `&'de [I32Be]`,
//! `Cow<'de, [u8]>` or a nested borrowing struct gets an implementation of
//! `DeserializeNbtRef<'de>` instead of `DeserializeNbt`, so it can only be
//! decoded from a reader that supports zero-copy. Borrowed fields must carry
//! `#[nbt(array = ...)]`, and the struct must declare the lifetime used for
//! borrowing (by convention `'de`, the first lifetime parameter).
//!
//! Newtype structs are transparent (their payload is the inner value); unit
//! structs are empty compounds. Enums, unions and multi-field tuple structs
//! are rejected with a compile error.

use std::collections::{BTreeSet, HashSet};

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, quote_spanned};
use syn::ext::IdentExt;
use syn::parse_quote;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{
    Data, DeriveInput, Field, Fields, FieldsUnnamed, GenericParam, Generics, Ident, Lifetime,
    LitStr, PathArguments, Token, Type, TypeParamBound, parse_macro_input,
};

/// Derives `nanonbt::SerializeNbt` for a struct.
#[proc_macro_derive(SerializeNbt, attributes(nbt))]
pub fn derive_serialize_nbt(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_serialize(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives `nanonbt::DeserializeNbt`, or `nanonbt::DeserializeNbtRef` for
/// structs with borrowed fields.
#[proc_macro_derive(DeserializeNbt, attributes(nbt))]
pub fn derive_deserialize_nbt(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_deserialize(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

// ------------------------------------------------------------- attributes

#[derive(Default)]
struct FieldAttrs {
    array: Option<ArraySpec>,
    skip: bool,
    rename: Option<String>,
}

#[derive(Clone)]
enum ArraySpec {
    /// `#[nbt(array)]`: infer the route from the element type.
    Inferred,
    /// `#[nbt(array = byte | int | long)]`.
    Kind(Ident),
}

impl ArraySpec {
    fn route_type(&self, ty: &Type) -> TokenStream2 {
        match self {
            Self::Kind(kind) => match kind.to_string().as_str() {
                "byte" => quote!(::nanonbt::route::ByteArray),
                "int" => quote!(::nanonbt::route::IntArray),
                _ => quote!(::nanonbt::route::LongArray),
            },
            Self::Inferred => {
                let span = ty.span();
                quote_spanned!(span=> <#ty as ::nanonbt::ArrayRouteFor>::Route)
            }
        }
    }
}

fn parse_field_attrs(field: &Field) -> syn::Result<FieldAttrs> {
    let mut attrs = FieldAttrs::default();
    for attr in &field.attrs {
        if !attr.path().is_ident("nbt") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("array") {
                if attrs.array.is_some() {
                    return Err(meta.error("duplicate `array` attribute"));
                }
                attrs.array = Some(if meta.input.peek(Token![=]) {
                    let kind: Ident = meta.value()?.parse()?;
                    match kind.to_string().as_str() {
                        "byte" | "int" | "long" => {}
                        other => {
                            return Err(syn::Error::new(
                                kind.span(),
                                format!(
                                    "unknown array kind `{other}`; expected `byte`, `int`, or `long`"
                                ),
                            ));
                        }
                    }
                    ArraySpec::Kind(kind)
                } else {
                    ArraySpec::Inferred
                });
                Ok(())
            } else if meta.path.is_ident("skip") {
                if attrs.skip {
                    return Err(meta.error("duplicate `skip` attribute"));
                }
                attrs.skip = true;
                Ok(())
            } else if meta.path.is_ident("rename") {
                let value: LitStr = meta.value()?.parse()?;
                attrs.rename = Some(value.value());
                Ok(())
            } else {
                Err(meta.error(
                    "unsupported `nbt` attribute; expected `array`, `skip`, or `rename`",
                ))
            }
        })?;
    }
    Ok(attrs)
}

fn field_key(ident: &Ident, attrs: &FieldAttrs) -> LitStr {
    attrs.rename.as_ref().map_or_else(
        || LitStr::new(&ident.unraw().to_string(), ident.span()),
        |name| LitStr::new(name, ident.span()),
    )
}

// ---------------------------------------------------------------- helpers

/// Visitor that checks whether a type mentions a given lifetime.
struct LifetimeFinder<'a> {
    target: &'a Lifetime,
    found: bool,
}

impl<'ast> Visit<'ast> for LifetimeFinder<'_> {
    fn visit_lifetime(&mut self, node: &'ast Lifetime) {
        if node.ident == self.target.ident {
            self.found = true;
        }
    }
}

/// Visitor that collects the generic type parameters mentioned by a type.
struct ParamCollector<'a> {
    params: &'a HashSet<String>,
    found: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for ParamCollector<'_> {
    fn visit_ident(&mut self, node: &'ast Ident) {
        let name = node.unraw().to_string();
        if self.params.contains(&name) {
            self.found.insert(name);
        }
    }
}

fn expect_struct(input: &DeriveInput) -> syn::Result<&Fields> {
    match &input.data {
        Data::Struct(data) => Ok(&data.fields),
        Data::Enum(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "`nanonbt` derives do not support enums",
        )),
        Data::Union(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "`nanonbt` derives do not support unions",
        )),
    }
}

fn field_list(fields: &Fields) -> Vec<&Field> {
    match fields {
        Fields::Named(named) => named.named.iter().collect(),
        Fields::Unnamed(unnamed) => unnamed.unnamed.iter().collect(),
        Fields::Unit => Vec::new(),
    }
}

/// Validates that a tuple struct has exactly one field and returns it.
///
/// Newtype structs are transparent. Other arities are rejected here so the
/// derive never indexes into an empty field list or silently mishandles a
/// multi-field tuple.
fn expect_newtype<'a>(ident: &Ident, unnamed: &'a FieldsUnnamed) -> syn::Result<&'a Field> {
    match unnamed.unnamed.len() {
        1 => Ok(&unnamed.unnamed[0]),
        0 => Err(syn::Error::new_spanned(
            ident,
            "zero-field tuple structs are not supported; use a unit struct instead",
        )),
        _ => Err(syn::Error::new_spanned(
            ident,
            "tuple structs with more than one field are not supported; use named fields",
        )),
    }
}

/// Rejects fields that would share a compound key.
///
/// Two fields can collide through `#[nbt(rename = "...")]`; without this check
/// the generated decoder would silently ignore one of them at runtime.
fn check_duplicate_keys(fields: &Fields) -> syn::Result<()> {
    let mut seen: HashSet<String> = HashSet::new();
    for field in field_list(fields) {
        let attrs = parse_field_attrs(field)?;
        if attrs.skip {
            continue;
        }
        let Some(ident) = field.ident.as_ref() else {
            continue;
        };
        let key = field_key(ident, &attrs);
        if !seen.insert(key.value()) {
            return Err(syn::Error::new_spanned(
                field,
                format!("duplicate NBT field name `{}`", key.value()),
            ));
        }
    }
    Ok(())
}

const fn is_reference(ty: &Type) -> bool {
    matches!(ty, Type::Reference(_))
}

fn is_slice_reference(ty: &Type) -> bool {
    matches!(ty, Type::Reference(reference) if matches!(&*reference.elem, Type::Slice(_)))
}

fn cow_argument(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Cow" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        syn::GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn is_cow(ty: &Type) -> bool {
    cow_argument(ty).is_some()
}

fn is_cow_slice(ty: &Type) -> bool {
    matches!(cow_argument(ty), Some(Type::Slice(_)))
}

fn type_mentions_lifetime(ty: &Type, lifetime: &Lifetime) -> bool {
    let mut finder = LifetimeFinder {
        target: lifetime,
        found: false,
    };
    finder.visit_type(ty);
    finder.found
}

fn used_type_params<'a>(types: impl Iterator<Item = &'a Type>, generics: &Generics) -> Vec<Ident> {
    let params: HashSet<String> = generics
        .type_params()
        .map(|param| param.ident.to_string())
        .collect();
    if params.is_empty() {
        return Vec::new();
    }

    let mut collector = ParamCollector {
        params: &params,
        found: BTreeSet::new(),
    };
    for ty in types {
        collector.visit_type(ty);
    }

    generics
        .type_params()
        .filter(|param| collector.found.contains(&param.ident.to_string()))
        .map(|param| param.ident.clone())
        .collect()
}

fn add_bound(generics: &mut Generics, param: &Ident, bound: TypeParamBound) {
    for generic in &mut generics.params {
        if let GenericParam::Type(type_param) = generic
            && type_param.ident == *param
        {
            type_param.bounds.push(bound);
            return;
        }
    }
}

// -------------------------------------------------------------- serialize

fn expand_serialize(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let fields = expect_struct(input)?;
    check_duplicate_keys(fields)?;

    let mut used_types: Vec<Type> = Vec::new();
    let mut tag: TokenStream2 = quote!(::nanonbt::Tag::Compound);
    let body: TokenStream2;

    match fields {
        Fields::Named(named) => {
            let mut writes = Vec::new();
            for field in &named.named {
                let attrs = parse_field_attrs(field)?;
                if attrs.skip {
                    continue;
                }
                let ident = field.ident.as_ref().expect("named field");
                let ty = &field.ty;
                used_types.push(ty.clone());
                let key = field_key(ident, &attrs);
                let value = quote!(&self.#ident);
                let (tag_byte, payload) = serialize_field(ty, &attrs, &value);
                writes.push(quote! {
                    __w.write_bytes(&[#tag_byte]);
                    ::nanonbt::write_string(__w, #key)?;
                    #payload
                });
            }
            body = quote! {
                #(#writes)*
                __w.write_bytes(&[::nanonbt::Tag::End.as_u8()]);
            };
        }
        Fields::Unnamed(unnamed) => {
            let field = expect_newtype(&input.ident, unnamed)?;
            let attrs = parse_field_attrs(field)?;
            let ty = &field.ty;
            used_types.push(ty.clone());
            let span = ty.span();
            let value = quote!(&self.0);
            if let Some(array) = &attrs.array {
                let route = array.route_type(ty);
                tag = quote_spanned!(span=> <#route as ::nanonbt::ArrayRoute>::TAG);
                body = quote_spanned!(span=> <#ty as ::nanonbt::SerializeNbtArray<#route>>::serialize_nbt_array(#value, __w)?;);
            } else {
                tag = quote_spanned!(span=> <#ty as ::nanonbt::SerializeNbt>::TAG);
                body = quote_spanned!(span=> <#ty as ::nanonbt::SerializeNbt>::serialize_nbt(#value, __w)?;);
            }
        }
        Fields::Unit => {
            body = quote! {
                __w.write_bytes(&[::nanonbt::Tag::End.as_u8()]);
            };
        }
    }

    let mut generics = input.generics.clone();
    for param in used_type_params(used_types.iter(), &input.generics) {
        add_bound(&mut generics, &param, parse_quote!(::nanonbt::SerializeNbt));
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::nanonbt::SerializeNbt for #name #ty_generics #where_clause {
            const TAG: ::nanonbt::Tag = #tag;

            fn serialize_nbt<__W: ::nanonbt::Write + ?Sized>(
                &self,
                __w: &mut __W,
            ) -> ::nanonbt::Result<()> {
                #body
                Ok(())
            }
        }
    })
}

#[allow(clippy::option_if_let_else)]
fn serialize_field(
    ty: &Type,
    attrs: &FieldAttrs,
    value: &TokenStream2,
) -> (TokenStream2, TokenStream2) {
    let span = ty.span();
    if let Some(array) = &attrs.array {
        let route = array.route_type(ty);
        (
            quote_spanned!(span=> <#route as ::nanonbt::ArrayRoute>::TAG.as_u8()),
            quote_spanned!(span=> <#ty as ::nanonbt::SerializeNbtArray<#route>>::serialize_nbt_array(#value, __w)?;),
        )
    } else {
        (
            quote_spanned!(span=> <#ty as ::nanonbt::SerializeNbt>::TAG.as_u8()),
            quote_spanned!(span=> <#ty as ::nanonbt::SerializeNbt>::serialize_nbt(#value, __w)?;),
        )
    }
}

// ------------------------------------------------------------ deserialize

#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldMode {
    /// Not part of the wire format; filled with `Default::default()`.
    Skipped,
    /// Decoded with `DeserializeNbt`.
    Owned,
    /// `&'de [T]` / `Cow<'de, [T]>`, decoded with `DeserializeNbtArrayRef`.
    BorrowedArray,
    /// A nested borrowing type, decoded with `DeserializeNbtRef`.
    Nested,
}

struct FieldInfo {
    index: usize,
    ident: Option<Ident>,
    ty: Type,
    attrs: FieldAttrs,
    mode: FieldMode,
}

fn expand_deserialize(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let fields = expect_struct(input)?;
    if let Fields::Unnamed(unnamed) = fields {
        expect_newtype(&input.ident, unnamed)?;
    }
    check_duplicate_keys(fields)?;

    let borrow_lifetime = input
        .generics
        .lifetimes()
        .next()
        .map(|param| param.lifetime.clone());

    let mut infos = Vec::new();
    let mut borrowed = false;
    for (index, field) in field_list(fields).into_iter().enumerate() {
        let attrs = parse_field_attrs(field)?;
        let ty = field.ty.clone();
        let mode = if attrs.skip {
            FieldMode::Skipped
        } else if is_reference(&ty) || is_cow(&ty) {
            borrowed = true;
            FieldMode::BorrowedArray
        } else if let Some(lifetime) = &borrow_lifetime {
            if type_mentions_lifetime(&ty, lifetime) {
                borrowed = true;
                FieldMode::Nested
            } else {
                FieldMode::Owned
            }
        } else {
            FieldMode::Owned
        };

        if mode == FieldMode::BorrowedArray {
            if attrs.array.is_none() {
                return Err(syn::Error::new_spanned(
                    field,
                    "borrowed fields require `#[nbt(array = byte|int|long)]` (or `#[nbt(array)]`), \
                     because a slice cannot be decoded as a list without copying",
                ));
            }
            if is_reference(&ty) && !is_slice_reference(&ty) {
                return Err(syn::Error::new_spanned(
                    field,
                    "borrowed fields must be slice references such as `&'de [u8]`",
                ));
            }
            if is_cow(&ty) && !is_cow_slice(&ty) {
                return Err(syn::Error::new_spanned(
                    field,
                    "borrowed `Cow` fields must wrap a slice, such as `Cow<'de, [u8]>`",
                ));
            }
            if borrow_lifetime.is_none() {
                return Err(syn::Error::new_spanned(
                    field,
                    "borrowed fields require the struct to declare a lifetime, \
                     such as `struct S<'de> { ... }`",
                ));
            }
        }

        infos.push(FieldInfo {
            index,
            ident: field.ident.clone(),
            ty,
            attrs,
            mode,
        });
    }

    build_decode_impl(input, fields, &infos, borrow_lifetime.as_ref(), borrowed)
}

fn build_decode_impl(
    input: &DeriveInput,
    fields: &Fields,
    infos: &[FieldInfo],
    borrow_lifetime: Option<&Lifetime>,
    borrowed: bool,
) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let mut tag: TokenStream2 = quote!(::nanonbt::Tag::Compound);
    let body: TokenStream2;
    let mut owned_types: Vec<Type> = Vec::new();
    let mut nested_types: Vec<Type> = Vec::new();

    match fields {
        Fields::Named(_) | Fields::Unit => {
            let mut declarations = Vec::new();
            let mut arms = Vec::new();
            let mut inits = Vec::new();

            for info in infos {
                let Some(ident) = &info.ident else {
                    continue;
                };

                if info.mode == FieldMode::Skipped {
                    inits.push(quote! {
                        #ident: ::core::default::Default::default(),
                    });
                    continue;
                }

                let var = format_ident!("__nbt_field_{}", info.index);
                let ty = &info.ty;
                let key = field_key(ident, &info.attrs);
                let (expected, decode) = decode_parts(info, borrow_lifetime);

                match info.mode {
                    FieldMode::Owned => owned_types.push(ty.clone()),
                    FieldMode::Nested => nested_types.push(ty.clone()),
                    _ => {}
                }

                declarations.push(quote! {
                    let mut #var: ::core::option::Option<#ty> = ::core::option::Option::None;
                });
                arms.push(quote! {
                    #key => {
                        if __nbt_tag != #expected {
                            return ::core::result::Result::Err(::nanonbt::Error::InvalidTag {
                                expected: #expected,
                                actual: __nbt_tag.as_u8(),
                            });
                        }
                        #var = ::core::option::Option::Some(#decode);
                    }
                });
                inits.push(quote! {
                    #ident: #var.ok_or(::nanonbt::Error::MissingField(#key))?,
                });
            }

            let construct = if matches!(fields, Fields::Unit) {
                quote!(::core::result::Result::Ok(Self))
            } else {
                quote!(::core::result::Result::Ok(Self { #(#inits)* }))
            };
            body = quote! {
                #(#declarations)*
                let mut __nbt_name = ::alloc::string::String::new();
                loop {
                    let __nbt_tag = ::nanonbt::Tag::from_u8(__r.read_u8()?)?;
                    if __nbt_tag == ::nanonbt::Tag::End {
                        break;
                    }
                    ::nanonbt::read_string_into(__r, &mut __nbt_name)?;
                    match __nbt_name.as_str() {
                        #(#arms)*
                        _ => ::nanonbt::skip_nbt(__r, __nbt_tag)?,
                    }
                }
                #construct
            };
        }
        Fields::Unnamed(_) => {
            let info = &infos[0];
            let ty = &info.ty;
            let span = ty.span();
            if info.mode == FieldMode::Skipped {
                return Err(syn::Error::new_spanned(
                    ty,
                    "`#[nbt(skip)]` is not supported on newtype fields",
                ));
            }
            body = match info.mode {
                FieldMode::Owned => {
                    owned_types.push(ty.clone());
                    if let Some(array) = &info.attrs.array {
                        let route = array.route_type(ty);
                        tag = quote_spanned!(span=> <#route as ::nanonbt::ArrayRoute>::TAG);
                        quote_spanned!(span=> ::core::result::Result::Ok(Self(<#ty as ::nanonbt::DeserializeNbtArray<#route>>::deserialize_nbt_array(__r)?)))
                    } else {
                        tag = quote_spanned!(span=> <#ty as ::nanonbt::DeserializeNbt>::TAG);
                        quote_spanned!(span=> ::core::result::Result::Ok(Self(<#ty as ::nanonbt::DeserializeNbt>::deserialize_nbt(__r)?)))
                    }
                }
                FieldMode::BorrowedArray => {
                    let lifetime = borrow_lifetime.expect("validated borrowed field");
                    let route = info
                        .attrs
                        .array
                        .as_ref()
                        .expect("validated borrowed field")
                        .route_type(ty);
                    tag = quote_spanned!(span=> <#route as ::nanonbt::ArrayRoute>::TAG);
                    quote_spanned!(span=> ::core::result::Result::Ok(Self(<#ty as ::nanonbt::DeserializeNbtArrayRef<#lifetime, #route>>::deserialize_nbt_array_ref(__r)?)))
                }
                FieldMode::Nested => {
                    let lifetime = borrow_lifetime.expect("validated nested field");
                    nested_types.push(ty.clone());
                    tag = quote_spanned!(span=> <#ty as ::nanonbt::DeserializeNbtRef<#lifetime>>::TAG);
                    quote_spanned!(span=> ::core::result::Result::Ok(Self(<#ty as ::nanonbt::DeserializeNbtRef<#lifetime>>::deserialize_nbt_ref(__r)?)))
                }
                FieldMode::Skipped => unreachable!("handled above"),
            };
        }
    }

    let mut generics = input.generics.clone();
    for param in used_type_params(owned_types.iter(), &input.generics) {
        add_bound(
            &mut generics,
            &param,
            parse_quote!(::nanonbt::DeserializeNbt),
        );
    }
    if let Some(lifetime) = borrow_lifetime {
        for param in used_type_params(nested_types.iter(), &input.generics) {
            add_bound(
                &mut generics,
                &param,
                parse_quote!(::nanonbt::DeserializeNbtRef<#lifetime>),
            );
        }
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    if borrowed {
        let lifetime = borrow_lifetime.expect("borrowed implies a lifetime");
        Ok(quote! {
            #[automatically_derived]
            impl #impl_generics ::nanonbt::DeserializeNbtRef<#lifetime> for #name #ty_generics #where_clause {
                const TAG: ::nanonbt::Tag = #tag;

                fn deserialize_nbt_ref<__R: ::nanonbt::ReadRef<#lifetime> + ?Sized>(
                    __r: &mut __R,
                ) -> ::nanonbt::Result<Self> {
                    #body
                }
            }
        })
    } else {
        Ok(quote! {
            #[automatically_derived]
            impl #impl_generics ::nanonbt::DeserializeNbt for #name #ty_generics #where_clause {
                const TAG: ::nanonbt::Tag = #tag;

                fn deserialize_nbt<__R: ::nanonbt::Read + ?Sized>(
                    __r: &mut __R,
                ) -> ::nanonbt::Result<Self> {
                    #body
                }
            }
        })
    }
}

#[allow(clippy::option_if_let_else)]
fn decode_parts(
    info: &FieldInfo,
    borrow_lifetime: Option<&Lifetime>,
) -> (TokenStream2, TokenStream2) {
    let ty = &info.ty;
    let span = ty.span();
    match info.mode {
        FieldMode::Owned | FieldMode::Skipped => {
            if let Some(array) = &info.attrs.array {
                let route = array.route_type(ty);
                (
                    quote_spanned!(span=> <#route as ::nanonbt::ArrayRoute>::TAG),
                    quote_spanned!(span=> <#ty as ::nanonbt::DeserializeNbtArray<#route>>::deserialize_nbt_array(__r)?),
                )
            } else {
                (
                    quote_spanned!(span=> <#ty as ::nanonbt::DeserializeNbt>::TAG),
                    quote_spanned!(span=> <#ty as ::nanonbt::DeserializeNbt>::deserialize_nbt(__r)?),
                )
            }
        }
        FieldMode::BorrowedArray => {
            let lifetime = borrow_lifetime.expect("borrowed field needs a lifetime");
            let route = info
                .attrs
                .array
                .as_ref()
                .expect("borrowed field needs an array attribute")
                .route_type(ty);
            (
                quote_spanned!(span=> <#route as ::nanonbt::ArrayRoute>::TAG),
                quote_spanned!(span=> <#ty as ::nanonbt::DeserializeNbtArrayRef<#lifetime, #route>>::deserialize_nbt_array_ref(__r)?),
            )
        }
        FieldMode::Nested => {
            let lifetime = borrow_lifetime.expect("nested field needs a lifetime");
            (
                quote_spanned!(span=> <#ty as ::nanonbt::DeserializeNbtRef<#lifetime>>::TAG),
                quote_spanned!(span=> <#ty as ::nanonbt::DeserializeNbtRef<#lifetime>>::deserialize_nbt_ref(__r)?),
            )
        }
    }
}
