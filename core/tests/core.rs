use pixel_factor_core::geometry::{hex_round, Lattice, Point};
use pixel_factor_core::{
    enrol, enrol_deterministic, verify, Error, KdfParams, Policy, Record, StaticPepper, Warning,
    ENC_OFFSETS_LEN,
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
    let lattice = Lattice::new(0.022).unwrap();
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
    let lattice = Lattice::new(0.022).unwrap();
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
    assert!(verify(&five(), &e.record, &pepper()).unwrap());
}

#[test]
fn nearby_taps_verify_and_far_taps_do_not() {
    let policy = test_policy();
    let e = enrol(&five(), "img-1", &policy, &pepper()).unwrap();
    let s = Lattice::new(policy.r).unwrap().circumradius();
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
fn records_do_not_reveal_the_tap_count() {
    // A five-tap and a six-tap record have every field the same size, and the
    // slots past the last tap differ between enrolments of the same taps.
    let mut six = five();
    six.push(Point::new(0.40, 0.10));
    let a = enrol(&five(), "img-1", &test_policy(), &pepper())
        .unwrap()
        .record;
    let b = enrol(&six, "img-1", &test_policy(), &pepper())
        .unwrap()
        .record;
    let again = enrol(&five(), "img-1", &test_policy(), &pepper())
        .unwrap()
        .record;
    assert_eq!(a.enc_offsets.len(), ENC_OFFSETS_LEN);
    assert_eq!(b.enc_offsets.len(), ENC_OFFSETS_LEN);
    assert_eq!(
        serde_json::to_string(&a).unwrap().len(),
        serde_json::to_string(&b).unwrap().len()
    );
    assert_ne!(a.enc_offsets[16..], again.enc_offsets[16..]);
}

#[test]
fn every_count_is_tried_against_the_record() {
    // Any count up to MAX_TAPS is a well-formed attempt: it fails at the
    // verifier, not by a count check, so the record reveals nothing.
    let e = enrol(&five(), "img-1", &test_policy(), &pepper()).unwrap();
    let mut taps = five();
    while taps.len() < 32 {
        taps.push(Point::new(0.5, 0.5));
    }
    for n in 1..=32 {
        assert_eq!(
            verify(&taps[..n], &e.record, &pepper()),
            Ok(n == 5),
            "n = {n}"
        );
    }
    assert!(!verify(&[], &e.record, &pepper()).unwrap());
    let mut too_many = taps.clone();
    too_many.push(Point::new(0.5, 0.5));
    assert!(!verify(&too_many, &e.record, &pepper()).unwrap());
}

#[test]
fn fill_does_not_affect_verification() {
    let salt = *b"0123456789abcdef";
    let a = enrol_deterministic(
        &five(),
        "i",
        &test_policy(),
        &pepper(),
        salt,
        [0; ENC_OFFSETS_LEN],
    )
    .unwrap();
    let b = enrol_deterministic(
        &five(),
        "i",
        &test_policy(),
        &pepper(),
        salt,
        [0xff; ENC_OFFSETS_LEN],
    )
    .unwrap();
    assert_eq!(a.record.enc_offsets[..16], b.record.enc_offsets[..16]);
    assert_eq!(a.record.verifier, b.record.verifier);
    assert!(verify(&five(), &b.record, &pepper()).unwrap());
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
            max: 8,
            got: 4
        }
    );
    let mut nine = five();
    nine.extend([0.1, 0.3, 0.5, 0.7].map(|x| Point::new(x, 0.45)));
    assert!(matches!(
        enrol(&nine, "i", &p, &pepper()),
        Err(Error::TapCount { got: 9, .. })
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

    // The revisit shares A's tolerance region exactly: tapping the original
    // A in both positions works.
    let mut login = taps.clone();
    login[2] = a;
    assert!(verify(&login, &e.record, &pepper()).unwrap());

    // A tap that is near A' but outside A's hexagon is rejected, because the
    // region was snapped to A, not centred on A'.
    let lattice = Lattice::new(policy.r).unwrap();
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
fn revisits_are_unlimited_within_the_tap_count() {
    let (a, b, c) = (
        Point::new(0.10, 0.10),
        Point::new(0.80, 0.20),
        Point::new(0.30, 0.60),
    );
    let (d, e) = (Point::new(0.55, 0.35), Point::new(0.90, 0.70));
    // Eight taps on five distinct points, returning to A three times.
    let taps = [a, b, a, c, a, d, e, a];
    let enrolment = enrol(&taps, "i", &test_policy(), &pepper()).unwrap();
    assert!(verify(&taps, &enrolment.record, &pepper()).unwrap());
    let mut wrong = taps;
    wrong[4] = c;
    assert!(!verify(&wrong, &enrolment.record, &pepper()).unwrap());
}

#[test]
fn revisits_still_need_enough_distinct_points() {
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
    let fill = [0x5a; ENC_OFFSETS_LEN];
    let e =
        enrol_deterministic(&five(), "kat-image", &test_policy(), &pepper(), salt, fill).unwrap();
    let json = serde_json::to_string(&e.record).unwrap();
    assert_eq!(json, include_str!("vectors/kat2.json").trim());
}

// ---------- input limits ----------

#[test]
fn tolerance_outside_the_supported_range_is_rejected() {
    for r in [0.0, -0.01, 0.00009, 0.25, 0.3, f64::NAN, f64::INFINITY] {
        assert!(Lattice::new(r).is_none(), "r = {r}");
        let policy = Policy { r, ..test_policy() };
        assert!(matches!(
            enrol(&five(), "i", &policy, &pepper()),
            Err(Error::InvalidPolicy(_))
        ));
    }
    assert!(Lattice::new(0.0001).is_some());
    assert!(Lattice::new(0.2499).is_some());
}

#[test]
fn aspect_outside_the_supported_range_is_rejected() {
    for aspect in [0.0, -1.0, 16.5, f64::NAN, f64::INFINITY] {
        let policy = Policy {
            aspect,
            ..test_policy()
        };
        assert!(matches!(
            enrol(&five(), "i", &policy, &pepper()),
            Err(Error::InvalidPolicy(_))
        ));
    }
}

#[test]
fn image_id_must_be_present_and_bounded() {
    let p = test_policy();
    assert_eq!(
        enrol(&five(), "", &p, &pepper()).unwrap_err(),
        Error::InvalidImageId
    );
    let long = "x".repeat(256);
    assert_eq!(
        enrol(&five(), &long, &p, &pepper()).unwrap_err(),
        Error::InvalidImageId
    );
    assert!(enrol(&five(), &"x".repeat(255), &p, &pepper()).is_ok());
}

#[test]
fn malformed_records_are_errors_not_mismatches() {
    let good = enrol(&five(), "img-1", &test_policy(), &pepper())
        .unwrap()
        .record;

    let mut r = good.clone();
    r.version = 1;
    assert!(matches!(
        verify(&five(), &r, &pepper()),
        Err(Error::InvalidRecord(_))
    ));

    let mut r = good.clone();
    r.enc_offsets.pop();
    assert!(matches!(
        verify(&five(), &r, &pepper()),
        Err(Error::InvalidRecord(_))
    ));

    let mut r = good.clone();
    r.enc_offsets.push(0);
    assert!(matches!(
        verify(&five(), &r, &pepper()),
        Err(Error::InvalidRecord(_))
    ));

    for bad_r in [0.0, 0.5, f64::NAN] {
        let mut r = good.clone();
        r.r = bad_r;
        assert!(matches!(
            verify(&five(), &r, &pepper()),
            Err(Error::InvalidRecord(_))
        ));
    }

    let mut r = good.clone();
    r.aspect = f64::INFINITY;
    assert!(matches!(
        verify(&five(), &r, &pepper()),
        Err(Error::InvalidRecord(_))
    ));

    let mut r = good;
    r.image_id = String::new();
    assert!(matches!(
        verify(&five(), &r, &pepper()),
        Err(Error::InvalidRecord(_))
    ));
}

#[test]
fn the_smallest_tolerance_still_round_trips() {
    // Far more cells than the default, to exercise large indices.
    let policy = Policy {
        r: 0.0001,
        aspect: 16.0,
        ..test_policy()
    };
    let taps = vec![
        Point::new(0.999, 15.99),
        Point::new(0.001, 0.001),
        Point::new(0.5, 8.0),
        Point::new(0.25, 12.0),
        Point::new(0.75, 4.0),
    ];
    let e = enrol(&taps, "i", &policy, &pepper()).unwrap();
    assert!(verify(&taps, &e.record, &pepper()).unwrap());
    let mut off = taps.clone();
    off[0] = Point::new(0.999 - 0.00013, 15.99);
    assert!(!verify(&off, &e.record, &pepper()).unwrap());
}

// ---------- boundaries found by mutation testing ----------

#[test]
fn hex_round_breaks_ties_the_same_way_every_time() {
    // On a cell edge either neighbour is a correct answer. Which one is
    // chosen must never change, or boundary taps would flip between releases.
    assert_eq!(hex_round(0.5, 0.5), (1, 0));
    assert_eq!(hex_round(0.5, -1.0), (1, -1));
    assert_eq!(hex_round(0.5, 0.0), (1, 0));
    assert_eq!(hex_round(-0.5, 0.0), (-1, 0));
}

#[test]
fn errors_describe_themselves() {
    let text = Error::TapCount {
        min: 5,
        max: 6,
        got: 4,
    }
    .to_string();
    assert!(text.contains("TapCount") && text.contains('4'), "{text}");
    assert!(Error::InvalidImageId.to_string().contains("InvalidImageId"));
}

#[test]
fn records_carry_the_pepper_version_in_use() {
    let p7 = StaticPepper::new(7, b"another pepper".to_vec());
    let record = enrol(&five(), "i", &test_policy(), &p7).unwrap().record;
    assert_eq!(record.pepper_version, 7);
    assert!(verify(&five(), &record, &p7).unwrap());
}

#[test]
fn policy_limits_are_exact() {
    // Fewer than two distinct points is never a sequence.
    let one = Policy {
        min_distinct: 1,
        max_taps: 1,
        ..test_policy()
    };
    assert!(matches!(
        enrol(&five(), "i", &one, &pepper()),
        Err(Error::InvalidPolicy(_))
    ));
    let two = Policy {
        min_distinct: 2,
        max_taps: 2,
        ..test_policy()
    };
    assert!(enrol(&five()[..2], "i", &two, &pepper()).is_ok());

    // The most taps may not be fewer than the distinct points required.
    let inverted = Policy {
        min_distinct: 5,
        max_taps: 4,
        ..test_policy()
    };
    assert!(matches!(
        enrol(&five(), "i", &inverted, &pepper()),
        Err(Error::InvalidPolicy(_))
    ));
    let exact = Policy {
        min_distinct: 5,
        max_taps: 5,
        ..test_policy()
    };
    assert!(enrol(&five(), "i", &exact, &pepper()).is_ok());

    // 32 taps is the most a policy may allow.
    let at_limit = Policy {
        min_distinct: 6,
        max_taps: 32,
        ..test_policy()
    };
    assert_eq!(
        enrol(&five(), "i", &at_limit, &pepper()).unwrap_err(),
        Error::TapCount {
            min: 6,
            max: 32,
            got: 5
        }
    );
    let over_limit = Policy {
        min_distinct: 6,
        max_taps: 33,
        ..test_policy()
    };
    assert!(matches!(
        enrol(&five(), "i", &over_limit, &pepper()),
        Err(Error::InvalidPolicy(_))
    ));
}

#[test]
fn a_full_length_sequence_enrols_and_verifies() {
    let policy = Policy {
        min_distinct: 32,
        max_taps: 32,
        ..test_policy()
    };
    // 32 points on a grid, visited in an order that keeps neighbours apart.
    let taps: Vec<Point> = (0..32)
        .map(|i| Point::new(0.05 + 0.1 * f64::from(i % 8), 0.1 + 0.15 * f64::from(i / 8)))
        .collect();
    let e = enrol(&taps, "i", &policy, &pepper()).unwrap();
    assert!(verify(&taps, &e.record, &pepper()).unwrap());
}

#[test]
fn consecutive_taps_exactly_two_circumradii_apart_are_allowed() {
    let policy = test_policy();
    let s = Lattice::new(policy.r).unwrap().circumradius();
    let mut taps = five();
    taps[0] = Point::new(0.0, 0.4);
    taps[1] = Point::new(2.0 * s, 0.4);
    assert_eq!(taps[0].distance(&taps[1]), 2.0 * s);
    assert!(enrol(&taps, "i", &policy, &pepper()).is_ok());

    taps[1] = Point::new(2.0 * s - 1e-9, 0.4);
    assert_eq!(
        enrol(&taps, "i", &policy, &pepper()).unwrap_err(),
        Error::ConsecutiveTooClose { tap: 1 }
    );
}

#[test]
fn points_exactly_four_r_apart_do_not_warn() {
    let policy = test_policy();
    let mut taps = five();
    taps[0] = Point::new(0.0, 0.4);
    taps[2] = Point::new(4.0 * policy.r, 0.4);
    assert_eq!(taps[0].distance(&taps[2]), 4.0 * policy.r);
    let e = enrol(&taps, "i", &policy, &pepper()).unwrap();
    assert!(e.warnings.is_empty(), "{:?}", e.warnings);

    taps[2] = Point::new(4.0 * policy.r - 1e-9, 0.4);
    let e = enrol(&taps, "i", &policy, &pepper()).unwrap();
    assert_eq!(
        e.warnings,
        vec![Warning::PointsClose {
            first: 0,
            second: 2
        }]
    );
}
