//! The `#[nbt(...)]` attributes, as written.

use syn::{Attribute, Error, LitStr, Path, Result, meta::ParseNestedMeta};

#[derive(Default)]
pub struct Container {
    pub krate: Option<Path>,
}

#[derive(Default)]
pub struct Field {
    pub ignore: bool,
    pub rename: Option<String>,
    pub array: Option<ArrayKind>,
}

#[derive(Default)]
pub struct Variant {
    pub rename: Option<String>,
}

/// The NBT array a field with `array = "..."` reads and writes as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayKind {
    /// `array = "byte"`: a `TAG_Byte_Array`.
    Byte,
    /// `array = "int"`: a `TAG_Int_Array`.
    Int,
    /// `array = "long"`: a `TAG_Long_Array`.
    Long,
}

impl ArrayKind {
    /// The array trait that reads and writes this kind.
    pub const fn type_name(self) -> &'static str {
        match self {
            Self::Byte => "ByteArray",
            Self::Int => "IntArray",
            Self::Long => "LongArray",
        }
    }

    /// The kind as `array = "..."` spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Byte => "byte",
            Self::Int => "int",
            Self::Long => "long",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        match name {
            "byte" => Some(Self::Byte),
            "int" => Some(Self::Int),
            "long" => Some(Self::Long),
            _ => None,
        }
    }
}

fn parse(
    attrs: &[Attribute],
    mut each: impl FnMut(ParseNestedMeta<'_>) -> Result<()>,
) -> Result<()> {
    for attr in attrs {
        if !attr.path().is_ident("nbt") {
            continue;
        }
        attr.parse_nested_meta(&mut each)?;
    }
    Ok(())
}

pub fn container(attrs: &[Attribute]) -> Result<Container> {
    let mut out = Container::default();
    parse(attrs, |meta| {
        if meta.path.is_ident("crate") {
            out.krate = Some(meta.value()?.parse::<Path>()?);
            Ok(())
        } else {
            Err(meta.error("unknown nbt container attribute"))
        }
    })?;
    Ok(out)
}

pub fn field(attrs: &[Attribute]) -> Result<Field> {
    let mut out = Field::default();
    parse(attrs, |meta| {
        if meta.path.is_ident("ignore") {
            out.ignore = true;
            Ok(())
        } else if meta.path.is_ident("rename") {
            let name: LitStr = meta.value()?.parse()?;
            out.rename = Some(name.value());
            Ok(())
        } else if meta.path.is_ident("array") {
            let kind: LitStr = meta.value()?.parse()?;
            out.array = Some(ArrayKind::parse(&kind.value()).ok_or_else(|| {
                Error::new_spanned(
                    &kind,
                    "unknown nbt array kind; expected \"byte\", \"int\" or \"long\"",
                )
            })?);
            Ok(())
        } else {
            Err(meta.error("unknown nbt field attribute"))
        }
    })?;
    Ok(out)
}

pub fn variant(attrs: &[Attribute]) -> Result<Variant> {
    let mut out = Variant::default();
    parse(attrs, |meta| {
        if meta.path.is_ident("rename") {
            let name: LitStr = meta.value()?.parse()?;
            out.rename = Some(name.value());
            Ok(())
        } else {
            Err(meta.error("unknown nbt variant attribute"))
        }
    })?;
    Ok(out)
}
