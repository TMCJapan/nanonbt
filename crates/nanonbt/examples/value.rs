//! A dynamic `Value` tree, and the typed values it converts to and from.

use std::collections::BTreeMap;

use nanonbt::{FromNBT, IntArray, ToNBT, Value, from_bytes, from_value, to_bytes, to_value};

#[derive(FromNBT, ToNBT, PartialEq, Debug)]
struct World<'a> {
    name: &'a str,
    level: i32,
}

fn main() -> nanonbt::Result<()> {
    let tree = Value::Compound(BTreeMap::from([
        ("hardcore".into(), Value::Byte(1)),
        ("level".into(), Value::Int(42)),
        ("name".into(), Value::String("overworld".into())),
        (
            "players".into(),
            Value::List(vec![
                Value::String("topi".into()),
                Value::String("steve".into()),
            ]),
        ),
        (
            "uuid".into(),
            Value::IntArray(IntArray::new(vec![1, 2, 3, -4])),
        ),
    ]));

    let bytes = to_bytes(&tree)?;
    println!("wrote {} bytes: {}", bytes.len(), hex(&bytes));

    let back = from_bytes::<Value>(&bytes)?;
    let Value::Compound(entries) = &back else {
        unreachable!("a document root is always a compound")
    };
    for (name, value) in entries {
        println!("{name:>8} = {value:?}");
    }

    // A tree reads into a struct, skipping the entries it does not know,
    // and strings borrow from the tree instead of being copied.
    let world = from_value::<World<'_>>(&back)?;
    println!("borrowed world: {world:?}");

    // A struct converts back into a tree, which is then editable.
    let mut converted = to_value(&world)?;
    if let Value::Compound(entries) = &mut converted {
        entries.insert("hardcore".into(), Value::Byte(0));
    }
    println!("edited tree: {converted:?}");

    // Tags are matched strictly, not converted.
    println!(
        "an Int is not a bool: {}",
        from_value::<bool>(&Value::Int(1)).unwrap_err()
    );

    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes.iter().fold(String::new(), |mut out, byte| {
        write!(out, "{byte:02x}").expect("writing to a String cannot fail");
        out
    })
}
