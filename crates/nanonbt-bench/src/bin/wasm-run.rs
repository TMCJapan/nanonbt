//! Counts every entry on wasm32 as wasmi fuel, for the wasm section of the
//! benchmark comment.
//!
//! The wasm build of this crate exports `bench_case_count`,
//! `bench_prepare(i)` and `bench_run(i)`; this host loads the module with
//! wasmi, resets the fuel counter between the two calls, and writes one
//! `{ id, count }` per entry to a JSON file the summarizer reads. Every case
//! runs in a fresh store and instance, so nothing a case allocates outlives
//! it and no case's count depends on the cases before it; wasmi counts wasm
//! instructions, so a count is exact and repeats on every run.
//!
//! The module is built for `wasm32-wasip1` — `pumpkin-nbt`'s `uuid` needs a
//! source of randomness, which `wasm32-unknown-unknown` cannot offer without
//! a JavaScript host — so the module imports the WASI functions every
//! `wasip1` `std` build imports. The measured entries never call them, but
//! wasmi resolves every import at instantiation, so [`stub_linker`] answers
//! them all with zeros.
//!
//! Usage: `wasm-run <MODULE.wasm> <OUTPUT.json>`. Build the module and this
//! host with the same features — both walk the same registry — and leave
//! `simdnbt` off, which does not build for wasm32:
//!
//! ```text
//! cargo build --locked --release --target wasm32-wasip1 --lib \
//!     --no-default-features --features fastnbt,hashify,pumpkin-nbt
//! cargo run --quiet --locked --release --no-default-features \
//!     --features fastnbt,hashify,pumpkin-nbt --bin wasm-run -- \
//!     target/wasm32-wasip1/release/nanonbt_bench.wasm wasm.json
//! ```

#![cfg(not(target_family = "wasm"))]

use std::env;
use std::error::Error;
use std::fs;
use std::process::ExitCode;

use serde::Serialize;
use wasmi::{Config, Engine, ExternType, Instance, Linker, Module, Store, Val};

/// The fuel every case starts from: far past the largest entry, so the count
/// is never capped, and far below [`u64::MAX`], so the remaining fuel a
/// `bench_run` can leave behind never underflows.
const FUEL: u64 = 1 << 62;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("wasm-run: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let (Some(module_path), Some(output_path)) = (args.next(), args.next()) else {
        return Err("usage: wasm-run <MODULE.wasm> <OUTPUT.json>".into());
    };

    let module_bytes = fs::read(&module_path)?;
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, &module_bytes[..])?;

    let cases = nanonbt_bench::cases();
    if cases.is_empty() {
        return Err("the registry is empty; the crate needs at least one target feature".into());
    }

    // The module and this host both walk the registry the entry files
    // describe; a build of either with different features would report
    // another build's counts under this build's ids.
    let linker = stub_linker(&engine, &module)?;
    let declared = {
        let (mut store, instance) = instantiate(&engine, &module, &linker)?;
        let count = instance.get_typed_func::<(), i32>(&store, "bench_case_count")?;
        count.call(&mut store, ())?
    };
    if declared as usize != cases.len() {
        return Err(format!(
            "the module holds {declared} cases but the host {}; build both with the same features",
            cases.len()
        )
        .into());
    }

    let mut entries = Vec::with_capacity(cases.len());
    for (index, case) in cases.iter().enumerate() {
        let (mut store, instance) = instantiate(&engine, &module, &linker)?;
        let prepare = instance.get_typed_func::<i32, ()>(&store, "bench_prepare")?;
        let run = instance.get_typed_func::<i32, ()>(&store, "bench_run")?;

        prepare.call(&mut store, index as i32)?;
        store.set_fuel(FUEL)?;
        run.call(&mut store, index as i32)?;
        let count = FUEL - store.get_fuel()?;

        entries.push(Entry {
            id: case.id(),
            count,
        });
    }

    let report = Report { entries };
    let mut json = serde_json::to_string_pretty(&report)?;
    json.push('\n');
    fs::write(&output_path, json)?;
    Ok(())
}

/// Instantiates the module into a fresh store, with the fuel already set so a
/// module `start` function can run.
///
/// The caller resets the fuel after `bench_prepare`, so the instantiation and
/// the setup never count toward the measurement.
fn instantiate(
    engine: &Engine,
    module: &Module,
    linker: &Linker<()>,
) -> Result<(Store<()>, Instance), Box<dyn Error>> {
    let mut store = Store::new(engine, ());
    store.set_fuel(FUEL)?;
    let instance = linker.instantiate_and_start(&mut store, module)?;
    Ok((store, instance))
}

/// A linker that satisfies the module's WASI imports with stubs.
///
/// The measured entries parse and write in memory and never call WASI, but a
/// `wasm32-wasip1` `std` build imports the functions anyway — `fd_write` for
/// a panic, `random_get` for `pumpkin-nbt`'s `uuid` — and wasmi resolves
/// every import at instantiation. Each stub answers with zeros: the only way
/// a counted run could reach one is by panicking, and a panic fails the job
/// either way.
///
/// A non-function import would not be satisfied here and fails instantiation
/// with wasmi's own message.
fn stub_linker(engine: &Engine, module: &Module) -> Result<Linker<()>, Box<dyn Error>> {
    let mut linker = Linker::new(engine);
    for import in module.imports() {
        let ExternType::Func(ty) = import.ty() else {
            continue;
        };
        linker.func_new(
            import.module(),
            import.name(),
            ty.clone(),
            |_caller, _params, results| {
                for result in results.iter_mut() {
                    *result = Val::default(result.ty());
                }
                Ok(())
            },
        )?;
    }
    Ok(linker)
}

/// The JSON the summarizer reads: one fuel count per entry.
#[derive(Serialize)]
struct Report {
    entries: Vec<Entry>,
}

#[derive(Serialize)]
struct Entry {
    /// The entry's report id.
    id: String,
    /// The wasm instructions its measured run consumed, as wasmi fuel.
    count: u64,
}
