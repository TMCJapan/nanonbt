//! Helpers shared by the differential test suites.

use nanonbt::Value;

/// [`Debug`] of a tree, but floats by their bits.
///
/// `Debug` prints every NaN as `NaN`, and the suites make NaNs of arbitrary
/// payload on purpose, so `Debug` alone cannot tell them apart.
pub fn show(value: &Value) -> String {
    match value {
        Value::Float(v) => format!("Float({:#010x})", v.to_bits()),
        Value::Double(v) => format!("Double({:#018x})", v.to_bits()),
        Value::List(list) => {
            let shown: Vec<String> = list.iter().map(show).collect();
            format!("List([{}])", shown.join(", "))
        }
        Value::Compound(map) => {
            let shown: Vec<String> = map
                .iter()
                .map(|(key, value)| format!("{key:?}: {}", show(value)))
                .collect();
            format!("Compound({{{}}})", shown.join(", "))
        }
        other => format!("{other:?}"),
    }
}

/// The same tree, with the compound maps swapped.
pub fn from_fast(value: fastnbt::Value) -> Value {
    match value {
        fastnbt::Value::Byte(v) => Value::Byte(v),
        fastnbt::Value::Short(v) => Value::Short(v),
        fastnbt::Value::Int(v) => Value::Int(v),
        fastnbt::Value::Long(v) => Value::Long(v),
        fastnbt::Value::Float(v) => Value::Float(v),
        fastnbt::Value::Double(v) => Value::Double(v),
        fastnbt::Value::String(v) => Value::String(v),
        fastnbt::Value::ByteArray(v) => Value::ByteArray(nanonbt::ByteArray::new(v.into_inner())),
        fastnbt::Value::IntArray(v) => Value::IntArray(nanonbt::IntArray::new(v.into_inner())),
        fastnbt::Value::LongArray(v) => Value::LongArray(nanonbt::LongArray::new(v.into_inner())),
        fastnbt::Value::List(v) => Value::List(v.into_iter().map(from_fast).collect()),
        fastnbt::Value::Compound(v) => {
            Value::Compound(v.into_iter().map(|(k, v)| (k, from_fast(v))).collect())
        }
    }
}

pub fn to_fast(value: Value) -> fastnbt::Value {
    match value {
        Value::Byte(v) => fastnbt::Value::Byte(v),
        Value::Short(v) => fastnbt::Value::Short(v),
        Value::Int(v) => fastnbt::Value::Int(v),
        Value::Long(v) => fastnbt::Value::Long(v),
        Value::Float(v) => fastnbt::Value::Float(v),
        Value::Double(v) => fastnbt::Value::Double(v),
        Value::String(v) => fastnbt::Value::String(v),
        Value::ByteArray(v) => fastnbt::Value::ByteArray(fastnbt::ByteArray::new(v.into_inner())),
        Value::IntArray(v) => fastnbt::Value::IntArray(fastnbt::IntArray::new(v.into_inner())),
        Value::LongArray(v) => fastnbt::Value::LongArray(fastnbt::LongArray::new(v.into_inner())),
        Value::List(v) => fastnbt::Value::List(v.into_iter().map(to_fast).collect()),
        Value::Compound(v) => {
            fastnbt::Value::Compound(v.into_iter().map(|(k, v)| (k, to_fast(v))).collect())
        }
    }
}
