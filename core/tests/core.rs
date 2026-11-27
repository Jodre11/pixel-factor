use pixel_factor_core::geometry::{hex_round, Lattice, Point};
use pixel_factor_core::{
    enrol, enrol_with_salt, verify, Error, KdfParams, Policy, Record, StaticPepper, Warning,
};

const ASPECT: f64 = 0.75;

/// Cheap Argon2id settings so the suite runs quickly. Not for production.
fn test_policy() -> Policy {
    Policy {
        kdf: KdfParams {
            m_kib: 256,
            t: 1,
            p: 1,
        },
        ..Policy::standard(ASPECT)
    }
}

fn pepper() -> StaticPepper {
    StaticPepper::new(1, b"test pepper, never use in production".to_vec())
}

fn five() -> Vec<Point> {
    vec![
        Point::new(0.10, 0.10),
        Point::new(0.80, 0.20),
        Point::new(0.30, 0.60),
        Point::new(0.55, 0.35),
        Point::new(0.90, 0.70),
    ]
}

/// Small deterministic generator, so the tests need no extra dependency.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

fn nudge(p: Point, dist: f64, angle: f64) -> Point {
    Point::new(p.x + dist * angle.cos(), p.y + dist * angle.sin())
}

// ---------- geometry ----------

#[test]
fn hex_round_picks_nearest_centre() {
    assert_eq!(hex_round(0.0, 0.0), (0, 0));
    assert_eq!(hex_round(0.49, 0.0), (0, 0));
    assert_eq!(hex_round(0.51, 0.0), (1, 0));
    assert_eq!(hex_round(-0.51, 0.0), (-1, 0));
    assert_eq!(hex_round(3.1, -1.9), (3, -2));
}

#[test]
fn tolerance_is_a_hexagon_around_the_enrolled_point() {
    let lattice = Lattice::new(0.022);
    let (r, s) = (lattice.inradius(), lattice.circumradius());
    let mut rng = Lcg(42);
    for _ in 0..20_000 {
        let p = Point::new(0.1 + 0.8 * rng.next(), 0.1 + 0.5 * rng.next());
        let (index, offset) = lattice.split(p);
        let angle = rng.next() * std::f64::consts::TAU;

        // Anything inside the inradius is always accepted.
        let inside = nudge(p, 0.995 * r, angle);
        assert_eq!(lattice.locate(inside, offset), index, "inside rejected");

        // Anything beyond the circumradius is always rejected.
        let outside = nudge(p, 1.005 * s, angle);
        assert_ne!(lattice.locate(outside, offset), index, "outside accepted");
    }
}

#[test]
fn offset_does_not_depend_on_which_cell() {
    // Shifting a point by a whole lattice vector changes the index but not
    // the offset, so the offset says nothing about the cell.
    let lattice = Lattice::new(0.022);
    let step = 2.0 * lattice.inradius(); // length of basis vector a1
    let p = Point::new(0.2, 0.3);
    let (i1, o1) = lattice.split(p);
    let (i2, o2) = lattice.split(Point::new(p.x + 3.0 * step, p.y));
    assert_eq!(i2.q, i1.q + 3);
    assert_eq!(i2.t, i1.t);
    assert!((i32::from(o1.u) - i32::from(o2.u)).abs() <= 1);
    assert!((i32::from(o1.v) - i32::from(o2.v)).abs() <= 1);
}

// ---------- enrol and verify ----------

#[test]
fn exact_taps_verify() {
    let e = enrol(&five(), "img-1", &test_policy(), &pepper()).unwrap();
    assert!(e.warnings.is_empty());
    assert_eq!(e.record.tap_count(), 5);
    assert!(verify(&five(), &e.record, &pepper()).unwrap());
}

#[test]
fn nearby_taps_verify_and_far_taps_do_not() {
    let policy = test_policy();
    let e = enrol(&five(), "img-1", &policy, &pepper()).unwrap();
    let s = Lattice::new(policy.r).circumradius();
    let mut rng = Lcg(7);
    for _ in 0..50 {
        let near: Vec<Point> = five()
            .iter()
            .map(|p| {
                nudge(
                    *p,
                    0.99 * policy.r * rng.next(),
                    rng.next() * std::f64::consts::TAU,
                )
            })
            .collect();
        assert!(verify(&near, &e.record, &pepper()).unwrap());
    }
    for k in 0..5 {
        let mut taps = five();
        taps[k] = nudge(taps[k], 1.01 * s, rng.next() * std::f64::consts::TAU);
        assert!(!verify(&taps, &e.record, &pepper()).unwrap(), "tap {k}");
    }
}

