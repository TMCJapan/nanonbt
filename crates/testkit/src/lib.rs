//! Test support: a seeded property harness and value generators.
//!
//! Failures print the exact seed; rerun a single case with `RT_SEED=<seed>`.
//! `RT_CASES=<n>` overrides the number of cases, `RT_SEED_BASE=<n>` shifts the
//! whole seed sequence to explore new inputs.

pub mod generate;
pub mod prop;
mod rng;

pub use prop::{check, check_n};
pub use rng::Pcg32;
