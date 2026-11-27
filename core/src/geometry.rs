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

/// A point on the image, in units of image width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

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
    /// `r` is the guaranteed tolerance (hexagon inradius), in width units.
    pub fn new(r: f64) -> Self {
        Self {
            r,
            s: 2.0 * r / SQRT3,
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
        let quantise = |frac: f64| -> u16 {
            let v = (frac * OFFSET_STEPS).floor();
            v.clamp(0.0, OFFSET_STEPS - 1.0) as u16
        };
        (
            CellIndex {
                q: qi as i32,
                t: ti as i32,
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
        CellIndex {
            q: a as i32,
            t: c as i32,
        }
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
pub fn hex_round(q: f64, t: f64) -> (i64, i64) {
    let (a, c) = (q, t);
    let b = -a - c;
    let (mut ra, mut rb, mut rc) = (a.round(), b.round(), c.round());
    let (da, db, dc) = ((ra - a).abs(), (rb - b).abs(), (rc - c).abs());
    if da > db && da > dc {
        ra = -rb - rc;
    } else if db > dc {
        rb = -ra - rc;
    } else {
        rc = -ra - rb;
    }
    let _ = rb;
    (ra as i64, rc as i64)
}

/// The secret part of an enrolled point: which hexagon it is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellIndex {
    pub q: i32,
    pub t: i32,
}

impl CellIndex {
    /// Fixed-width encoding: two big-endian i32 values.
    pub fn encode(&self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[..4].copy_from_slice(&self.q.to_be_bytes());
        out[4..].copy_from_slice(&self.t.to_be_bytes());
        out
    }
}

/// The public part of an enrolled point: where it sits within its lattice
/// cell. Reveals nothing about which cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offset {
    pub u: u16,
    pub v: u16,
}

impl Offset {
    pub fn to_bytes(self) -> [u8; 4] {
        let mut out = [0u8; 4];
        out[..2].copy_from_slice(&self.u.to_be_bytes());
        out[2..].copy_from_slice(&self.v.to_be_bytes());
        out
    }

    pub fn from_bytes(b: [u8; 4]) -> Self {
        Self {
            u: u16::from_be_bytes([b[0], b[1]]),
            v: u16::from_be_bytes([b[2], b[3]]),
        }
    }
}
