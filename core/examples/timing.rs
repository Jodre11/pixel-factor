//! Times one enrolment and one verification at production settings.
use pixel_factor_core::geometry::Point;
use pixel_factor_core::{enrol, verify, Policy, StaticPepper};
use std::time::Instant;

fn main() {
    let taps = [
        Point::new(0.10, 0.10),
        Point::new(0.80, 0.20),
        Point::new(0.30, 0.60),
        Point::new(0.55, 0.35),
        Point::new(0.90, 0.70),
    ];
    let pepper = StaticPepper::new(1, b"dev pepper".to_vec());
    let t = Instant::now();
    let e = enrol(&taps, "demo", &Policy::standard(0.75), &pepper).unwrap();
    println!("enrol:  {:?}", t.elapsed());
    let t = Instant::now();
    let ok = verify(&taps, &e.record, &pepper).unwrap();
    println!("verify: {:?} (matched: {ok})", t.elapsed());
}
