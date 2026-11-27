//! Breach-resistant verification of an ordered sequence of taps on an image.
//!
//! A user enrols by tapping a sequence of points on an image. Later taps are
//! accepted if each lands within a hexagonal tolerance of the enrolled point,
//! in the same order. The stored [`Record`] cannot be used to recover or guess
//! the sequence without a pepper that is held outside the database.
//!
//! How it works:
//!
//! * Each point is split into a secret cell index and a public offset
//!   ([`geometry::Lattice::split`]). Subtracting the offset from any nearby
//!   tap and rounding to the nearest hexagon centre recovers the index.
//! * The first offset is stored in the clear. Each later offset is encrypted
//!   under a key derived from the pepper and all earlier indices, so a stolen
//!   record does not show which taps revisit a point.
//! * One verifier covers the whole ordered sequence:
//!   `HMAC(pepper, Argon2id(indices, salt))`. There is never a hash per point.
//!
//! This crate does no I/O. Rate limiting, lockout, and record storage belong
//! to the caller.

pub mod geometry;

use argon2::{Algorithm, Argon2, Params, Version};
use geometry::{CellIndex, Lattice, Offset, Point};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

/// Format version written into every record.
pub const RECORD_VERSION: u8 = 1;

const DOMAIN_OFFSET: &[u8] = b"pfv1/offset";
const DOMAIN_KDF: &[u8] = b"pfv1/kdf";
const DOMAIN_FINAL: &[u8] = b"pfv1/final";

