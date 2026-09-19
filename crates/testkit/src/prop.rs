use std::panic::{self, AssertUnwindSafe};

use crate::rng::{Pcg32, mix64};

/// Default seed base; fixed so CI runs are reproducible.
const DEFAULT_BASE: u64 = 0x7274_5f73_796e_6321;

/// Outcome of one property case. `Err` carries a human-readable reason.
pub type CaseResult = Result<(), String>;

fn env_u64(name: &str) -> Option<u64> {
    let raw = std::env::var(name).ok()?;
    let raw = raw.trim();
    let parsed = raw
        .strip_prefix("0x")
        .map_or_else(|| raw.parse(), |hex| u64::from_str_radix(hex, 16));
    match parsed {
        Ok(v) => Some(v),
        Err(e) => panic!("{name}={raw:?} is not a u64: {e}"),
    }
}

/// Runs `f` for `default_cases` seeds (or `RT_CASES`), or just `RT_SEED`.
///
/// Panics with the failing seed on the first `Err` or panic inside `f`.
pub fn check_n(name: &str, default_cases: u64, mut f: impl FnMut(&mut Pcg32) -> CaseResult) {
    if let Some(seed) = env_u64("RT_SEED") {
        run_case(name, seed, &mut f);
        return;
    }
    let cases = env_u64("RT_CASES").unwrap_or(default_cases);
    let base = env_u64("RT_SEED_BASE").unwrap_or(DEFAULT_BASE);
    for i in 0..cases {
        run_case(name, mix64(base ^ mix64(i)), &mut f);
    }
}

/// [`check_n`] with a default of 256 cases.
pub fn check(name: &str, f: impl FnMut(&mut Pcg32) -> CaseResult) {
    check_n(name, 256, f);
}

fn run_case(name: &str, seed: u64, f: &mut impl FnMut(&mut Pcg32) -> CaseResult) {
    let mut rng = Pcg32::seed_from_u64(seed);
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| f(&mut rng)));
    let reason = match outcome {
        Ok(Ok(())) => return,
        Ok(Err(reason)) => reason,
        Err(payload) => payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "non-string panic payload".to_owned()),
    };
    panic!("property `{name}` failed: {reason}\n  reproduce with: RT_SEED=0x{seed:016x}");
}

/// `ensure!(cond, "fmt", args..)` returns `Err(String)` from a property case.
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($fmt:tt)+) => {
        if !$cond {
            return Err(format!($($fmt)+));
        }
    };
}

/// `ensure_eq!(a, b, "context")` returns `Err` with both values on mismatch.
#[macro_export]
macro_rules! ensure_eq {
    ($left:expr, $right:expr $(, $($fmt:tt)+)?) => {
        match (&$left, &$right) {
            (l, r) => {
                if *l != *r {
                    let ctx: String = String::new() $( + &format!($($fmt)+) )?;
                    return Err(format!("{ctx}\n  left:  {l:?}\n  right: {r:?}"));
                }
            }
        }
    };
}
