//! The validated description of a type to generate code for.

use syn::{Data, DeriveInput, Error, Fields, Generics, Ident, Path, Result, Type};

use crate::attrs;

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
    pub ignore: bool,
}

pub struct Variant {
    pub ident: Ident,
    pub name: String,
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
                            let name = attrs.rename.unwrap_or_else(|| unraw(&ident));
                            Ok(Field {
                                ident,
                                ty: field.ty.clone(),
                                name,
                                ignore: attrs.ignore,
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
                        Ok(Variant {
                            ident: variant.ident.clone(),
                            name: attrs.rename.unwrap_or_else(|| variant.ident.to_string()),
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
