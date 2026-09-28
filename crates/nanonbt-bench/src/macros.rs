//! The portable macro backend for the entry files.
//!
//! `benches/compare/macros.rs` expands each entry into an `iai-callgrind`
//! `#[library_benchmark]` that counts instructions under callgrind; this
//! module expands the same invocations into [`Case`](crate::portable::Case)
//! values that a runtime walk can drive — the wasmi fuel runner and the smoke
//! runner — so the four `benches/compare/targets/*.rs` files compile in both
//! trees unchanged apart from which module `library_benchmark_group` comes
//! from.
//!
//! The naming contract is the same on both sides: an entry is named
//! `<kind>_<target>_<id>` by its function and `#[bench]` id, and its report id
//! is `<kind>/<target>/<id>`; see `crate::report`.
//!
//! The `args` of an iai parse entry are the document bytes, which
//! `iai-callgrind` evaluates once, outside the callgrind toggle; here the
//! case keeps the document builder as a function and the harness calls it
//! once, also outside the measurement. A write entry's iai setup runs once
//! per run, outside the toggle; here `Case::write`'s setup runs once per
//! case, also outside the measurement. The iai backend's `$setup` function
//! name stays part of each write macro's pattern — the target files pass it —
//! but the portable backend does not generate it, since nothing here calls it
//! by name.

/// A parse entry: the measured run parses `$input` into its target.
macro_rules! parse_bench {
    ($name:ident, $id:ident, $input:expr, $parse:expr) => {
        /// The entry, as a case for the portable harnesses.
        fn $name() -> $crate::portable::Case {
            fn run(bytes: &[u8]) {
                let _ = ::std::hint::black_box(($parse)(bytes));
            }

            $crate::portable::Case::parse(stringify!($name), stringify!($id), || $input, run)
        }
    };
}

/// A write entry: the setup builds the value once, outside the measurement,
/// and the measured run writes it.
macro_rules! write_bench {
    ($name:ident, $setup:ident, $id:ident, $input:expr, $ty:ty, $parse:expr, $write:expr) => {
        /// The entry, as a case for the portable harnesses.
        fn $name() -> $crate::portable::Case {
            fn setup(bytes: Vec<u8>) -> Box<dyn ::std::any::Any> {
                let value: $ty = ($parse)(&bytes);
                Box::new(value)
            }

            fn write(value: &dyn ::std::any::Any) {
                let value = value
                    .downcast_ref::<$ty>()
                    .expect("the setup built the write value");
                let _ = ::std::hint::black_box(($write)(value));
            }

            $crate::portable::Case::write(
                stringify!($name),
                stringify!($id),
                || $input,
                setup,
                write,
            )
        }
    };
}

/// A write entry whose value borrows from its input: the setup leaks the
/// document bytes, so the value can hold the borrow for the whole case.
macro_rules! write_bench_leaked {
    ($name:ident, $setup:ident, $id:ident, $input:expr, $ty:ty, $parse:expr, $write:expr) => {
        /// The entry, as a case for the portable harnesses.
        fn $name() -> $crate::portable::Case {
            fn setup(bytes: Vec<u8>) -> Box<dyn ::std::any::Any> {
                let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
                let value: $ty = ($parse)(bytes);
                Box::new(value)
            }

            fn write(value: &dyn ::std::any::Any) {
                let value = value
                    .downcast_ref::<$ty>()
                    .expect("the setup built the write value");
                let _ = ::std::hint::black_box(($write)(value));
            }

            $crate::portable::Case::write(
                stringify!($name),
                stringify!($id),
                || $input,
                setup,
                write,
            )
        }
    };
}

/// A group with no cases, for a target whose feature is off.
#[allow(unused_macros)]
macro_rules! empty_group {
    ($name:ident) => {
        pub mod $name {
            /// No cases: the target's feature is off.
            pub fn cases() -> ::std::vec::Vec<$crate::portable::Case> {
                ::std::vec::Vec::new()
            }
        }
    };
}

/// A group of entries: the modules the entry macros generated, gathered into
/// the list the registry concatenates.
macro_rules! library_benchmark_group {
    (name = $group:ident; benchmarks = $($bench:ident),* $(,)?) => {
        pub mod $group {
            /// Every case of the group, in source order.
            pub fn cases() -> ::std::vec::Vec<$crate::portable::Case> {
                ::std::vec![$(super::$bench()),*]
            }
        }
    };
}

pub(crate) use {
    empty_group, library_benchmark_group, parse_bench, write_bench, write_bench_leaked,
};