/// Errors from enrolment or from the pepper provider.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// Policy values are out of range.
    InvalidPolicy(&'static str),
    /// A tap is not a finite coordinate inside the image.
    PointOutOfBounds { tap: usize },
    /// Wrong number of taps for the policy.
    TapCount { min: usize, max: usize, got: usize },
    /// Two consecutive taps are less than two circumradii apart.
    ConsecutiveTooClose { tap: usize },
    /// Not enough distinct points in the sequence.
    TooFewDistinct { required: usize, got: usize },
    /// The record is malformed.
    InvalidRecord(&'static str),
    /// Argon2 rejected its parameters or failed.
    Kdf(String),
    /// The pepper provider failed, or does not hold the requested version.
    Pepper(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Error {}

/// Performs the keyed HMAC. In production this is a KMS or HSM call, so the
/// pepper never sits in application memory or beside the database.
pub trait PepperProvider {
    /// The version new records should be written with.
    fn current_version(&self) -> u32;
    /// HMAC-SHA256 of `msg` under the pepper with the given version.
    fn hmac(&self, version: u32, msg: &[u8]) -> Result<[u8; 32], Error>;
}

/// An in-memory pepper. For development and tests only.
pub struct StaticPepper {
    version: u32,
    key: Vec<u8>,
}

impl StaticPepper {
    pub fn new(version: u32, key: impl Into<Vec<u8>>) -> Self {
        Self {
            version,
            key: key.into(),
        }
    }
}

impl PepperProvider for StaticPepper {
    fn current_version(&self) -> u32 {
        self.version
    }

    fn hmac(&self, version: u32, msg: &[u8]) -> Result<[u8; 32], Error> {
        if version != self.version {
            return Err(Error::Pepper(format!("unknown pepper version {version}")));
        }
        let mut mac =
            HmacSha256::new_from_slice(&self.key).map_err(|e| Error::Pepper(e.to_string()))?;
        mac.update(msg);
        Ok(mac.finalize().into_bytes().into())
    }
}

/// Argon2id cost parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory in KiB.
    pub m_kib: u32,
    /// Iterations.
    pub t: u32,
    /// Parallelism.
    pub p: u32,
}

impl Default for KdfParams {
    /// 64 MiB, 3 iterations, parallelism 1.
    fn default() -> Self {
        Self {
            m_kib: 64 * 1024,
            t: 3,
            p: 1,
        }
    }
}

/// Enrolment rules.
#[derive(Clone, Debug, PartialEq)]
pub struct Policy {
    /// Guaranteed tolerance, as a fraction of image width.
    pub r: f64,
    /// Image height divided by width.
    pub aspect: f64,
    /// Minimum number of distinct points.
    pub min_distinct: usize,
    /// How many taps may revisit an earlier point.
    pub max_revisits: usize,
    pub kdf: KdfParams,
}

impl Policy {
    /// r = 2.2% of width, 5 distinct points, 1 revisit, default Argon2id.
    pub fn standard(aspect: f64) -> Self {
        Self {
            r: 0.022,
            aspect,
            min_distinct: 5,
            max_revisits: 1,
            kdf: KdfParams::default(),
        }
    }

    fn validate(&self) -> Result<(), Error> {
        if !(self.r.is_finite() && self.r > 0.0 && self.r < 0.25) {
            return Err(Error::InvalidPolicy("r must be in (0, 0.25)"));
        }
        if !(self.aspect.is_finite() && self.aspect > 0.0) {
            return Err(Error::InvalidPolicy("aspect must be positive"));
        }
        if self.min_distinct < 2 {
            return Err(Error::InvalidPolicy("min_distinct must be at least 2"));
        }
        if self.min_distinct + self.max_revisits > 32 {
            return Err(Error::InvalidPolicy("at most 32 taps"));
        }
        Ok(())
    }
}

/// What gets stored for a user. Safe to keep in the database: without the
/// pepper it cannot be checked against guesses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub version: u8,
    pub image_id: String,
    pub r: f64,
    pub aspect: f64,
    pub kdf: KdfParams,
    pub pepper_version: u32,
    #[serde(with = "hex")]
    pub salt: [u8; 16],
    /// Offset of the first tap, in the clear.
    #[serde(with = "hex")]
    pub first_offset: [u8; 4],
    /// Encrypted offsets of taps 2..n, four bytes each.
    #[serde(with = "hex")]
    pub enc_offsets: Vec<u8>,
    #[serde(with = "hex")]
    pub verifier: [u8; 32],
}

impl Record {
    /// Number of taps in the enrolled sequence.
    pub fn tap_count(&self) -> usize {
        1 + self.enc_offsets.len() / 4
    }

    /// Bytes that bind the keys and verifier to this record's parameters, so
    /// a record cannot be reused under a different image or tolerance.
    fn context(&self) -> Vec<u8> {
        context(&self.image_id, self.r, self.aspect)
    }
}

/// Non-fatal findings from enrolment.
#[derive(Clone, Debug, PartialEq)]
pub enum Warning {
    /// Two distinct points are closer than 4r. Valid, but fiddly on a phone.
    PointsClose { first: usize, second: usize },
}

/// Result of a successful enrolment.
#[derive(Clone, Debug)]
pub struct Enrolment {
    pub record: Record,
    pub warnings: Vec<Warning>,
}

/// Enrol a sequence of taps.
pub fn enrol(
    taps: &[Point],
    image_id: &str,
    policy: &Policy,
    pepper: &dyn PepperProvider,
) -> Result<Enrolment, Error> {
    let mut salt = [0u8; 16];
    getrandom::getrandom(&mut salt).map_err(|e| Error::Kdf(e.to_string()))?;
    enrol_with_salt(taps, image_id, policy, pepper, salt)
}

/// Enrol with a caller-supplied salt. Exists so that test vectors are
/// reproducible. Production code should call [`enrol`].
pub fn enrol_with_salt(
    taps: &[Point],
    image_id: &str,
    policy: &Policy,
    pepper: &dyn PepperProvider,
    salt: [u8; 16],
) -> Result<Enrolment, Error> {
    policy.validate()?;
    let lattice = Lattice::new(policy.r);

    let (min, max) = (
        policy.min_distinct,
        policy.min_distinct + policy.max_revisits,
    );
    if taps.len() < min || taps.len() > max {
        return Err(Error::TapCount {
            min,
            max,
            got: taps.len(),
        });
    }
    for (i, p) in taps.iter().enumerate() {
        if !in_bounds(p, policy.aspect) {
            return Err(Error::PointOutOfBounds { tap: i });
        }
    }

    // Snap revisits to the earlier point, and track which taps are distinct.
    let mut points: Vec<Point> = Vec::with_capacity(taps.len());
    let mut first_seen: Vec<usize> = Vec::with_capacity(taps.len());
    for (i, tap) in taps.iter().enumerate() {
        if i > 0 && tap.distance(&points[i - 1]) < 2.0 * lattice.circumradius() {
            return Err(Error::ConsecutiveTooClose { tap: i });
        }
        match (0..i).find(|&j| first_seen[j] == j && lattice.within_tolerance(points[j], *tap)) {
            Some(j) => {
                points.push(points[j]);
                first_seen.push(j);
            }
            None => {
                points.push(*tap);
                first_seen.push(i);
            }
        }
    }

    let distinct: Vec<usize> = (0..points.len()).filter(|&i| first_seen[i] == i).collect();
    if distinct.len() < policy.min_distinct {
        return Err(Error::TooFewDistinct {
            required: policy.min_distinct,
            got: distinct.len(),
        });
    }

    let mut warnings = Vec::new();
    for (n, &i) in distinct.iter().enumerate() {
        for &j in &distinct[n + 1..] {
            if points[i].distance(&points[j]) < 4.0 * policy.r {
                warnings.push(Warning::PointsClose {
                    first: i,
                    second: j,
                });
            }
        }
    }

    let pepper_version = pepper.current_version();
    let ctx = context(image_id, policy.r, policy.aspect);
    let mut indices: Vec<CellIndex> = Vec::with_capacity(points.len());
    let mut first_offset = [0u8; 4];
    let mut enc_offsets = Vec::with_capacity(4 * (points.len() - 1));
    for (i, p) in points.iter().enumerate() {
        let (index, offset) = lattice.split(*p);
        if i == 0 {
            first_offset = offset.to_bytes();
        } else {
            let ks = offset_keystream(pepper, pepper_version, &salt, &ctx, &indices)?;
            enc_offsets.extend_from_slice(&xor4(offset.to_bytes(), ks));
        }
        indices.push(index);
    }

    let verifier = compute_verifier(pepper, pepper_version, &salt, &ctx, &policy.kdf, &indices)?;

    Ok(Enrolment {
        record: Record {
            version: RECORD_VERSION,
            image_id: image_id.to_owned(),
            r: policy.r,
            aspect: policy.aspect,
            kdf: policy.kdf,
            pepper_version,
            salt,
            first_offset,
            enc_offsets,
            verifier,
        },
        warnings,
    })
}

/// Check a sequence of taps against a record.
///
/// Returns `Ok(true)` only if every tap matches, in order. The result is all
/// or nothing: there is no indication of which tap was wrong. `Err` means the
/// record or the pepper provider is at fault, not the taps.
pub fn verify(taps: &[Point], record: &Record, pepper: &dyn PepperProvider) -> Result<bool, Error> {
    if record.version != RECORD_VERSION {
        return Err(Error::InvalidRecord("unsupported version"));
    }
    if !record.enc_offsets.len().is_multiple_of(4) {
        return Err(Error::InvalidRecord("enc_offsets length"));
    }
    if !(record.r.is_finite() && record.r > 0.0 && record.aspect.is_finite()) {
        return Err(Error::InvalidRecord("geometry"));
    }
    if taps.len() != record.tap_count() || taps.iter().any(|p| !in_bounds(p, record.aspect)) {
        return Ok(false);
    }

    let lattice = Lattice::new(record.r);
    let ctx = record.context();
    let mut indices: Vec<CellIndex> = Vec::with_capacity(taps.len());
    for (i, tap) in taps.iter().enumerate() {
        let offset = if i == 0 {
            Offset::from_bytes(record.first_offset)
        } else {
            let ks = offset_keystream(pepper, record.pepper_version, &record.salt, &ctx, &indices)?;
            let start = 4 * (i - 1);
            let mut enc = [0u8; 4];
            enc.copy_from_slice(&record.enc_offsets[start..start + 4]);
            // No integrity check on purpose: a wrong key must give a
            // plausible offset, never an error, or taps become checkable
            // one at a time.
            Offset::from_bytes(xor4(enc, ks))
        };
        indices.push(lattice.locate(*tap, offset));
    }

    let candidate = compute_verifier(
        pepper,
        record.pepper_version,
        &record.salt,
        &ctx,
        &record.kdf,
        &indices,
    )?;
    Ok(candidate.ct_eq(&record.verifier).into())
}

fn in_bounds(p: &Point, aspect: f64) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.x >= 0.0 && p.x <= 1.0 && p.y >= 0.0 && p.y <= aspect
}

