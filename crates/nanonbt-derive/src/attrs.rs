//! The `#[nbt(...)]` attributes, as written.

use syn::{Attribute, LitStr, Path, Result, meta::ParseNestedMeta};

#[derive(Default)]
pub struct Container {
    pub krate: Option<Path>,
}

#[derive(Default)]
pub struct Field {
    pub ignore: bool,
    pub rename: Option<String>,
}

#[derive(Default)]
pub struct Variant {
    pub rename: Option<String>,
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
