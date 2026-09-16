//! Seeded random inputs, checked against fastnbt on every public seam.
//!
//! These reach what the Kani proofs cannot: error paths on malformed input,
//! arbitrary string contents, and compounds with many entries.

mod common;

use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};

use common::{from_fast, to_fast};
use nanonbt::Value;
use rt_testkit::{Pcg32, check_n, ensure, ensure_eq, generate};
use serde::{Deserialize, Serialize};

/// fastnbt's result, or `None` where it panics.
fn fastnbt_outcome<T>(f: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(f)).ok()
}

const TAGS: [u8; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

#[allow(clippy::cast_possible_truncation)] // any bits will do
fn scalar(rng: &mut Pcg32, tag: u8, depth: u32) -> Value {
    let bits = generate::u64_edgy(rng);
    match tag {
        1 => Value::Byte(bits as i8),
        2 => Value::Short(bits as i16),
        3 => Value::Int(bits as i32),
        4 => Value::Long(bits.cast_signed()),
        5 => Value::Float(f32::from_bits(bits as u32)),
        6 => Value::Double(f64::from_bits(bits)),
        7 => Value::ByteArray(nanonbt::ByteArray::new(
            generate::bytes(rng, 12)
                .into_iter()
                .map(u8::cast_signed)
                .collect(),
        )),
        8 => Value::String(generate::string(rng, 8)),
        9 => list(rng, depth),
        10 => compound(rng, depth, 4),
        11 => Value::IntArray(nanonbt::IntArray::new(
            (0..generate::len(rng, 5))
                .map(|_| rng.next_u32().cast_signed())
                .collect(),
        )),
        _ => Value::LongArray(nanonbt::LongArray::new(
            (0..generate::len(rng, 3))
                .map(|_| generate::i64_edgy(rng))
                .collect(),
        )),
    }
}

fn any_tag(rng: &mut Pcg32, depth: u32) -> u8 {
    let tags = if depth == 0 { &TAGS[..8] } else { &TAGS[..] };
    *rng.choose(tags).unwrap_or(&1)
}

/// Usually one element type, as NBT requires; sometimes not.
fn list(rng: &mut Pcg32, depth: u32) -> Value {
    let tag = any_tag(rng, depth.saturating_sub(1));
    let mixed = rng.ratio(1, 10);
    Value::List(
        (0..generate::len(rng, 4))
            .map(|_| {
                let tag = if mixed { any_tag(rng, 0) } else { tag };
                scalar(rng, tag, depth.saturating_sub(1))
            })
            .collect(),
    )
}

fn key(rng: &mut Pcg32) -> String {
    match rng.below(20) {
        0 => "__fastnbt_byte_array".to_owned(),
        1 => ["", "a", "Level", "\0", "🦀"][rng.index(5)].to_owned(),
        _ => generate::string(rng, 6),
    }
}

fn compound(rng: &mut Pcg32, depth: u32, max_entries: usize) -> Value {
    Value::Compound(
        (0..generate::len(rng, max_entries))
            .map(|_| {
                let tag = any_tag(rng, depth.saturating_sub(1));
                (key(rng), scalar(rng, tag, depth.saturating_sub(1)))
            })
            .collect(),
    )
}

/// A document that is valid, or a mutation of one.
///
/// A root whose keys include an array token beside others is one fastnbt
/// refuses, and mutating nothing is no test at all, so keep drawing.
fn document(rng: &mut Pcg32) -> Vec<u8> {
    let mut valid = Vec::new();
    for _ in 0..8 {
        if let Ok(bytes) = fastnbt::to_bytes(&to_fast(compound(rng, 3, 5))) {
            valid = bytes;
            break;
        }
    }
    if rng.ratio(1, 3) {
        valid
    } else {
        generate::mutate(rng, &valid)
    }
}

fn debug<T: std::fmt::Debug>(value: &T) -> String {
    format!("{value:?}")
}

/// [`debug`] of a tree, but floats by their bits.
///
/// `Debug` prints every NaN as `NaN`, and the generator makes NaNs of
/// arbitrary payload on purpose, so `Debug` alone cannot tell them apart.
fn show(value: &Value) -> String {
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
        other => debug(other),
    }
}

/// [`show`] of a tree that may be missing.
fn show_opt(value: Option<&Value>) -> String {
    value.map_or_else(|| String::from("None"), show)
}

#[derive(Deserialize, PartialEq, Debug)]
#[allow(dead_code)] // compared through Debug
struct Typed {
    #[serde(rename = "a")]
    byte: Option<i8>,
    #[serde(rename = "Level", default)]
    level: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    list: Vec<i32>,
    flag: Option<bool>,
    text: Option<String>,
    bytes: Option<nanonbt::ByteArray>,
    ignored: Option<()>,
}

#[derive(Deserialize, Debug)]
struct Empty {}

