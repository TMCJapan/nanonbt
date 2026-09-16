//! Comparing deserialized values.

/// Equality with floats compared by bits, and without `memcmp`, whose loop
/// would need unwinding as far as the bytes compared.
pub trait Same {
    fn same(&self, other: &Self) -> bool;
}

macro_rules! same_by_eq {
    ($($t:ty)*) => {
        $(impl Same for $t {
            fn same(&self, other: &Self) -> bool {
                self == other
            }
        })*
    };
}

same_by_eq!(i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 bool char);

impl Same for f32 {
    fn same(&self, other: &Self) -> bool {
        self.to_bits() == other.to_bits()
    }
}

impl Same for f64 {
    fn same(&self, other: &Self) -> bool {
        self.to_bits() == other.to_bits()
    }
}

impl<T: Same> Same for Option<T> {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Some(a), Some(b)) => a.same(b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl<T: Same> Same for [T] {
    fn same(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().zip(other).all(|(a, b)| a.same(b))
    }
}

impl<T: Same> Same for Vec<T> {
    fn same(&self, other: &Self) -> bool {
        self[..].same(&other[..])
    }
}

impl<T: Same> Same for &[T] {
    fn same(&self, other: &Self) -> bool {
        (**self).same(*other)
    }
}
