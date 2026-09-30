//! The wasm module's exports for the fuel runner.
//!
//! The module exports three functions: `bench_case_count` says how many cases
//! the registry holds, `bench_prepare(i)` builds case `i`'s document and
//! write value into the module's own state, and `bench_run(i)` performs the
//! measured work once. The host resets wasmi's fuel counter between the two
//! calls, so only `bench_run` is counted; see `src/bin/wasm-run.rs`, which
//! also cross-checks the two registries before it trusts an index.
//!
//! The module is built as a `cdylib` for `wasm32-wasip1`; nothing here
//! compiles on the host (`src/lib.rs` gates the module on
//! `target_family = "wasm"`).

use std::any::Any;
use std::cell::RefCell;

use crate::portable::{Case, Run};

thread_local! {
    /// What `bench_prepare` built for `bench_run` to measure.
    static PREPARED: RefCell<Option<Prepared>> = const { RefCell::new(None) };
}

/// The result of one `bench_prepare`.
enum Prepared {
    /// The document a parse case measures.
    Parse(Vec<u8>),
    /// The value a write case measures.
    Write(Box<dyn Any>),
}

/// The number of registered cases.
///
/// The host walks its own registry beside the module's counts, so this is a
/// cross-check, not the source of the ids.
#[unsafe(no_mangle)]
pub extern "C" fn bench_case_count() -> i32 {
    crate::cases().len() as i32
}

/// Builds case `index`'s document and measured value; the host does not count
/// this call.
///
/// Panics when `index` is out of bounds, which the host treats as a trap; the
/// host checks the count first and never asks for one that is not there.
#[unsafe(no_mangle)]
pub extern "C" fn bench_prepare(index: i32) {
    let case = case(index);
    let prepared = match &case.run {
        Run::Parse(_) => Prepared::Parse((case.input)()),
        Run::Write { setup, .. } => Prepared::Write(setup((case.input)())),
    };
    PREPARED.with(|cell| *cell.borrow_mut() = Some(prepared));
}

/// Runs case `index` once; the host counts the fuel this call consumes.
#[unsafe(no_mangle)]
pub extern "C" fn bench_run(index: i32) {
    let case = case(index);
    PREPARED.with(|cell| {
        let mut prepared = cell.borrow_mut();
        match (&case.run, prepared.as_mut()) {
            (Run::Parse(parse), Some(Prepared::Parse(input))) => parse(input),
            (Run::Write { write, .. }, Some(Prepared::Write(value))) => write(&**value),
            _ => panic!("bench_prepare must run before bench_run"),
        }
    });
}

/// The case at `index`.
fn case(index: i32) -> &'static Case {
    crate::cases()
        .get(usize::try_from(index).expect("the host asks for a registered case"))
        .expect("the host asks for a registered case")
}