fn context(image_id: &str, r: f64, aspect: f64) -> Vec<u8> {
    let id = image_id.as_bytes();
    let mut out = Vec::with_capacity(4 + id.len() + 16);
    out.extend_from_slice(&(id.len() as u32).to_be_bytes());
    out.extend_from_slice(id);
    out.extend_from_slice(&r.to_bits().to_be_bytes());
    out.extend_from_slice(&aspect.to_bits().to_be_bytes());
    out
}

fn encode_indices(indices: &[CellIndex]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 8 * indices.len());
    out.push(indices.len() as u8);
    for i in indices {
        out.extend_from_slice(&i.encode());
    }
    out
}

fn xor4(a: [u8; 4], b: [u8; 4]) -> [u8; 4] {
    [a[0] ^ b[0], a[1] ^ b[1], a[2] ^ b[2], a[3] ^ b[3]]
}

/// Keystream for the offset of the tap that follows `earlier`. The count of
/// earlier indices is part of the message, so each position gets its own key.
fn offset_keystream(
    pepper: &dyn PepperProvider,
    pepper_version: u32,
    salt: &[u8; 16],
    ctx: &[u8],
    earlier: &[CellIndex],
) -> Result<[u8; 4], Error> {
    let mut msg = Vec::with_capacity(DOMAIN_OFFSET.len() + 17 + ctx.len() + 1 + 8 * earlier.len());
    msg.extend_from_slice(DOMAIN_OFFSET);
    msg.push(0);
    msg.extend_from_slice(salt);
    msg.extend_from_slice(ctx);
    msg.extend_from_slice(&encode_indices(earlier));
    let mac = pepper.hmac(pepper_version, &msg)?;
    Ok([mac[0], mac[1], mac[2], mac[3]])
}

fn compute_verifier(
    pepper: &dyn PepperProvider,
    pepper_version: u32,
    salt: &[u8; 16],
    ctx: &[u8],
    kdf: &KdfParams,
    indices: &[CellIndex],
) -> Result<[u8; 32], Error> {
    let mut password = Vec::with_capacity(DOMAIN_KDF.len() + 1 + ctx.len() + 1 + 8 * indices.len());
    password.extend_from_slice(DOMAIN_KDF);
    password.push(0);
    password.extend_from_slice(ctx);
    password.extend_from_slice(&encode_indices(indices));

    let params =
        Params::new(kdf.m_kib, kdf.t, kdf.p, Some(32)).map_err(|e| Error::Kdf(e.to_string()))?;
    let mut stretched = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(&password, salt, &mut stretched)
        .map_err(|e| Error::Kdf(e.to_string()))?;

    let mut msg = Vec::with_capacity(DOMAIN_FINAL.len() + 1 + 32);
    msg.extend_from_slice(DOMAIN_FINAL);
    msg.push(0);
    msg.extend_from_slice(&stretched);
    pepper.hmac(pepper_version, &msg)
}
