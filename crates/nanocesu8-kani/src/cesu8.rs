//! `Cesu8::new` and `Cesu8::decode` against the `cesu8` crate.
//!
//! Only single bytes: two symbolic bytes through either decoder exhaust 5 GB.

use nanocesu8::Cesu8;

proofs! {
    /// A single byte is accepted as the same text by both, except that only
    /// the `cesu8` crate takes a raw NUL; neither panics on any.
    fn decode_1() unwind 4 {
        let bytes: [u8; 1] = kani::any();
        let ours = Cesu8::new(&bytes).map(Cesu8::decode);
        let theirs = ::cesu8::from_java_cesu8(&bytes);
        let same = match (&ours, &theirs) {
            (Ok(ours), Ok(theirs)) => **ours == **theirs,
            (Err(_), Ok(_)) => bytes == [0],
            (Err(_), Err(_)) => true,
            (Ok(_), Err(_)) => false,
        };
        assert!(same, "decoders disagree");
    }
}
