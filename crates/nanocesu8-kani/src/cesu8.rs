//! `nanocesu8::from_java_cesu8` against the `cesu8` crate.
//!
//! Only single bytes: two symbolic bytes through either decoder exhaust 5 GB.

use nanocesu8::from_java_cesu8;

proofs! {
    /// Both decoders accept the same single bytes, as the same text, and
    /// nanocesu8's does not panic on any.
    fn decode_1() unwind 4 {
        let bytes: [u8; 1] = kani::any();
        let ours = from_java_cesu8(&bytes);
        let theirs = ::cesu8::from_java_cesu8(&bytes);
        let same = match (&ours, &theirs) {
            (Ok(ours), Ok(theirs)) => **ours == **theirs,
            (Err(_), Err(_)) => true,
            _ => false,
        };
        assert!(same, "decoders disagree");
    }
}