#[test]
fn documents_deserialize_like_fastnbt() {
    check_n("documents_deserialize_like_fastnbt", 4096, |rng| {
        let bytes = document(rng);

        let fast = fastnbt_outcome(|| fastnbt::from_bytes::<fastnbt::Value>(&bytes).ok());
        let nano = nanonbt::from_bytes::<Value>(&bytes).ok();
        match fast {
            Some(fast) => ensure_eq!(
                show_opt(nano.as_ref()),
                show_opt(fast.map(from_fast).as_ref()),
                "Value {bytes:02x?}"
            ),
            None => ensure!(nano.is_none(), "fastnbt panicked on {bytes:02x?}"),
        }

        let as_tree = |m: BTreeMap<String, Value>| Value::Compound(m);
        let fast = fastnbt_outcome(|| {
            fastnbt::from_bytes::<BTreeMap<String, fastnbt::Value>>(&bytes).ok()
        });
        let nano = nanonbt::from_bytes::<BTreeMap<String, Value>>(&bytes).ok();
        match fast {
            Some(fast) => ensure_eq!(
                show_opt(nano.map(as_tree).as_ref()),
                show_opt(
                    fast.map(|m| as_tree(m.into_iter().map(|(k, v)| (k, from_fast(v))).collect()))
                        .as_ref()
                ),
                "map {bytes:02x?}"
            ),
            None => ensure!(nano.is_none(), "fastnbt panicked on {bytes:02x?}"),
        }

        let fast = fastnbt_outcome(|| fastnbt::from_bytes::<Empty>(&bytes).is_ok());
        let nano = nanonbt::from_bytes::<Empty>(&bytes).is_ok();
        ensure_eq!(Some(nano), fast.or(Some(false)), "skipping {bytes:02x?}");
        Ok(())
    });
}

#[test]
fn typed_documents_deserialize_like_fastnbt() {
    #[derive(Serialize)]
    struct Source {
        a: i8,
        #[serde(rename = "Level")]
        level: BTreeMap<String, i32>,
        list: Vec<i32>,
        flag: i64,
        text: String,
        bytes: fastnbt::ByteArray,
        ignored: fastnbt::Value,
    }

    check_n("typed_documents_deserialize_like_fastnbt", 2048, |rng| {
        let ignored = to_fast(compound(rng, 2, 3));
        let source = Source {
            a: 1,
            level: BTreeMap::from([(generate::string(rng, 3), rng.next_u32().cast_signed())]),
            list: (0..generate::len(rng, 3))
                .map(|_| rng.next_u32().cast_signed())
                .collect(),
            flag: generate::i64_edgy(rng),
            text: generate::string(rng, 5),
            bytes: fastnbt::ByteArray::new(
                generate::bytes(rng, 4)
                    .into_iter()
                    .map(u8::cast_signed)
                    .collect(),
            ),
            ignored,
        };
        let valid = fastnbt::to_bytes(&source).unwrap_or_default();
        let bytes = if rng.bool() {
            valid
        } else {
            generate::mutate(rng, &valid)
        };

        let fast = fastnbt_outcome(|| fastnbt::from_bytes::<Typed>(&bytes).ok());
        let nano = nanonbt::from_bytes::<Typed>(&bytes).ok();
        match fast {
            Some(fast) => ensure_eq!(debug(&nano), debug(&fast), "{bytes:02x?}"),
            None => ensure!(nano.is_none(), "fastnbt panicked on {bytes:02x?}"),
        }
        Ok(())
    });
}

/// Keeps at most one entry per compound, so that each has a single order.
fn prune(value: Value) -> Value {
    match value {
        Value::Compound(map) => Value::Compound(
            map.into_iter()
                .take(1)
                .map(|(k, v)| (k, prune(v)))
                .collect(),
        ),
        Value::List(list) => Value::List(list.into_iter().map(prune).collect()),
        other => other,
    }
}

/// Whether every list holds one element type, as NBT requires.
///
/// A list that does not serializes, deliberately, to bytes no reader can
/// resynchronize, so what comes back depends on where the entry landed.
fn single_typed_lists(value: &Value) -> bool {
    let kind = std::mem::discriminant::<Value>;
    match value {
        Value::List(list) => {
            list.windows(2).all(|pair| kind(&pair[0]) == kind(&pair[1]))
                && list.iter().all(single_typed_lists)
        }
        Value::Compound(map) => map.values().all(single_typed_lists),
        _ => true,
    }
}

/// Whether fastnbt's answer depends on its hash order: it tells an array
/// wrapper from a compound by whichever key its map yields first.
fn order_sensitive(value: &Value) -> bool {
    match value {
        Value::Compound(map) => {
            (map.len() > 1 && map.keys().any(|k| k.starts_with("__fastnbt_")))
                || map.values().any(order_sensitive)
        }
        Value::List(list) => list.iter().any(order_sensitive),
        _ => false,
    }
}

