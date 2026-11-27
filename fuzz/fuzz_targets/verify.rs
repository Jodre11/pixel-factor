//! Fuzz target: `verify` must never panic, whatever the record and taps.
//!
//! Run with `make fuzz`.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use pixel_factor_core::geometry::Point;
use pixel_factor_core::{verify, KdfParams, Record, StaticPepper, ENC_OFFSETS_LEN};

#[derive(Arbitrary, Debug)]
struct Input {
    version: u8,
    image_id: String,
    r: f64,
    aspect: f64,
    pepper_version: u32,
    salt: [u8; 16],
    first_offset: [u8; 4],
    // Fixed length, so the fuzzer reaches the decryption path, not only the
    // length check that rejects a malformed record.
    enc_offsets: [u8; ENC_OFFSETS_LEN],
    verifier: [u8; 32],
    taps: Vec<(f64, f64)>,
}

fuzz_target!(|input: Input| {
    let record = Record {
        version: input.version,
        image_id: input.image_id,
        r: input.r,
        aspect: input.aspect,
        // Fixed and cheap, so the fuzzer spends its time in our code.
        kdf: KdfParams {
            m_kib: 8,
            t: 1,
            p: 1,
        },
        pepper_version: input.pepper_version,
        salt: input.salt,
        first_offset: input.first_offset,
        enc_offsets: input.enc_offsets.to_vec(),
        verifier: input.verifier,
    };
    let taps: Vec<Point> = input
        .taps
        .into_iter()
        .map(|(x, y)| Point::new(x, y))
        .collect();
    let pepper = StaticPepper::new(1, b"fuzz pepper".to_vec());
    let _ = verify(&taps, &record, &pepper);
});
