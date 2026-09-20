//! One module per target: the structs it parses into, and how it writes them.

pub mod nanonbt;

#[cfg(feature = "fastnbt")]
pub mod fastnbt;
#[cfg(feature = "pumpkin-nbt")]
pub mod pumpkin;
#[cfg(feature = "simdnbt")]
pub mod simdnbt;