#[test]
fn trees_serialize_like_fastnbt() {
    check_n("trees_serialize_like_fastnbt", 4096, |rng| {
        let tree = if rng.ratio(1, 8) {
            let tag = any_tag(rng, 2);
            prune(scalar(rng, tag, 2))
        } else {
            prune(compound(rng, 3, 2))
        };
        let fast = fastnbt_outcome(|| fastnbt::to_bytes(&to_fast(tree.clone())).ok());
        let nano = nanonbt::to_bytes(&tree).ok();
        match fast {
            Some(fast) => ensure_eq!(nano, fast, "{tree:?}"),
            None => ensure!(nano.is_none(), "fastnbt panicked on {tree:?}"),
        }

        let opts = (rng.bool(), generate::string(rng, 3));
        let fast = fastnbt::SerOpts::new().serialize_root_compound_name(opts.0);
        let nano = nanonbt::SerOpts::new().serialize_root_compound_name(opts.0);
        let (fast, nano) = if rng.bool() {
            (fast.root_name(opts.1.clone()), nano.root_name(opts.1))
        } else {
            (fast, nano)
        };
        ensure_eq!(
            nanonbt::to_bytes_with_opts(&tree, nano).ok(),
            fastnbt::to_bytes_with_opts(&to_fast(tree.clone()), fast).ok(),
            "opts {tree:?}"
        );
        Ok(())
    });
}

/// What multi-entry compounds serialize to, which bytes cannot be compared.
///
/// `prune` keeps the byte-for-byte suite above to one entry per compound,
/// because the two crates order entries differently. Reading each crate's
/// bytes back through fastnbt drops the ordering and leaves the rest:
/// headers, lengths, End tags and where each entry lands.
#[test]
fn multi_entry_trees_serialize_to_the_same_document() {
    check_n(
        "multi_entry_trees_serialize_to_the_same_document",
        4096,
        |rng| {
            let tree = compound(rng, 3, 4);
            // Which key fastnbt sees first decides whether a compound is an
            // array at all, so those trees have no single answer to compare,
            // and a mixed list has no readable answer at all.
            if order_sensitive(&tree) || !single_typed_lists(&tree) {
                return Ok(());
            }
            let nano = nanonbt::to_bytes(&tree).ok();
            let fast = fastnbt_outcome(|| fastnbt::to_bytes(&to_fast(tree.clone())).ok()).flatten();
            let read = |bytes: &Option<Vec<u8>>| {
                bytes.as_ref().map(|bytes| {
                    show_opt(
                        fastnbt::from_bytes::<fastnbt::Value>(bytes)
                            .map(from_fast)
                            .ok()
                            .as_ref(),
                    )
                })
            };
            ensure_eq!(read(&nano), read(&fast), "round trip {tree:?}");
            Ok(())
        },
    );
}

#[test]
fn trees_convert_like_fastnbt() {
    check_n("trees_convert_like_fastnbt", 4096, |rng| {
        let tag = any_tag(rng, 3);
        let tree = scalar(rng, tag, 3);
        let fast_tree = to_fast(tree.clone());

        let fast = fastnbt_outcome(|| fastnbt::to_value(&fast_tree).ok());
        let nano = nanonbt::to_value(&tree).ok();
        match fast {
            Some(fast) => ensure_eq!(
                show_opt(nano.as_ref()),
                show_opt(fast.map(from_fast).as_ref()),
                "to_value {tree:?}"
            ),
            None => ensure!(nano.is_none(), "fastnbt panicked on {tree:?}"),
        }

        if order_sensitive(&tree) {
            return Ok(());
        }
        let fast = fastnbt_outcome(|| fastnbt::from_value::<fastnbt::Value>(&fast_tree).ok());
        let nano = nanonbt::from_value::<Value>(&tree).ok();
        match fast {
            Some(fast) => ensure_eq!(
                show_opt(nano.as_ref()),
                show_opt(fast.map(from_fast).as_ref()),
                "from_value {tree:?}"
            ),
            None => ensure!(nano.is_none(), "fastnbt panicked on {tree:?}"),
        }

        let fast = fastnbt_outcome(|| fastnbt::from_value::<Typed>(&fast_tree).ok());
        let nano = nanonbt::from_value::<Typed>(&tree).ok();
        match fast {
            Some(fast) => ensure_eq!(debug(&nano), debug(&fast), "typed from_value {tree:?}"),
            None => ensure!(nano.is_none(), "fastnbt panicked on {tree:?}"),
        }
        Ok(())
    });
}

#[test]
fn modified_utf8_matches_the_cesu8_crate() {
    // 4096, not the default: the encoder has no proof at all, and the
    // decoder's covers one byte, so this suite carries the module.
    check_n("modified_utf8_matches_the_cesu8_crate", 4096, |rng| {
        let text = generate::string(rng, 12);
        let encoded = nanonbt::cesu8::to_java_cesu8(&text);
        ensure_eq!(&*encoded, &*cesu8::to_java_cesu8(&text), "encode {text:?}");

        let bytes = generate::mutate(rng, &encoded);
        ensure_eq!(
            nanonbt::cesu8::from_java_cesu8(&bytes).ok(),
            cesu8::from_java_cesu8(&bytes).ok(),
            "decode {bytes:02x?}"
        );
        Ok(())
    });
}
