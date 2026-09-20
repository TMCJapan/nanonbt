//! The `#[random_names]` attribute: a marker struct becomes one whose field
//! names are drawn from a fixed-seed generator.
//!
//! The bench's name documents need many fields whose names are long and
//! arbitrary, but the names must not sit in the source and every target must
//! read the same bytes. This macro draws them instead: the same arguments give
//! the same names in `nanonbt`, `fastnbt`, `simdnbt` and `pumpkin-nbt`, and
//! `cargo expand` is where they can be read.
//!
//! The generated struct keeps the marker's attributes, so each target's own
//! `#[derive(...)]` list applies to the generated fields. `#[random_names]`
//! must come before `#[derive]`: a derive would otherwise run on the marker.

use std::collections::HashSet;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use syn::{
    Data, DeriveInput, Fields, LitInt, LitStr,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// The alphabet for the random parts of a name.
const ALNUM: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// All Rust keywords, so that a short name can never collide with one.
const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "union", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// The attribute arguments.
struct Args {
    count: usize,
    len: usize,
    prefix: String,
}

impl Parse for Args {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let count = input.parse::<LitInt>()?.base10_parse()?;
        let mut len = None;
        let mut prefix = String::new();
        while !input.is_empty() {
            input.parse::<syn::Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let key = input.parse::<syn::Ident>()?;
            input.parse::<syn::Token![=]>()?;
            match key.to_string().as_str() {
                "len" => len = Some(input.parse::<LitInt>()?.base10_parse()?),
                "prefix" => prefix = input.parse::<LitStr>()?.value(),
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        "unknown argument; expected `len` or `prefix`",
                    ));
                }
            }
        }
        let len = len.ok_or_else(|| syn::Error::new(Span::call_site(), "missing `len`"))?;
        Ok(Self { count, len, prefix })
    }
}

/// A deterministic seed derived from the args.
fn make_seed(args: &Args) -> [u8; 32] {
    let mut base = [0u8; 32];
    // Use a simple deterministic mix of count / len / prefix.
    base[0..8].copy_from_slice(&(args.count as u64).to_le_bytes());
    base[8..16].copy_from_slice(&(args.len as u64).to_le_bytes());
    for (i, b) in args.prefix.bytes().enumerate() {
        base[16 + (i % 16)] ^= b;
    }
    base
}

/// Draws `count` distinct, non-keyword names of length `len` using a
/// fixed-seed `StdRng`.
fn names(args: &Args) -> Result<Vec<String>, String> {
    let seed_bytes = make_seed(args);
    let mut rng = StdRng::from_seed(seed_bytes);
    let mut seen = HashSet::with_capacity(args.count);
    let mut names = Vec::with_capacity(args.count);
    let suffix = args
        .len
        .checked_sub(args.prefix.len())
        .ok_or("len must not be shorter than the prefix")?;
    if suffix == 0 && args.count > 1 {
        return Err("len leaves no room for random bytes".into());
    }
    for _ in 0..args.count * 100 {
        if names.len() == args.count {
            break;
        }
        let mut s = args.prefix.clone();
        for _ in 0..suffix {
            if s.is_empty() {
                s.push(ALNUM[rng.gen_range(0..26)] as char);
            } else {
                s.push(ALNUM[rng.gen_range(0..ALNUM.len())] as char);
            }
        }
        if s.is_empty() {
            s.push(ALNUM[rng.gen_range(0..26)] as char);
        }
        if !KEYWORDS.contains(&s.as_str()) && seen.insert(s.clone()) {
            names.push(s);
        }
    }
    if names.len() < args.count {
        return Err(format!(
            "could only draw {} of {} names",
            names.len(),
            args.count
        ));
    }
    Ok(names)
}

fn expand(args: &Args, input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let ident = &input.ident;
    if !matches!(&input.data, Data::Struct(data) if matches!(data.fields, Fields::Unit)) {
        return Err(syn::Error::new_spanned(
            ident,
            "#[random_names] expects a unit struct",
        ));
    }
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "#[random_names] does not support generics",
        ));
    }
    if args.count == 0 {
        return Err(syn::Error::new_spanned(ident, "count must not be zero"));
    }
    let names = names(args).map_err(|msg| syn::Error::new_spanned(ident, msg))?;
    let fields: Vec<_> = names.iter().map(|n| format_ident!("{n}")).collect();
    let literals: Vec<_> = names
        .iter()
        .map(|n| LitStr::new(n, Span::call_site()))
        .collect();
    let values: Vec<_> = (0..names.len())
        .map(|v| LitInt::new(&format!("{v}i32"), Span::call_site()))
        .collect();
    let filtered_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("random_names"))
        .collect();
    let vis = &input.vis;
    Ok(quote! {
        #(#filtered_attrs)*
        #vis struct #ident {
            #(pub #fields: i32,)*
        }

        #[allow(dead_code)]
        impl #ident {
            /// A value that counts the fields from zero, in declaration order.
            pub fn sample() -> Self {
                Self {
                    #(#fields: #values,)*
                }
            }

            /// Builds the struct by asking `lookup` for every field, in
            /// declaration order.
            pub fn from_lookup(mut lookup: impl FnMut(&'static str) -> i32) -> Self {
                Self {
                    #(#fields: lookup(#literals),)*
                }
            }

            /// Calls `visit` with every field's name and value, in declaration
            /// order.
            pub fn for_each(&self, mut visit: impl FnMut(&'static str, i32)) {
                #(visit(#literals, self.#fields);)*
            }
        }
    })
}

#[proc_macro_attribute]
pub fn random_names(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as Args);
    let input = parse_macro_input!(item as DeriveInput);
    match expand(&args, &input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}