#[test]
fn order_matters() {
    let e = enrol(&five(), "img-1", &test_policy(), &pepper()).unwrap();
    let mut swapped = five();
    swapped.swap(1, 3);
    assert!(!verify(&swapped, &e.record, &pepper()).unwrap());
    let mut reversed = five();
    reversed.reverse();
    assert!(!verify(&reversed, &e.record, &pepper()).unwrap());
}

#[test]
fn wrong_tap_count_fails_cleanly() {
    let e = enrol(&five(), "img-1", &test_policy(), &pepper()).unwrap();
    assert!(!verify(&five()[..4], &e.record, &pepper()).unwrap());
    let mut six = five();
    six.push(Point::new(0.5, 0.5));
    assert!(!verify(&six, &e.record, &pepper()).unwrap());
}

#[test]
fn out_of_bounds_and_non_finite_taps_fail_cleanly() {
    let e = enrol(&five(), "img-1", &test_policy(), &pepper()).unwrap();
    for bad in [
        Point::new(-0.1, 0.1),
        Point::new(0.1, 0.9),
        Point::new(f64::NAN, 0.1),
        Point::new(0.1, f64::INFINITY),
    ] {
        let mut taps = five();
        taps[2] = bad;
        assert!(!verify(&taps, &e.record, &pepper()).unwrap());
    }
}

#[test]
fn wrong_pepper_fails() {
    let e = enrol(&five(), "img-1", &test_policy(), &pepper()).unwrap();
    let other = StaticPepper::new(1, b"a different pepper".to_vec());
    assert!(!verify(&five(), &e.record, &other).unwrap());
}

#[test]
fn unknown_pepper_version_is_an_error_not_a_mismatch() {
    let e = enrol(&five(), "img-1", &test_policy(), &pepper()).unwrap();
    let rotated = StaticPepper::new(2, b"test pepper, never use in production".to_vec());
    assert!(matches!(
        verify(&five(), &e.record, &rotated),
        Err(Error::Pepper(_))
    ));
}

#[test]
fn record_is_bound_to_its_image_and_tolerance() {
    let e = enrol(&five(), "img-1", &test_policy(), &pepper()).unwrap();

    let mut other_image = e.record.clone();
    other_image.image_id = "img-2".into();
    assert!(!verify(&five(), &other_image, &pepper()).unwrap());

    let mut looser = e.record.clone();
    looser.r = 0.1;
    assert!(!verify(&five(), &looser, &pepper()).unwrap());
}

#[test]
fn tampered_offsets_never_error() {
    // Offsets carry no integrity check on purpose. Whatever the stored bytes
    // are, verification must return a plain yes or no, never an error, or an
    // attacker could test taps one at a time.
    let e = enrol(&five(), "img-1", &test_policy(), &pepper()).unwrap();
    let mut rng = Lcg(99);
    for _ in 0..200 {
        let mut r = e.record.clone();
        for b in r.enc_offsets.iter_mut() {
            *b = (rng.next() * 256.0) as u8;
        }
        for b in r.first_offset.iter_mut() {
            *b = (rng.next() * 256.0) as u8;
        }
        assert!(verify(&five(), &r, &pepper()).is_ok());
    }
}

// ---------- enrolment rules ----------

#[test]
fn tap_count_limits() {
    let p = test_policy();
    assert_eq!(
        enrol(&five()[..4], "i", &p, &pepper()).unwrap_err(),
        Error::TapCount {
            min: 5,
            max: 6,
            got: 4
        }
    );
    let mut seven = five();
    seven.push(Point::new(0.5, 0.1));
    seven.push(Point::new(0.2, 0.4));
    assert!(matches!(
        enrol(&seven, "i", &p, &pepper()),
        Err(Error::TapCount { got: 7, .. })
    ));
}

#[test]
fn consecutive_taps_must_be_apart() {
    let mut taps = five();
    taps[2] = Point::new(taps[1].x + 0.03, taps[1].y);
    assert_eq!(
        enrol(&taps, "i", &test_policy(), &pepper()).unwrap_err(),
        Error::ConsecutiveTooClose { tap: 2 }
    );
}

