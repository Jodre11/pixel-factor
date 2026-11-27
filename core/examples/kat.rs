//! Prints the known-answer record used by the test suite.
use pixel_factor_core::geometry::Point;
use pixel_factor_core::{enrol_with_salt, KdfParams, Policy, StaticPepper};

fn main() {
    let taps = [
        Point::new(0.10, 0.10),
        Point::new(0.80, 0.20),
        Point::new(0.30, 0.60),
        Point::new(0.55, 0.35),
        Point::new(0.90, 0.70),
    ];
    let policy = Policy {
        kdf: KdfParams {
            m_kib: 256,
            t: 1,
            p: 1,
        },
        ..Policy::standard(0.75)
    };
    let pepper = StaticPepper::new(1, b"test pepper, never use in production".to_vec());
    let e = enrol_with_salt(&taps, "kat-image", &policy, &pepper, *b"0123456789abcdef").unwrap();
    println!("{}", serde_json::to_string(&e.record).unwrap());
}
