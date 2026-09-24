//! The generated implementations.
//!
//! Names, both `rename`d and default, are encoded as Java's modified UTF-8
//! while the macro runs: writing borrows the static bytes and reading
//! compares the raw bytes, so a name with a NUL or a non-BMP character needs
//! no conversion and allocates nothing at run time.

use nanocesu8::Cesu8;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    GenericParam, Generics, Lifetime, LifetimeParam, LitByteStr, Path, WherePredicate, parse_quote,
};

use crate::model::{Array, ArrayContainer, Field, Model, Shape, Variant, option_inner};

pub fn to_nbt(model: &Model) -> TokenStream {
    match &model.shape {
        Shape::Struct(fields) => struct_to_nbt(model, fields),
        Shape::Newtype(ty) => newtype_to_nbt(model, ty),
        Shape::Enum(variants) => enum_to_nbt(model, variants),
    }
}

pub fn from_nbt(model: &Model) -> TokenStream {
    match &model.shape {
        Shape::Struct(fields) => struct_from_nbt(model, fields),
        Shape::Newtype(ty) => newtype_from_nbt(model, ty),
        Shape::Enum(variants) => enum_from_nbt(model, variants),
    }
}

/// Adds `'de` to the generics, or reuses one already named that.
fn add_de(generics: &mut Generics) -> Lifetime {
    if let Some(lifetime) = generics
        .lifetimes()
        .map(|param| param.lifetime.clone())
        .find(|lifetime| lifetime.ident == "de")
    {
        return lifetime;
    }
    let de = Lifetime::new("'de", Span::call_site());
    generics
        .params
        .insert(0, GenericParam::Lifetime(LifetimeParam::new(de.clone())));
    de
}

/// The type a field's own implementation is required of: `Option<T>` fields
/// use `T`, since `Option` is handled by the generated code.
fn bounded_type(field: &Field) -> &syn::Type {
    option_inner(&field.ty).unwrap_or(&field.ty)
}