#[test]
fn one_revisit_is_allowed_and_snapped() {
    let policy = test_policy();
    // A, B, A', C, D, E where A' is a slightly different tap on A.
    let a = Point::new(0.10, 0.10);
    let a2 = Point::new(0.11, 0.105);
    let taps = vec![
        a,
        Point::new(0.80, 0.20),
        a2,
        Point::new(0.30, 0.60),
        Point::new(0.55, 0.35),
        Point::new(0.90, 0.70),
    ];
    let e = enrol(&taps, "i", &policy, &pepper()).unwrap();
    assert_eq!(e.record.tap_count(), 6);

    // The revisit shares A's tolerance region exactly: tapping the original
    // A in both positions works.
    let mut login = taps.clone();
    login[2] = a;
    assert!(verify(&login, &e.record, &pepper()).unwrap());

    // A tap that is near A' but outside A's hexagon is rejected, because the
    // region was snapped to A, not centred on A'.
    let lattice = Lattice::new(policy.r);
    let far_side = Point::new(a2.x + 0.02, a2.y);
    assert!(!lattice.within_tolerance(a, far_side));
    login[2] = far_side;
    assert!(!verify(&login, &e.record, &pepper()).unwrap());
}

#[test]
fn revisit_is_not_visible_in_the_record() {
    let a = Point::new(0.10, 0.10);
    let taps = vec![
        a,
        Point::new(0.80, 0.20),
        a,
        Point::new(0.30, 0.60),
        Point::new(0.55, 0.35),
        Point::new(0.90, 0.70),
    ];
    let e = enrol(&taps, "i", &test_policy(), &pepper()).unwrap();
    // Tap 3 has the same true offset as tap 1, but its stored bytes differ.
    assert_ne!(&e.record.enc_offsets[4..8], &e.record.first_offset[..]);
}

#[test]
fn too_many_revisits_are_rejected() {
    let a = Point::new(0.10, 0.10);
    let b = Point::new(0.80, 0.20);
    // A, B, A, B, A: allowed by the consecutive rule, but only 2 distinct.
    assert_eq!(
        enrol(&[a, b, a, b, a], "i", &test_policy(), &pepper()).unwrap_err(),
        Error::TooFewDistinct {
            required: 5,
            got: 2
        }
    );
    // Six taps with two revisits: only 4 distinct.
    let c = Point::new(0.30, 0.60);
    let d = Point::new(0.55, 0.35);
    assert_eq!(
        enrol(&[a, b, a, c, d, c], "i", &test_policy(), &pepper()).unwrap_err(),
        Error::TooFewDistinct {
            required: 5,
            got: 4
        }
    );
}

#[test]
fn close_points_produce_a_warning() {
    let mut taps = five();
    // Non-consecutive, 0.06 apart: outside each other's hexagon, inside 4r.
    taps[3] = Point::new(taps[0].x + 0.06, taps[0].y);
    let e = enrol(&taps, "i", &test_policy(), &pepper()).unwrap();
    assert_eq!(
        e.warnings,
        vec![Warning::PointsClose {
            first: 0,
            second: 3
        }]
    );
    assert!(verify(&taps, &e.record, &pepper()).unwrap());
}

#[test]
fn enrolment_rejects_points_outside_the_image() {
    let mut taps = five();
    taps[4] = Point::new(0.5, 0.76);
    assert_eq!(
        enrol(&taps, "i", &test_policy(), &pepper()).unwrap_err(),
        Error::PointOutOfBounds { tap: 4 }
    );
}

// ---------- record format ----------

#[test]
fn salts_differ_between_enrolments() {
    let a = enrol(&five(), "i", &test_policy(), &pepper())
        .unwrap()
        .record;
    let b = enrol(&five(), "i", &test_policy(), &pepper())
        .unwrap()
        .record;
    assert_ne!(a.salt, b.salt);
    assert_ne!(a.verifier, b.verifier);
    assert_ne!(a.enc_offsets, b.enc_offsets);
    // The first offset is public geometry and is the same.
    assert_eq!(a.first_offset, b.first_offset);
}

#[test]
fn record_round_trips_through_json() {
    let e = enrol(&five(), "img-1", &test_policy(), &pepper()).unwrap();
    let json = serde_json::to_string(&e.record).unwrap();
    let back: Record = serde_json::from_str(&json).unwrap();
    assert_eq!(back, e.record);
    assert!(verify(&five(), &back, &pepper()).unwrap());
}

#[test]
fn known_answer_vector() {
    // Fixed inputs must always produce this exact record. If this test
    // fails, the format has changed and existing records will not verify.
    let salt = *b"0123456789abcdef";
    let e = enrol_with_salt(&five(), "kat-image", &test_policy(), &pepper(), salt).unwrap();
    let json = serde_json::to_string(&e.record).unwrap();
    assert_eq!(json, include_str!("vectors/kat1.json").trim());
}
