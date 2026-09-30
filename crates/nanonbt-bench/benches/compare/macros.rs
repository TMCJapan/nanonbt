//! The macros that turn each benchmark into one `#[library_benchmark]`.
//!
//! `iai-callgrind` counts the instructions of one `#[library_benchmark]`
//! function per run and files the count under the function's name and its
//! `#[bench::id]`. The report id is recovered from those two names, so every
//! entry is named `<kind>_<target>_<id>` — `parse_nanonbt_serde_small` is
//! `parse/nanonbt-serde/small` — and the id after the last underscore is the
//! one its `#[bench]` attribute carries; `examples/bench-summary.rs` depends
//! on both halves.
//!
//! The `args` of a parse entry are the document bytes; `iai-callgrind`
//! evaluates them once, outside the callgrind toggle, so building the
//! document is never measured. A write entry needs the parsed value, which a
//! setup function produces once per run, also outside the toggle. Callgrind
//! collects only the wrapper module around the benchmark function itself, so
//! neither side is part of the count.
//!
//! Setup returns its value by value, so a struct that borrows from its input
//! cannot be built by the shared `write_bench!`: the value would outlive the
//! local `Vec<u8>`. [`write_bench_leaked!`] leaks the document bytes to give
//! the borrow a `'static` life instead; the leak is one allocation per run and
//! falls outside the measurement like any other setup.
//!
//! `empty_group!` stands in for a target's group when its feature is off, so
//! `main!` always names the same four groups; see `compare/main.rs`.
//!
//! `library_benchmark_group` is re-exported from `iai-callgrind` so the four
//! `targets/*.rs` files can import every macro from one place: in this tree
//! the group macro is the iai one, and in the portable library
//! (`src/macros.rs`) it gathers the same entry list into the runtime registry.

/// A parse benchmark: `$parse` decodes `$input` into its target on every run.
macro_rules! parse_bench {
    ($name:ident, $id:ident, $input:expr, $parse:expr) => {
        #[::iai_callgrind::library_benchmark]
        #[bench::$id(args = ($input))]
        fn $name(bytes: Vec<u8>) {
            let _ = ::std::hint::black_box(($parse)(&bytes));
        }
    };
}

/// A write benchmark: `$parse` sets the value up once, and `$write` encodes it
/// on every run.
macro_rules! write_bench {
    ($name:ident, $setup:ident, $id:ident, $input:expr, $ty:ty, $parse:expr, $write:expr) => {
        fn $setup(bytes: Vec<u8>) -> $ty {
            ($parse)(&bytes)
        }

        #[::iai_callgrind::library_benchmark]
        #[bench::$id(args = ($input), setup = $setup)]
        fn $name(value: $ty) {
            let _ = ::std::hint::black_box(($write)(&value));
        }
    };
}

/// A write benchmark for a value that borrows from its input: the setup leaks
/// the document bytes, so the value can hold the borrow for the whole run.
///
/// `$ty` is the borrowed type with a real lifetime — `Borrowed<'a>`, say — and
/// `$static` the same type with that lifetime fixed to `'static`.
macro_rules! write_bench_leaked {
    ($name:ident, $setup:ident, $id:ident, $input:expr, $ty:ty, $parse:expr, $write:expr) => {
        fn $setup(bytes: Vec<u8>) -> $ty {
            let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
            ($parse)(bytes)
        }

        #[::iai_callgrind::library_benchmark]
        #[bench::$id(args = ($input), setup = $setup)]
        fn $name(value: $ty) {
            let _ = ::std::hint::black_box(($write)(&value));
        }
    };
}

/// A group with no benchmarks, for a target whose feature is off.
///
/// The body mirrors what `iai-callgrind`'s `library_benchmark_group!` expands
/// to, empty; the private field and function signatures are the interface
/// `main!` calls.
#[allow(unused_macros)]
macro_rules! empty_group {
    ($name:ident) => {
        pub mod $name {
            pub const __BENCHES: &[&(
                &'static str,
                fn() -> Option<::iai_callgrind::__internal::InternalLibraryBenchmarkConfig>,
                &[::iai_callgrind::__internal::InternalMacroLibBench],
            )] = &[];

            pub fn __get_config()
            -> Option<::iai_callgrind::__internal::InternalLibraryBenchmarkConfig> {
                None
            }

            pub fn __compare_by_id() -> Option<bool> {
                None
            }

            pub fn __run_setup(_run: bool) -> bool {
                false
            }

            pub fn __run_teardown(_run: bool) -> bool {
                false
            }

            pub fn __run(_group_index: usize, _bench_index: usize) {}
        }
    };
}

pub use iai_callgrind::library_benchmark_group;

pub(crate) use {empty_group, parse_bench, write_bench, write_bench_leaked};
