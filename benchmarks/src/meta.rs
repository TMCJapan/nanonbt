//! Scenario metadata shared by the benchmarks and the report generator.

use crate::{fixtures, nanonbt_models};

/// A benchmark scenario: one logical NBT document type.
#[derive(Clone, Copy, Debug)]
pub struct Scenario {
    /// The criterion group name; also the report section id.
    pub group: &'static str,
    /// Human-readable title.
    pub title: &'static str,
    /// One-line description of what is measured.
    pub description: &'static str,
}

/// All scenarios, in report order.
pub const SCENARIOS: &[Scenario] = &[
    Scenario {
        group: "small",
        title: "Small struct",
        description: "Eight scalar fields: two ints, float, double, short, long, string, byte and bool.",
    },
    Scenario {
        group: "byte_array",
        title: "Byte array (1 MiB)",
        description: "TAG_Byte_Array of 1 MiB, owned and borrowed.",
    },
    Scenario {
        group: "int_array",
        title: "Int array (64 Ki elements)",
        description: "TAG_Int_Array of 65,536 i32 values, owned and borrowed (big-endian wire view).",
    },
    Scenario {
        group: "long_array",
        title: "Long array (64 Ki elements)",
        description: "TAG_Long_Array of 65,536 i64 values, owned and borrowed (big-endian wire view).",
    },
    Scenario {
        group: "strings",
        title: "Strings",
        description: "One 32 KiB string plus a 2,048 element list of 64-byte strings.",
    },
    Scenario {
        group: "compounds",
        title: "List of compounds",
        description: "1,024 compound tags, each with scalars, a name and a four string list.",
    },
    Scenario {
        group: "many_fields",
        title: "Many fields",
        description: "48 scalar fields plus a string; stresses field-name dispatch (nanonbt hashify).",
    },
    Scenario {
        group: "deep",
        title: "Deep nesting",
        description: "Four levels of nested compounds with scalars and strings.",
    },
    Scenario {
        group: "chunk",
        title: "Chunk section",
        description: "Minecraft-flavored section: long/byte arrays, light arrays and a compound palette.",
    },
];

/// Returns the encoded byte length of a scenario's document.
///
/// The size is measured by encoding the fixture with `nanonbt`; the interop
/// tests assert that `fastnbt` and `simdnbt` produce the exact same bytes, so
/// this length is shared by every library.
///
/// # Panics
///
/// Panics when `group` is not one of [`SCENARIOS`].
pub fn encoded_len(group: &str) -> usize {
    match group {
        "small" => {
            nanonbt_models::small_write(&nanonbt_models::small_from_fixture(&fixtures::small()))
                .len()
        }
        "byte_array" => nanonbt_models::byte_array_owned(&nanonbt_models::byte_array_from_fixture(
            &fixtures::byte_array(),
        ))
        .len(),
        "int_array" => nanonbt_models::int_array_owned(&nanonbt_models::int_array_from_fixture(
            &fixtures::int_array(),
        ))
        .len(),
        "long_array" => nanonbt_models::long_array_owned(&nanonbt_models::long_array_from_fixture(
            &fixtures::long_array(),
        ))
        .len(),
        "strings" => nanonbt_models::strings_write(&nanonbt_models::strings_from_fixture(
            &fixtures::strings(),
        ))
        .len(),
        "compounds" => nanonbt_models::compounds_write(&nanonbt_models::compounds_from_fixture(
            &fixtures::compounds(),
        ))
        .len(),
        "many_fields" => nanonbt_models::many_fields_write(
            &nanonbt_models::many_fields_from_fixture(&fixtures::many_fields()),
        )
        .len(),
        "deep" => {
            nanonbt_models::deep_write(&nanonbt_models::deep_from_fixture(&fixtures::deep())).len()
        }
        "chunk" => {
            nanonbt_models::chunk_write(&nanonbt_models::chunk_from_fixture(&fixtures::chunk()))
                .len()
        }
        other => panic!("unknown scenario: {other}"),
    }
}
