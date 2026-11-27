//! Hexagonal lattice geometry.
//!
//! All coordinates are in units of image width: x runs 0..=1 and y runs
//! 0..=aspect, where aspect = height / width. Screen pixels never appear here.
//!
//! Hexagons are pointy-top. `r` is the inradius (the guaranteed tolerance) and
//! `s = 2r / sqrt(3)` is the circumradius (the worst-case accepted distance).
//! Hexagon centres form a lattice with basis vectors
//! a1 = (sqrt(3) s, 0) and a2 = (sqrt(3) s / 2, 1.5 s).

/// sqrt(3) to full f64 precision.
pub const SQRT3: f64 = 1.732_050_807_568_877_2;

/// Offsets are quantised to this many steps per lattice basis vector.
pub const OFFSET_STEPS: f64 = 65536.0;

/// Smallest tolerance accepted, as a fraction of image width.
///
/// Below this, cell indices for a large image could exceed an `i32`.
pub const MIN_R: f64 = 1e-4;

/// Tolerance must be strictly below this fraction of image width.
pub const MAX_R: f64 = 0.25;

/// Largest image aspect ratio (height / width) accepted.
pub const MAX_ASPECT: f64 = 16.0;

/// A point on the image, in units of image width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// Horizontal position, 0 at the left edge and 1 at the right.
    pub x: f64,
    /// Vertical position, 0 at the top and `aspect` at the bottom.
    pub y: f64,
}

impl Point {
    /// A point at (`x`, `y`), in units of image width.
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Straight-line distance to `other`, in units of image width.
    pub fn distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

/// The hexagon lattice for a given tolerance.
#[derive(Clone, Copy, Debug)]
pub struct Lattice {
    r: f64,
    s: f64,
}

impl Lattice {
    /// A lattice whose hexagons have inradius `r`, in width units.
    ///
    /// Returns `None` unless `r` is finite and within [`MIN_R`]..[`MAX_R`].
    pub fn new(r: f64) -> Option<Self> {
        if r.is_finite() && (MIN_R..MAX_R).contains(&r) {
            Some(Self {
                r,
                s: 2.0 * r / SQRT3,
            })
        } else {
            None
        }
    }

    /// Guaranteed tolerance.
    pub fn inradius(&self) -> f64 {
        self.r
    }

    /// Worst-case accepted distance.
    pub fn circumradius(&self) -> f64 {
        self.s
    }

    /// Convert a point to fractional lattice coordinates (q, t), such that
    /// point = q * a1 + t * a2.
    pub fn to_lattice(&self, p: Point) -> (f64, f64) {
        let t = p.y / (1.5 * self.s);
        let q = p.x / (SQRT3 * self.s) - t / 2.0;
        (q, t)
    }

    /// Split a point into its secret cell index and its public offset.
    ///
    /// The index is the integer part of the lattice coordinates. The offset is
    /// the fractional part, quantised to 16 bits per axis. By construction,
    /// `point - offset` lands on a lattice point, which is a hexagon centre.
    pub fn split(&self, p: Point) -> (CellIndex, Offset) {
        let (q, t) = self.to_lattice(p);
        let (qi, ti) = (q.floor(), t.floor());
        (
            CellIndex {
                q: to_i32(qi),
                t: to_i32(ti),
            },
            Offset {
                u: quantise(q - qi),
                v: quantise(t - ti),
            },
        )
    }

    /// Recover the cell index for a tap, given the offset stored at enrolment.
    ///
    /// Any tap inside the hexagon centred on the enrolled point yields the
    /// enrolled index.
    pub fn locate(&self, tap: Point, offset: Offset) -> CellIndex {
        let (q, t) = self.to_lattice(tap);
        let (a, c) = hex_round(
            q - f64::from(offset.u) / OFFSET_STEPS,
            t - f64::from(offset.v) / OFFSET_STEPS,
        );
        CellIndex { q: a, t: c }
    }

    /// True if `tap` falls inside the hexagon centred on `centre`.
    pub fn within_tolerance(&self, centre: Point, tap: Point) -> bool {
        let (cq, ct) = self.to_lattice(centre);
        let (q, t) = self.to_lattice(tap);
        hex_round(q - cq, t - ct) == (0, 0)
    }
}

/// Round fractional lattice coordinates to the nearest hexagon centre, using
/// cube-coordinate rounding.
pub fn hex_round(q: f64, t: f64) -> (i32, i32) {
    let (a, c) = (q, t);
    let b = -a - c;
    let (mut ra, rb, mut rc) = (a.round(), b.round(), c.round());
    let (da, db, dc) = ((ra - a).abs(), (rb - b).abs(), (rc - c).abs());
    // Cube coordinates must sum to zero. Recompute whichever of the three
    // moved furthest. When that is `b`, the two we return are already right.
    if da > db && da > dc {
        ra = -rb - rc;
    } else if db <= dc {
        rc = -ra - rb;
    }
    (to_i32(ra), to_i32(rc))
}

/// Convert an already-integral float to `i32`, saturating at the type's
/// bounds. NaN becomes 0.
///
/// Callers keep values far inside the range: `Lattice::new` bounds `r` and the
/// public API bounds the image, so saturation is a backstop, not a code path.
#[allow(clippy::cast_possible_truncation)]
fn to_i32(v: f64) -> i32 {
    v.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

/// Quantise a fraction in 0..1 to 16 bits.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn quantise(frac: f64) -> u16 {
    (frac * OFFSET_STEPS)
        .floor()
        .clamp(0.0, f64::from(u16::MAX)) as u16
}

/// The secret part of an enrolled point: which hexagon it is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellIndex {
    /// Lattice coordinate along basis vector a1.
    pub q: i32,
    /// Lattice coordinate along basis vector a2.
    pub t: i32,
}

impl CellIndex {
    /// Fixed-width encoding: two big-endian i32 values.
    pub fn encode(&self) -> [u8; 8] {
        let [q0, q1, q2, q3] = self.q.to_be_bytes();
        let [t0, t1, t2, t3] = self.t.to_be_bytes();
        [q0, q1, q2, q3, t0, t1, t2, t3]
    }
}

/// The public part of an enrolled point: where it sits within its lattice
/// cell. Reveals nothing about which cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offset {
    /// Position along basis vector a1, in 1/65536ths of a cell.
    pub u: u16,
    /// Position along basis vector a2, in 1/65536ths of a cell.
    pub v: u16,
}

impl Offset {
    /// Fixed-width encoding: two big-endian u16 values.
    pub fn to_bytes(self) -> [u8; 4] {
        let [u0, u1] = self.u.to_be_bytes();
        let [v0, v1] = self.v.to_be_bytes();
        [u0, u1, v0, v1]
    }

    /// Inverse of [`Offset::to_bytes`]. Every input is a valid offset.
    pub fn from_bytes(b: [u8; 4]) -> Self {
        let [u0, u1, v0, v1] = b;
        Self {
            u: u16::from_be_bytes([u0, u1]),
            v: u16::from_be_bytes([v0, v1]),
        }
    }
}
