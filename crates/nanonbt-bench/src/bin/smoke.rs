//! Runs every registered entry once, natively: the build-and-run check for
//! the platforms that cannot count instructions.
//!
//! Windows and macOS have no valgrind, so their runners get no numbers; what
//! they can do is build the suite and prove that every entry runs there. This
//! binary walks the same registry the `iai-callgrind` bench target and the
//! wasmi fuel runner build from the same entry files, so a target file that
//! no longer parses on the platform, or a document that stopped being valid
//! NBT, fails the job.
//!
//! Every entry prints its report id once it has run, so a panic names the
//! entry that follows the last printed id.

use std::process::ExitCode;

use nanonbt_bench::portable::Run;

fn main() -> ExitCode {
    let cases = nanonbt_bench::cases();
    if cases.is_empty() {
        eprintln!("smoke: the registry is empty; the crate needs at least one target feature");
        return ExitCode::FAILURE;
    }
    for case in cases {
        let input = (case.input)();
        match &case.run {
            Run::Parse(parse) => parse(&input),
            Run::Write { setup, write } => {
                let value = setup(input);
                write(&*value);
            }
        }
        println!("{}", case.id());
    }
    println!("{} entries ran", cases.len());
    ExitCode::SUCCESS
}
