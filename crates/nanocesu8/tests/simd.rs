//! Long inputs and chunk boundaries, where the `simd` fast paths take over.
//!
//! Under the `simd` feature, UTF-8 validation is delegated to `simdutf8`
//! and the encode scan runs 32 bytes at a time. These tests walk interesting
//! byte windows and long inputs across the sizes where those fast paths take
//! over, and hold the results against `core::str::from_utf8`, the `cesu8`
//! crate, and hand-derived oracles. They run in both configurations: the
//! scalar path must agree just the same.

use std::borrow::Cow;

use nanocesu8::Cesu8;
use rt_testkit::{check_n, ensure, ensure_eq, generate};

/// Long enough for the head, one 32-byte block, and a tail.
const BUF: usize = 100;

#[test]
fn encode_borrows_around_every_boundary() {
    for len in 0..=2 * BUF {
        let plain = "a".repeat(len);
        let encoded = nanocesu8::to_java_cesu8(&plain);
        assert!(matches!(encoded, Cow::Borrowed(_)), "{len} ASCII bytes");
        assert_eq!(encoded.as_ptr(), plain.as_ptr(), "{len} ASCII bytes");
        for pos in 0..len {
            let mut bytes = vec![b'a'; len];
            bytes[pos] = 0;
            let text = String::from_utf8(bytes).expect("NUL is UTF-8");
            let encoded = nanocesu8::to_java_cesu8(&text);
            assert!(
                matches!(encoded, Cow::Owned(_)),
                "NUL at {pos} of {len} must copy"
            );
            assert_eq!(
                &*encoded,
                &*cesu8::to_java_cesu8(&text),
                "NUL at {pos} of {len}"
            );

            let text = format!("{}\u{10401}{}", "a".repeat(pos), "a".repeat(len - pos));
            let encoded = nanocesu8::to_java_cesu8(&text);
            assert!(
                matches!(encoded, Cow::Owned(_)),
                "astral at {pos} of {len} must copy"
            );
            assert_eq!(
                &*encoded,
                &*cesu8::to_java_cesu8(&text),
                "astral at {pos} of {len}"
            );
        }
    }
}

#[test]
fn two_byte_windows_agree_with_the_oracles() {
    // Across the head, the block and the tail, and the seams between them.
    const POSITIONS: [usize; 11] = [0, 1, 62, 63, 64, 65, 66, 94, 95, 96, 97];
    let mut buf = vec![b'a'; BUF];
    for first in 0..=255u8 {
        for second in 0..=255u8 {
            for pos in POSITIONS {
                buf[pos] = first;
                buf[pos + 1] = second;
                // Inside a run of ASCII, the only window that is modified
                // UTF-8 without being UTF-8 is `C0 80`: two-byte sequences
                // `C2..=DF` are UTF-8 as well, and three-byte sequences and
                // surrogate pairs do not fit the window.
                if let Ok(text) = str::from_utf8(&buf) {
                    let ours = nanocesu8::from_java_cesu8(&buf).expect("UTF-8 decodes");
                    assert!(matches!(ours, Cow::Borrowed(_)), "{buf:02x?} must borrow");
                    assert_eq!(&*ours, text, "{buf:02x?}");
                    Cesu8::new(&buf).expect("UTF-8 validates");
                } else {
                    let accepted = (first, second) == (0xc0, 0x80);
                    assert_eq!(
                        nanocesu8::from_java_cesu8(&buf).is_ok(),
                        accepted,
                        "{buf:02x?}"
                    );
                    assert_eq!(Cesu8::new(&buf).is_ok(), accepted, "{buf:02x?}");
                    if accepted {
                        let text = nanocesu8::from_java_cesu8(&buf).expect("C0 80 decodes");
                        assert!(matches!(text, Cow::Owned(_)), "{buf:02x?}");
                        assert_eq!(
                            &*text,
                            format!("{}\0{}", "a".repeat(pos), "a".repeat(BUF - 2 - pos)),
                            "{buf:02x?}"
                        );
                    }
                }
                buf[pos] = b'a';
                buf[pos + 1] = b'a';
            }
        }
    }
}