/// The array trait that writes and reads an `array` field, with its element.
fn array_trait(krate: &Path, array: &Array) -> TokenStream {
    let name = format_ident!("{}", array.kind.type_name());
    let element = &array.element;
    quote!(#krate::#name<#element>)
}

/// `name` as the exact modified UTF-8 bytes NBT stores.
///
/// The encoding happens while the macro runs, so the generated code never
/// has to turn a `str` into modified UTF-8. [`encoded_name`] borrows the
/// bytes, which are valid by construction.
fn name_bytes(name: &str) -> LitByteStr {
    LitByteStr::new(Cesu8::from_str(name).as_bytes(), Span::call_site())
}

/// The borrowed, already validated `Cesu8` a generated write uses.
fn encoded_name(krate: &Path, name: &str) -> TokenStream {
    let bytes = name_bytes(name);
    // SAFETY: `name_bytes` encodes with `Cesu8::from_str`, so the bytes are
    // modified UTF-8.
    quote!(unsafe { #krate::Cesu8::from_bytes_unchecked(#bytes) })
}

fn struct_to_nbt(model: &Model, fields: &[Field]) -> TokenStream {
    let krate = &model.krate;
    let ident = &model.ident;
    let mut generics = model.generics.clone();
    {
        let where_clause = generics.make_where_clause();
        for field in fields.iter().filter(|field| !field.ignore) {
            if let Some(array) = &field.array {
                let array_trait = array_trait(krate, array);
                let element = &array.element;
                where_clause
                    .predicates
                    .push(parse_quote!([#element]: #array_trait));
            } else {
                let ty = bounded_type(field);
                where_clause
                    .predicates
                    .push(parse_quote!(#ty: #krate::ToNBT));
            }
        }
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let writes = fields.iter().filter(|field| !field.ignore).map(|field| {
        let field_ident = &field.ident;
        let name = encoded_name(krate, &field.name);
        let option = option_inner(&field.ty).is_some();
        let write = match &field.array {
            Some(array) => {
                let array_trait = array_trait(krate, array);
                let element = &array.element;
                let slice = if option {
                    quote!(&value[..])
                } else {
                    quote!(&self.#field_ident[..])
                };
                quote! {
                    <[#element] as #array_trait>::write_entry(#slice, #name, writer)?;
                }
            }
            None if option => quote! {
                #krate::ToNBT::write_entry(value, #name, writer)?;
            },
            None => quote! {
                #krate::ToNBT::write_entry(&self.#field_ident, #name, writer)?;
            },
        };
        if option {
            quote! {
                if let ::core::option::Option::Some(value) = &self.#field_ident {
                    #write
                }
            }
        } else {
            write
        }
    });

    quote! {
        impl #impl_generics #krate::ToNBT for #ident #ty_generics #where_clause {
            const TAG: u8 = #krate::TAG_COMPOUND;

            fn write<W: #krate::Write>(&self, writer: &mut W) -> #krate::Result<()> {
                #(#writes)*
                #krate::Write::write_end(writer)?;
                ::core::result::Result::Ok(())
            }
        }
    }
}

fn newtype_to_nbt(model: &Model, ty: &syn::Type) -> TokenStream {
    let krate = &model.krate;
    let ident = &model.ident;
    let mut generics = model.generics.clone();
    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#ty: #krate::ToNBT));
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics #krate::ToNBT for #ident #ty_generics #where_clause {
            const TAG: u8 = <#ty as #krate::ToNBT>::TAG;

            fn write<W: #krate::Write>(&self, writer: &mut W) -> #krate::Result<()> {
                #krate::ToNBT::write(&self.0, writer)
            }
        }
    }
}

fn enum_to_nbt(model: &Model, variants: &[Variant]) -> TokenStream {
    let krate = &model.krate;
    let ident = &model.ident;
    let (impl_generics, ty_generics, where_clause) = model.generics.split_for_impl();
    let arms = variants.iter().map(|variant| {
        let variant_ident = &variant.ident;
        let name = encoded_name(krate, &variant.name);
        quote! {
            Self::#variant_ident => #krate::Write::write_cesu8(writer, #name),
        }
    });

    quote! {
        impl #impl_generics #krate::ToNBT for #ident #ty_generics #where_clause {
            const TAG: u8 = #krate::TAG_STRING;

            fn write<W: #krate::Write>(&self, writer: &mut W) -> #krate::Result<()> {
                match self {
                    #(#arms)*
                }
            }
        }
    }
}

fn struct_from_nbt(model: &Model, fields: &[Field]) -> TokenStream {
    let krate = &model.krate;
    let ident = &model.ident;
    let mut generics = model.generics.clone();
    let de = add_de(&mut generics);
    {
        let where_clause = generics.make_where_clause();
        for field in fields {
            let predicate: WherePredicate = if field.ignore {
                let ty = &field.ty;
                parse_quote!(#ty: ::core::default::Default)
            } else if let Some(array) = &field.array {
                let array_trait = array_trait(krate, array);
                let element = &array.element;
                match array.container {
                    ArrayContainer::Borrowed => {
                        let ty = bounded_type(field);
                        parse_quote!(#ty: #krate::FromNBT<#de>)
                    }
                    ArrayContainer::Vec | ArrayContainer::Fixed => {
                        parse_quote!([#element]: #array_trait)
                    }
                }
            } else {
                let ty = bounded_type(field);
                parse_quote!(#ty: #krate::FromNBT<#de>)
            };
            where_clause.predicates.push(predicate);
        }
    }
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let (_, ty_generics, _) = model.generics.split_for_impl();

    let accumulators = fields.iter().filter(|field| !field.ignore).map(|field| {
        let acc = format_ident!("__nbt_{}", field.ident);
        let ty = bounded_type(field);
        quote! {
            let mut #acc: ::core::option::Option<#ty> = ::core::option::Option::None;
        }
    });
    let arms = fields.iter().filter(|field| !field.ignore).map(|field| {
        let acc = format_ident!("__nbt_{}", field.ident);
        let name = name_bytes(&field.name);
        let read = field.array.as_ref().map_or_else(
            || {
                let ty = bounded_type(field);
                quote!(<#ty as #krate::FromNBT<#de>>::read(tag, reader)?)
            },
            |array| {
                let array_trait = array_trait(krate, array);
                let element = &array.element;
                let read = quote!(<[#element] as #array_trait>::read(tag, reader)?);
                match array.container {
                    // A borrow keeps the input, so it reads through its own
                    // implementation, byte arrays having one and wider
                    // integers not, since their bytes are big-endian.
                    ArrayContainer::Borrowed => {
                        let ty = bounded_type(field);
                        quote!(<#ty as #krate::FromNBT<#de>>::read(tag, reader)?)
                    }
                    ArrayContainer::Vec => read,
                    ArrayContainer::Fixed => quote!(
                        ::core::convert::TryInto::try_into(#read)
                            .map_err(|_| #krate::Error::wrong_len())?
                    ),
                }
            },
        );
        quote! {
            #name => {
                #acc = ::core::option::Option::Some(#read);
            }
        }
    });
    let assigned = fields.iter().map(|field| {
        let field_ident = &field.ident;
        if field.ignore {
            quote! {
                #field_ident: ::core::default::Default::default(),
            }
        } else {
            let acc = format_ident!("__nbt_{}", field.ident);
            if option_inner(&field.ty).is_some() {
                quote! {
                    #field_ident: #acc,
                }
            } else {
                let name = &field.name;
                quote! {
                    #field_ident: #acc
                        .ok_or_else(|| #krate::Error::missing_field(#name))?,
                }
            }
        }
    });

    quote! {
        impl #impl_generics #krate::FromNBT<#de> for #ident #ty_generics #where_clause {
            fn read<R: #krate::Read<#de>>(
                tag: u8,
                reader: &mut R,
            ) -> #krate::Result<Self> {
                if tag != #krate::TAG_COMPOUND {
                    return ::core::result::Result::Err(#krate::Error::invalid_tag(tag));
                }
                #(#accumulators)*
                #krate::Read::nest(reader, |reader| {
                    loop {
                        let tag = #krate::Read::read_tag(reader)?;
                        if tag == #krate::TAG_END {
                            break;
                        }
                        let name = #krate::Read::read_name(reader)?;
                        match name.as_bytes() {
                            #(#arms)*
                            _ => #krate::Read::skip(reader, tag)?,
                        }
                    }
                    ::core::result::Result::Ok(())
                })?;
                ::core::result::Result::Ok(Self {
                    #(#assigned)*
                })
            }
        }
    }
}

fn newtype_from_nbt(model: &Model, ty: &syn::Type) -> TokenStream {
    let krate = &model.krate;
    let ident = &model.ident;
    let mut generics = model.generics.clone();
    let de = add_de(&mut generics);
    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#ty: #krate::FromNBT<#de>));
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let (_, ty_generics, _) = model.generics.split_for_impl();

    quote! {
        impl #impl_generics #krate::FromNBT<#de> for #ident #ty_generics #where_clause {
            fn read<R: #krate::Read<#de>>(
                tag: u8,
                reader: &mut R,
            ) -> #krate::Result<Self> {
                ::core::result::Result::Ok(Self(
                    <#ty as #krate::FromNBT<#de>>::read(tag, reader)?,
                ))
            }
        }
    }
}

fn enum_from_nbt(model: &Model, variants: &[Variant]) -> TokenStream {
    let krate = &model.krate;
    let ident = &model.ident;
    let mut generics = model.generics.clone();
    let de = add_de(&mut generics);
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let (_, ty_generics, _) = model.generics.split_for_impl();
    let arms = variants.iter().map(|variant| {
        let variant_ident = &variant.ident;
        let name = name_bytes(&variant.name);
        quote! {
            #name => ::core::result::Result::Ok(Self::#variant_ident),
        }
    });

    quote! {
        impl #impl_generics #krate::FromNBT<#de> for #ident #ty_generics #where_clause {
            fn read<R: #krate::Read<#de>>(
                tag: u8,
                reader: &mut R,
            ) -> #krate::Result<Self> {
                if tag != #krate::TAG_STRING {
                    return ::core::result::Result::Err(#krate::Error::invalid_tag(tag));
                }
                let name = #krate::Read::read_cesu8(reader)?;
                match name.as_bytes() {
                    #(#arms)*
                    _ => ::core::result::Result::Err(#krate::Error::unknown_variant()),
                }
            }
        }
    }
}
