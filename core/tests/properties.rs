//! Property tests: the invariants the scheme rests on, checked against
//! generated inputs instead of hand-picked ones.

use pixel_factor_core::geometry::{Lattice, Offset, Point, MAX_R, MIN_R};
use pixel_factor_core::{enrol_with_salt, verify, KdfParams, Policy, Record, StaticPepper};
use proptest::prelude::*;

const ASPECT: f64 = 0.75;

fn cheap_policy(r: f64) -> Policy {
    Policy {
        r,
        kdf: KdfParams {
            m_kib: 64,
            t: 1,
            p: 1,
        },
        ..Policy::standard(ASPECT)
    }
}

fn pepper() -> StaticPepper {
    StaticPepper::new(1, b"property test pepper".to_vec())
}

fn nudge(p: Point, dist: f64, angle: f64) -> Point {
    Point::new(p.x + dist * angle.cos(), p.y + dist * angle.sin())
}

/// Five points, one per cell of a coarse grid, so they are always far enough
/// apart to enrol. Taps stay clear of the image edge so nudged copies remain
/// inside it.
fn sequence() -> impl Strategy<Value = Vec<Point>> {
    let cells: Vec<(usize, usize)> = (0..4)
        .flat_map(|cx| (0..3).map(move |cy| (cx, cy)))
        .collect();
    (
        Just(cells).prop_shuffle(),
        proptest::collection::vec((0.0..0.06f64, 0.0..0.06f64), 5),
    )
        .prop_map(|(cells, jitter)| {
            cells
                .into_iter()
                .zip(jitter)
                .map(|((cx, cy), (jx, jy))| {
                    Point::new(0.1 + 0.2 * cx as f64 + jx, 0.1 + 0.2 * cy as f64 + jy)
                })
                .collect()
        })
}

