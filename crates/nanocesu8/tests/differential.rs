//! Seeded random inputs, checked against the `cesu8` crate.

use nanocesu8::{Cesu8, Cesu8Buf};
use testkit::{check_n, ensure, ensure_eq, generate};

#[test]
fn modified_utf8_matches_the_cesu8_crate() {
    // 4096, not the default: the encoder has no proof at all, and the
    // decoder's covers one byte, so this suite carries the crate.
    check_n("modified_utf8_matches_the_cesu8_crate", 4096, |rng| {
        let text = generate::string(rng, 12);
        let encoded = nanocesu8::to_java_cesu8(&text);
        ensure_eq!(&*encoded, &*cesu8::to_java_cesu8(&text), "encode {text:?}");

        let bytes = generate::mutate(rng, &encoded);
        ensure_eq!(
            nanocesu8::from_java_cesu8(&bytes).ok(),
            cesu8::from_java_cesu8(&bytes).ok(),
            "decode {bytes:02x?}"
        );

        match Cesu8::new(&bytes) {
            Ok(valid) => {
                let theirs = cesu8::from_java_cesu8(&bytes).expect("both accept");
                ensure_eq!(&*valid.decode(), &*theirs, "decode {bytes:02x?}");
                ensure_eq!(
                    valid.chars().collect::<String>().as_str(),
                    &*theirs,
                    "chars {bytes:02x?}"
                );
                ensure_eq!(
                    Cesu8Buf::from_bytes(&bytes)
                        .expect("both accept")
                        .as_cesu8(),
                    valid,
                    "buffer {bytes:02x?}"
                );
            }
            Err(_) => ensure!(
                cesu8::from_java_cesu8(&bytes).is_err(),
                "both reject {bytes:02x?}"
            ),
        }
        Ok(())
    });
}