#[test]
fn edge_sequences_agree_with_the_cesu8_crate() {
    // Every byte that starts, ends or breaks a range in the grammar.
    const EDGES: &[u8] = &[
        0x00, 0x41, 0x7f, 0x80, 0x8f, 0x90, 0x9f, 0xa0, 0xbf, 0xc0, 0xc1, 0xc2, 0xdf, 0xe0, 0xed,
        0xef, 0xf0, 0xf4, 0xf5, 0xff,
    ];
    let mut buf = vec![b'a'; BUF];
    for &first in EDGES {
        for &second in EDGES {
            for &third in EDGES {
                for pos in [63, 64, 96] {
                    buf[pos..pos + 3].copy_from_slice(&[first, second, third]);
                    agree_with_both_oracles(&buf);
                    buf[pos..pos + 3].fill(b'a');
                }
                for &fourth in EDGES {
                    for pos in [63, 93] {
                        buf[pos..pos + 4].copy_from_slice(&[first, second, third, fourth]);
                        agree_with_both_oracles(&buf);
                        buf[pos..pos + 4].fill(b'a');
                    }
                }
            }
        }
    }
}

#[test]
fn long_inputs_match_the_cesu8_crate() {
    check_n("long_inputs_match_the_cesu8_crate", 4096, |rng| {
        let target = 64 + rng.index(193);
        let mut text = String::new();
        while text.len() < target {
            text.push_str(&generate::string(rng, 16));
            text.push('a');
        }
        let encoded = nanocesu8::to_java_cesu8(&text);
        ensure_eq!(&*encoded, &*cesu8::to_java_cesu8(&text), "encode");

        let mutated = generate::mutate(rng, &encoded);
        ensure_oracles_agree(&mutated)?;

        let mut raw = vec![0u8; 64 + rng.index(193)];
        rng.fill_bytes(&mut raw);
        ensure_oracles_agree(&raw)?;
        Ok(())
    });
}

/// `from_java_cesu8` must borrow exactly the UTF-8 bytes, and otherwise
/// agree with the `cesu8` crate about acceptance and text.
fn agree_with_both_oracles(buf: &[u8]) {
    let theirs = cesu8::from_java_cesu8(buf);
    let accepted = theirs.is_ok();
    if let Ok(text) = str::from_utf8(buf) {
        let ours = nanocesu8::from_java_cesu8(buf)
            .unwrap_or_else(|_| panic!("valid UTF-8 must decode: {buf:02x?}"));
        assert!(matches!(ours, Cow::Borrowed(_)), "{buf:02x?} must borrow");
        assert_eq!(&*ours, text, "{buf:02x?}");
    } else {
        assert_eq!(
            nanocesu8::from_java_cesu8(buf).ok(),
            theirs.ok(),
            "{buf:02x?}"
        );
    }
    assert_eq!(Cesu8::new(buf).is_ok(), accepted, "{buf:02x?}");
}

/// [`agree_with_both_oracles`] as a property case, plus the iteration and
/// decoding of an accepted [`Cesu8`].
fn ensure_oracles_agree(bytes: &[u8]) -> Result<(), String> {
    if let Ok(text) = str::from_utf8(bytes) {
        let ours = nanocesu8::from_java_cesu8(bytes).ok();
        ensure!(matches!(ours, Some(Cow::Borrowed(_))), "{bytes:02x?}");
        ensure_eq!(ours.as_deref(), Some(text), "borrow {bytes:02x?}");
    }
    ensure_eq!(
        nanocesu8::from_java_cesu8(bytes).ok(),
        cesu8::from_java_cesu8(bytes).ok(),
        "decode {bytes:02x?}"
    );
    match Cesu8::new(bytes) {
        Ok(valid) => {
            let theirs = cesu8::from_java_cesu8(bytes).expect("both accept");
            ensure_eq!(&*valid.decode(), &*theirs, "decode {bytes:02x?}");
            ensure_eq!(
                valid.chars().collect::<String>().as_str(),
                &*theirs,
                "chars {bytes:02x?}"
            );
        }
        Err(_) => ensure!(
            cesu8::from_java_cesu8(bytes).is_err(),
            "both reject {bytes:02x?}"
        ),
    }
    Ok(())
}