fn any_f64() -> impl Strategy<Value = f64> {
    prop_oneof![
        any::<f64>(),
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(f64::NEG_INFINITY),
        -2.0..2.0f64,
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    /// A tap within the inradius of the enrolled point always lands in the
    /// enrolled cell, for every supported tolerance.
    #[test]
    fn taps_inside_the_inradius_always_match(
        r in MIN_R..(MAX_R * 0.999),
        x in 0.0..1.0f64,
        y in 0.0..16.0f64,
        frac in 0.0..0.995f64,
        angle in 0.0..std::f64::consts::TAU,
    ) {
        let lattice = Lattice::new(r).unwrap();
        let p = Point::new(x, y);
        let (index, offset) = lattice.split(p);
        let tap = nudge(p, frac * lattice.inradius(), angle);
        prop_assert_eq!(lattice.locate(tap, offset), index);
    }

    /// A tap beyond the circumradius never lands in the enrolled cell.
    #[test]
    fn taps_beyond_the_circumradius_never_match(
        r in MIN_R..(MAX_R * 0.999),
        x in 0.0..1.0f64,
        y in 0.0..16.0f64,
        factor in 1.005..50.0f64,
        angle in 0.0..std::f64::consts::TAU,
    ) {
        let lattice = Lattice::new(r).unwrap();
        let p = Point::new(x, y);
        let (index, offset) = lattice.split(p);
        let tap = nudge(p, factor * lattice.circumradius(), angle);
        prop_assert_ne!(lattice.locate(tap, offset), index);
    }

    /// The offset says nothing about the cell: moving a point by whole
    /// lattice vectors leaves the offset where it was.
    #[test]
    fn offset_is_independent_of_the_cell(
        r in 0.001..0.2f64,
        x in 0.0..1.0f64,
        y in 0.0..1.0f64,
        dq in -20i32..20,
        dt in -20i32..20,
    ) {
        let lattice = Lattice::new(r).unwrap();
        let s = lattice.circumradius();
        let sqrt3 = 3f64.sqrt();
        let p = Point::new(x, y);
        let moved = Point::new(
            x + sqrt3 * s * f64::from(dq) + sqrt3 * s / 2.0 * f64::from(dt),
            y + 1.5 * s * f64::from(dt),
        );
        let (i1, o1) = lattice.split(p);
        let (i2, o2) = lattice.split(moved);
        // Allow for the offset wrapping across a cell boundary through
        // floating point error.
        let close = |a: u16, b: u16| {
            let d = (i32::from(a) - i32::from(b)).rem_euclid(65536);
            d <= 2 || d >= 65534
        };
        prop_assert!(close(o1.u, o2.u) && close(o1.v, o2.v));
        prop_assert!((i2.q - i1.q - dq).abs() <= 1 && (i2.t - i1.t - dt).abs() <= 1);
    }

    /// Offset bytes round-trip, and every 4-byte value is a valid offset.
    #[test]
    fn offset_bytes_round_trip(bytes in any::<[u8; 4]>()) {
        prop_assert_eq!(Offset::from_bytes(bytes).to_bytes(), bytes);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    /// Any valid sequence verifies when every tap is within tolerance.
    #[test]
    fn nearby_sequences_verify(
        taps in sequence(),
        salt in any::<[u8; 16]>(),
        nudges in proptest::collection::vec((0.0..0.99f64, 0.0..std::f64::consts::TAU), 5),
    ) {
        let policy = cheap_policy(0.022);
        let record = enrol_with_salt(&taps, "prop", &policy, &pepper(), salt).unwrap().record;
        let near: Vec<Point> = taps
            .iter()
            .zip(&nudges)
            .map(|(p, (frac, angle))| nudge(*p, frac * policy.r, *angle))
            .collect();
        prop_assert_eq!(verify(&near, &record, &pepper()), Ok(true));
    }

    /// Moving any single tap outside its hexagon fails the whole sequence.
    #[test]
    fn one_bad_tap_fails_the_sequence(
        taps in sequence(),
        salt in any::<[u8; 16]>(),
        which in 0usize..5,
        factor in 1.01..1.6f64,
        angle in 0.0..std::f64::consts::TAU,
    ) {
        let policy = cheap_policy(0.022);
        let record = enrol_with_salt(&taps, "prop", &policy, &pepper(), salt).unwrap().record;
        let s = Lattice::new(policy.r).unwrap().circumradius();
        let mut wrong = taps.clone();
        wrong[which] = nudge(wrong[which], factor * s, angle);
        prop_assert_eq!(verify(&wrong, &record, &pepper()), Ok(false));
    }

    /// Any reordering of the taps fails.
    #[test]
    fn reordered_sequences_fail(
        taps in sequence(),
        salt in any::<[u8; 16]>(),
        a in 0usize..5,
        b in 0usize..5,
    ) {
        prop_assume!(a != b);
        let record = enrol_with_salt(&taps, "prop", &cheap_policy(0.022), &pepper(), salt)
            .unwrap()
            .record;
        let mut swapped = taps.clone();
        swapped.swap(a, b);
        prop_assert_eq!(verify(&swapped, &record, &pepper()), Ok(false));
    }

    /// Whatever taps arrive, a well-formed record gives yes or no, never an
    /// error and never a panic. This is the no-per-tap-oracle rule.
    #[test]
    fn arbitrary_taps_never_error(
        taps in sequence(),
        salt in any::<[u8; 16]>(),
        attempt in proptest::collection::vec((any_f64(), any_f64()), 0..8),
    ) {
        let record = enrol_with_salt(&taps, "prop", &cheap_policy(0.022), &pepper(), salt)
            .unwrap()
            .record;
        let attempt: Vec<Point> = attempt.into_iter().map(|(x, y)| Point::new(x, y)).collect();
        prop_assert!(verify(&attempt, &record, &pepper()).is_ok());
    }

    /// Whatever the stored offsets are, verification gives yes or no.
    #[test]
    fn arbitrary_offsets_never_error(
        taps in sequence(),
        salt in any::<[u8; 16]>(),
        first in any::<[u8; 4]>(),
        rest in any::<[u8; 16]>(),
    ) {
        let mut record = enrol_with_salt(&taps, "prop", &cheap_policy(0.022), &pepper(), salt)
            .unwrap()
            .record;
        record.first_offset = first;
        record.enc_offsets = rest.to_vec();
        prop_assert!(verify(&taps, &record, &pepper()).is_ok());
    }

    /// A record with arbitrary field values never causes a panic.
    #[test]
    fn arbitrary_records_never_panic(
        version in any::<u8>(),
        image_id in ".{0,300}",
        r in any_f64(),
        aspect in any_f64(),
        salt in any::<[u8; 16]>(),
        first_offset in any::<[u8; 4]>(),
        enc_offsets in proptest::collection::vec(any::<u8>(), 0..200),
        verifier in any::<[u8; 32]>(),
        attempt in proptest::collection::vec((any_f64(), any_f64()), 0..8),
    ) {
        let record = Record {
            version,
            image_id,
            r,
            aspect,
            kdf: KdfParams { m_kib: 64, t: 1, p: 1 },
            pepper_version: 1,
            salt,
            first_offset,
            enc_offsets,
            verifier,
        };
        let attempt: Vec<Point> = attempt.into_iter().map(|(x, y)| Point::new(x, y)).collect();
        let _ = verify(&attempt, &record, &pepper());
    }
}
