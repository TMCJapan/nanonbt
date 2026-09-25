//! Seeded random inputs, checked against the `cesu8` crate.

use nanocesu8::{Cesu8, Cesu8Buf};
use testkit::{check_n, ensure, ensure_eq, generate};

#[test]
fn modified_utf8_matches_the_cesu8_crate() {
    // 4096, not the default: the encoder has no proof at all, and the
    // decoder's covers one byte, so this suite carries the crate.
    check_n("modified_utf8_matches_the_cesu8_crate", 4096, |rng| {
        let text = generate::string(rng, 12);
        let encoded = Cesu8::from_str(&text);
        ensure_eq!(
            encoded.as_bytes(),
            &*cesu8::to_java_cesu8(&text),
            "encode {text:?}"
        );

        let bytes = generate::mutate(rng, encoded.as_bytes());
        if let Ok(valid) = Cesu8::new(&bytes) {
            let theirs = cesu8::from_java_cesu8(&bytes).expect("both accept");
            ensure_eq!(&*valid.decode(), &*theirs, "decode {bytes:02x?}");
            ensure_eq!(
                &*Cesu8::decode_bytes(&bytes).expect("both accept"),
                &*theirs,
                "decode_bytes {bytes:02x?}"
            );
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
        } else {
            ensure!(
                Cesu8::decode_bytes(&bytes).is_err(),
                "decode_bytes accepts what new refuses: {bytes:02x?}"
            );
            if cesu8::from_java_cesu8(&bytes).is_ok() {
                // The `cesu8` crate is strictly more permissive: it takes
                // plain UTF-8 with a raw NUL or four-byte lead.
                let plain = std::str::from_utf8(&bytes)
                    .map_err(|_| format!("accepted only when UTF-8: {bytes:02x?}"))?;
                ensure!(
                    plain.bytes().any(|b| b == 0 || b >= 0xf0),
                    "UTF-8 without a NUL or four-byte lead is modified UTF-8: {bytes:02x?}"
                );
            }
        }
        Ok(())
    });
}
