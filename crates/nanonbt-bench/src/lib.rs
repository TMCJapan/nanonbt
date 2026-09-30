//! The portable side of the comparison suite: the entry files compiled
//! against a runtime registry instead of `iai-callgrind`'s macros.
//!
//! The suite counts instructions under callgrind, which exists on Linux
//! x86_64 and aarch64 alone. The same `benches/compare/targets/*.rs` files
//! compile into this library, with `crate::macros` in place of
//! `benches/compare/macros.rs`, and the registry they fill drives the
//! harnesses that cover the other platforms:
//!
//! - `src/bin/wasm-run.rs` counts each entry as wasmi fuel on wasm32, where
//!   neither valgrind nor a native runner exists;
//! - `src/bin/smoke.rs` runs every entry once on Windows and macOS, whose
//!   runners build the suite and check it end to end without counts;
//! - `examples/bench-summary.rs` pairs whatever a platform produced with the
//!   base commit's counts on the same platform, under the report ids
//!   `crate::report` derives from the entry names.
//!
//! `simdnbt` cannot build for wasm32, so the wasm build and its host run with
//! `--no-default-features --features fastnbt,hashify,pumpkin-nbt`; every
//! other harness runs the default features. Both sides of a comparison must
//! use the same features, and the wasm host checks that its registry and the
//! module's agree.
//!
//! The library shares the entry files with the bench target; it is not
//! published and only exists for the harnesses above.

// The entry files name struct fields after their NBT keys, exactly as the
// bench target does; see `benches/compare/main.rs`.
#![allow(non_snake_case)]

pub mod portable;
pub mod report;

mod macros;

#[path = "../benches/compare/documents.rs"]
mod documents;
#[path = "../benches/compare/targets/mod.rs"]
mod targets;

#[cfg(target_family = "wasm")]
mod wasm_exports;

use std::sync::LazyLock;

#[allow(unused_imports)]
use macros::empty_group;
use portable::Case;

#[cfg(feature = "fastnbt")]
use targets::fastnbt::fastnbt_entries;
#[cfg(not(feature = "fastnbt"))]
empty_group!(fastnbt_entries);

#[cfg(feature = "simdnbt")]
use targets::simdnbt::simdnbt_entries;
#[cfg(not(feature = "simdnbt"))]
empty_group!(simdnbt_entries);

#[cfg(feature = "pumpkin-nbt")]
use targets::pumpkin::pumpkin_entries;
#[cfg(not(feature = "pumpkin-nbt"))]
empty_group!(pumpkin_entries);

use targets::nanonbt::nanonbt_entries;

/// Every registered entry, in suite order.
///
/// The order is a function of the source: the registry concatenates the
/// groups in a fixed order and each group lists its entries in source order,
/// so two builds of the crate with the same features hold the same cases in
/// the same order. The wasm host walks its own registry and the module's
/// counts side by side by index, which is why the two must be built with the
/// same features.
pub fn cases() -> &'static [Case] {
    static CASES: LazyLock<Vec<Case>> = LazyLock::new(|| {
        let mut cases = Vec::new();
        cases.extend(nanonbt_entries::cases());
        cases.extend(fastnbt_entries::cases());
        cases.extend(simdnbt_entries::cases());
        cases.extend(pumpkin_entries::cases());
        cases
    });
    &CASES
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::cases;

    #[test]
    fn cases_exist() {
        assert!(!cases().is_empty());
    }

    /// The comparison pairs two runs by report id, so two entries must never
    /// share one, and every entry must be placeable in the report.
    #[test]
    fn ids_are_unique() {
        let mut seen = BTreeSet::new();
        for case in cases() {
            let id = case.id();
            assert!(seen.insert(id.clone()), "duplicate benchmark id `{id}`");
        }
    }
}
