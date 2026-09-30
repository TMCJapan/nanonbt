//! The portable benchmark registry: the runtime counterpart of the
//! `iai-callgrind` macros.
//!
//! The four entry files in `benches/compare/targets/` compile against either
//! macro backend. `benches/compare/macros.rs` expands each entry into an
//! `iai-callgrind` `#[library_benchmark]`, which counts instructions under
//! callgrind; `crate::macros` expands the same invocations into one [`Case`]
//! here. The iai backend runs where valgrind runs — Linux x86_64 and aarch64 —
//! and the registry drives the two harnesses that cover the rest: the wasmi
//! fuel runner on wasm32 and the smoke runner on Windows and macOS. Both
//! backends name every entry the same way, so a report can pair whatever a
//! platform produced with the counts of the others; see `crate::report`.
//!
//! A [`Case`] keeps its document as a function, not as bytes: a harness
//! builds the document once per case, outside the measurement, exactly as the
//! iai backend builds it once per run outside the callgrind toggle.

use std::any::Any;

use crate::report;

/// One benchmark entry, the registry counterpart of an iai
/// `#[library_benchmark]`.
pub struct Case {
    /// The entry's function name, `<kind>_<target>_<id>`.
    pub function_name: &'static str,
    /// The bench id the entry's iai `#[bench]` attribute carries.
    pub bench_id: &'static str,
    /// The entry's document; a harness builds it once, outside the
    /// measurement.
    pub input: fn() -> Vec<u8>,
    /// The measured work.
    pub run: Run,
}

/// How a case turns its document into measured work.
pub enum Run {
    /// Parse the document on every run.
    Parse(fn(&[u8])),
    /// Build the value once, outside the measurement, then write it on every
    /// run.
    Write {
        /// Builds the measured value from the document. A value that borrows
        /// from its input leaks the document to keep the borrow alive.
        setup: fn(Vec<u8>) -> Box<dyn Any>,
        /// Writes the value `setup` built.
        write: fn(&dyn Any),
    },
}

impl Case {
    /// A parse case.
    pub fn parse(
        function_name: &'static str,
        bench_id: &'static str,
        input: fn() -> Vec<u8>,
        parse: fn(&[u8]),
    ) -> Self {
        Self {
            function_name,
            bench_id,
            input,
            run: Run::Parse(parse),
        }
    }

    /// A write case.
    pub fn write(
        function_name: &'static str,
        bench_id: &'static str,
        input: fn() -> Vec<u8>,
        setup: fn(Vec<u8>) -> Box<dyn Any>,
        write: fn(&dyn Any),
    ) -> Self {
        Self {
            function_name,
            bench_id,
            input,
            run: Run::Write { setup, write },
        }
    }

    /// The entry's report id, the name the comparison pairs by.
    ///
    /// Every entry is named `<kind>_<target>_<id>`, which
    /// [`report::display_id`] turns into `<kind>/<target>/<id>`.
    pub fn id(&self) -> String {
        report::display_id(self.function_name, self.bench_id)
            .expect("every entry is named <kind>_<target>_<id>")
    }
}
